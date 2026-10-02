//! Saving the game in progress and best times as small JSON files in the
//! user's data directory: `~/.local/share/tudoku` on Linux,
//! `~/Library/Application Support/tudoku` on macOS.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::deal::Difficulty;
use crate::game::{Game, SavedGame};

const GAME_FILE: &str = "game.json";
const STATS_FILE: &str = "stats.json";

/// Per-level record. Best times only count games solved without hints.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelStats {
    pub solved: u32,
    pub best_ms: Option<u64>,
}

impl LevelStats {
    pub fn best(&self) -> Option<Duration> {
        self.best_ms.map(Duration::from_millis)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    levels: BTreeMap<Difficulty, LevelStats>,
}

impl Stats {
    pub fn level(&self, level: Difficulty) -> LevelStats {
        self.levels.get(&level).copied().unwrap_or_default()
    }

    /// Count a solve. Returns true when it set a new best time, which only
    /// a hint-free solve can do.
    pub fn record(&mut self, level: Difficulty, time: Duration, hints_used: u32) -> bool {
        let entry = self.levels.entry(level).or_default();
        entry.solved += 1;
        let ms = u64::try_from(time.as_millis()).unwrap_or(u64::MAX);
        let is_best = hints_used == 0 && entry.best_ms.is_none_or(|best| ms < best);
        if is_best {
            entry.best_ms = Some(ms);
        }
        is_best
    }
}

/// Where saves live. A store without a directory (no home, or in tests)
/// silently keeps nothing.
pub struct Store {
    dir: Option<PathBuf>,
}

impl Store {
    pub fn open_default() -> Self {
        Self {
            dir: dirs::data_dir().map(|d| d.join("tudoku")),
        }
    }

    #[cfg(test)]
    pub fn at(dir: &Path) -> Self {
        Self {
            dir: Some(dir.to_path_buf()),
        }
    }

    #[cfg(test)]
    pub fn none() -> Self {
        Self { dir: None }
    }

    fn path(&self, file: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(file))
    }

    fn read<T: for<'de> Deserialize<'de>>(&self, file: &str) -> Option<T> {
        let text = fs::read_to_string(self.path(file)?).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write<T: Serialize>(&self, file: &str, value: &T) -> io::Result<()> {
        let Some(path) = self.path(file) else {
            return Ok(());
        };
        let json = serde_json::to_string(value).map_err(io::Error::other)?;
        write_atomic(&path, json.as_bytes())
    }

    /// The saved game, if there is one and it checks out. A damaged or
    /// stale save is ignored rather than trusted.
    pub fn load_game(&self) -> Option<Game> {
        Game::restore(self.read::<SavedGame>(GAME_FILE)?).ok()
    }

    pub fn save_game(&self, game: &Game) -> io::Result<()> {
        self.write(GAME_FILE, &game.snapshot())
    }

    pub fn clear_game(&self) -> io::Result<()> {
        match self.path(GAME_FILE).map(fs::remove_file) {
            Some(Err(e)) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    pub fn load_stats(&self) -> Stats {
        self.read(STATS_FILE).unwrap_or_default()
    }

    pub fn save_stats(&self, stats: &Stats) -> io::Result<()> {
        self.write(STATS_FILE, stats)
    }
}

/// Write to a temporary file and rename it into place, so a crash mid-write
/// never leaves a truncated save behind.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sudoku::test_board;

    fn game() -> Game {
        let (p, s) = test_board();
        Game::new(Difficulty::Hard, p, s)
    }

    #[test]
    fn best_times_count_only_hint_free_solves() {
        let mut stats = Stats::default();
        let secs = Duration::from_secs;
        assert!(
            stats.record(Difficulty::Hard, secs(300), 0),
            "first clean solve"
        );
        assert!(
            !stats.record(Difficulty::Hard, secs(100), 2),
            "hints never count"
        );
        assert!(!stats.record(Difficulty::Hard, secs(400), 0), "slower");
        assert!(stats.record(Difficulty::Hard, secs(200), 0), "faster");
        let hard = stats.level(Difficulty::Hard);
        assert_eq!(hard.solved, 4);
        assert_eq!(hard.best(), Some(secs(200)));
        assert_eq!(stats.level(Difficulty::Zen), LevelStats::default());
    }

    #[test]
    fn games_and_stats_round_trip_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        assert!(store.load_game().is_none());
        assert_eq!(store.load_stats(), Stats::default());

        let mut g = game();
        g.enter_digit(4);
        store.save_game(&g).unwrap();
        let back = store.load_game().expect("saved game loads");
        assert_eq!(back.values, g.values);

        let mut stats = Stats::default();
        stats.record(Difficulty::Zen, Duration::from_secs(61), 0);
        store.save_stats(&stats).unwrap();
        assert_eq!(store.load_stats(), stats);
        let text = fs::read_to_string(dir.path().join(STATS_FILE)).unwrap();
        assert!(text.contains("\"Zen\""), "levels are keyed by name: {text}");

        store.clear_game().unwrap();
        assert!(store.load_game().is_none());
        store.clear_game().expect("clearing twice is fine");
    }

    #[test]
    fn damaged_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        fs::write(dir.path().join(GAME_FILE), "{ not json").unwrap();
        fs::write(dir.path().join(STATS_FILE), "[1, 2").unwrap();
        assert!(store.load_game().is_none());
        assert_eq!(store.load_stats(), Stats::default());
    }

    #[test]
    fn a_store_without_a_directory_keeps_nothing() {
        let store = Store::none();
        store.save_game(&game()).unwrap();
        assert!(store.load_game().is_none());
        store.clear_game().unwrap();
    }
}
