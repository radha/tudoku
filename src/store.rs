//! Saving the game in progress and best times as small JSON files in the
//! user's data directory: `~/.local/share/tudoku` on Linux,
//! `~/Library/Application Support/tudoku` on macOS.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::deal::Difficulty;
use crate::game::{Game, SavedGame};

const GAME_FILE: &str = "game.json";
const STATS_FILE: &str = "stats.json";

/// Bumped when the level names change. Version 0 is the five-level build.
const STATS_VERSION: u32 = 1;

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
        entry.solved = entry.solved.saturating_add(1);
        let ms = u64::try_from(time.as_millis()).unwrap_or(u64::MAX);
        let is_best = hints_used == 0 && entry.best_ms.is_none_or(|best| ms < best);
        if is_best {
            entry.best_ms = Some(ms);
        }
        is_best
    }
}

/// Stats as written to disk, keyed by level name.
#[derive(Serialize, Deserialize)]
struct StatsFile {
    #[serde(default)]
    version: u32,
    levels: BTreeMap<String, LevelStats>,
}

impl StatsFile {
    fn from_stats(stats: &Stats) -> Self {
        Self {
            version: STATS_VERSION,
            levels: stats
                .levels
                .iter()
                .map(|(level, s)| (level.name().to_string(), *s))
                .collect(),
        }
    }

    /// Levels by their current names; version 0 files use the old names
    /// and are converted. Names that mean nothing now are skipped.
    fn into_stats(self) -> Stats {
        let mut stats = Stats::default();
        for (name, level_stats) in self.levels {
            let level = match self.version {
                0 if name == "Easy" => None, // the old Easy (45 clues) is gone
                0 => Difficulty::from_legacy_name(&name),
                _ => Difficulty::from_name(&name),
            };
            if let Some(level) = level {
                let entry = stats.levels.entry(level).or_default();
                entry.solved = entry.solved.saturating_add(level_stats.solved);
                entry.best_ms = match (entry.best_ms, level_stats.best_ms) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
        }
        stats
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

    /// Stats as they are on disk now (`None` for a store that keeps
    /// nothing). An unreadable file is moved aside to `stats.json.damaged`
    /// rather than silently overwritten by the next solve.
    pub fn load_stats(&self) -> Option<Stats> {
        let path = self.path(STATS_FILE)?;
        let Ok(text) = fs::read_to_string(&path) else {
            return Some(Stats::default());
        };
        Some(match serde_json::from_str::<StatsFile>(&text) {
            Ok(file) => file.into_stats(),
            Err(_) => {
                let _ = fs::rename(&path, path.with_extension("json.damaged"));
                Stats::default()
            }
        })
    }

    pub fn save_stats(&self, stats: &Stats) -> io::Result<()> {
        self.write(STATS_FILE, &StatsFile::from_stats(stats))
    }
}

/// Write to a temporary file and rename it into place, so a crash mid-write
/// never leaves a truncated save behind. Each write gets its own temporary
/// name, so two running copies of tudoku can't clobber each other's.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let n = WRITES.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{}-{n}.tmp", std::process::id()));
    let written = fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
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
        assert_eq!(store.load_stats(), Some(Stats::default()));

        let mut g = game();
        g.enter_digit(4);
        store.save_game(&g).unwrap();
        let back = store.load_game().expect("saved game loads");
        assert_eq!(back.values, g.values);

        let mut stats = Stats::default();
        stats.record(Difficulty::Zen, Duration::from_secs(61), 0);
        store.save_stats(&stats).unwrap();
        assert_eq!(store.load_stats(), Some(stats));
        let text = fs::read_to_string(dir.path().join(STATS_FILE)).unwrap();
        assert!(text.contains("\"Zen\""), "levels are keyed by name: {text}");

        store.clear_game().unwrap();
        assert!(store.load_game().is_none());
        store.clear_game().expect("clearing twice is fine");
    }

    #[test]
    fn legacy_stats_move_to_the_levels_their_puzzles_belong_to() {
        // Written by the five-level build: Easy, Medium, Hard, Expert, Zen.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(STATS_FILE), r#"{"levels":{"Easy":{"solved":3,"best_ms":1000},"Hard":{"solved":1,"best_ms":594328},"Expert":{"solved":2,"best_ms":2000},"Zen":{"solved":32,"best_ms":330137}}}"#).unwrap();
        let stats = Store::at(dir.path()).load_stats().unwrap();
        assert_eq!(
            stats.level(Difficulty::Easy),
            LevelStats::default(),
            "old Easy (45 clues) is dropped"
        );
        assert_eq!(
            stats.level(Difficulty::Medium),
            LevelStats {
                solved: 1,
                best_ms: Some(594328)
            }
        );
        assert_eq!(stats.level(Difficulty::Hard).solved, 2);
        assert_eq!(
            stats.level(Difficulty::Master),
            LevelStats {
                solved: 32,
                best_ms: Some(330137)
            }
        );
        assert_eq!(stats.level(Difficulty::Zen), LevelStats::default());
        assert!(!dir.path().join("stats.json.damaged").exists());
    }

    #[test]
    fn converted_stats_are_never_converted_again() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        let mut stats = Stats::default();
        stats.record(Difficulty::Hard, Duration::from_secs(90), 0);
        store.save_stats(&stats).unwrap();
        let text = fs::read_to_string(dir.path().join(STATS_FILE)).unwrap();
        assert!(text.contains("\"version\":1"), "{text}");
        assert_eq!(store.load_stats(), Some(stats), "Hard stays Hard");
    }

    #[test]
    fn unknown_level_names_are_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(STATS_FILE), r#"{"version":1,"levels":{"Nope":{"solved":1,"best_ms":null},"Zen":{"solved":4,"best_ms":9}}}"#).unwrap();
        let stats = Store::at(dir.path()).load_stats().unwrap();
        assert_eq!(stats.level(Difficulty::Zen).solved, 4);
    }

    #[test]
    fn damaged_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        fs::write(dir.path().join(GAME_FILE), "{ not json").unwrap();
        fs::write(dir.path().join(STATS_FILE), "[1, 2").unwrap();
        assert!(store.load_game().is_none());
        assert_eq!(store.load_stats(), Some(Stats::default()));
        assert!(
            dir.path().join("stats.json.damaged").exists(),
            "a damaged stats file is kept, not overwritten"
        );
    }

    #[test]
    fn a_store_without_a_directory_keeps_nothing() {
        let store = Store::none();
        store.save_game(&game()).unwrap();
        assert!(store.load_game().is_none());
        assert_eq!(store.load_stats(), None);
        store.clear_game().unwrap();
    }

    #[test]
    fn concurrent_writers_never_tear_or_collide() {
        let dir = tempfile::tempdir().unwrap();
        std::thread::scope(|s| {
            for t in 0..2u64 {
                let path = dir.path();
                s.spawn(move || {
                    let store = Store::at(path);
                    for i in 0..200 {
                        let mut stats = Stats::default();
                        stats.record(Difficulty::Easy, Duration::from_secs(t * 1000 + i), 0);
                        store.save_stats(&stats).expect("no shared temp file");
                        assert!(store.load_stats().is_some());
                    }
                });
            }
        });
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != STATS_FILE)
            .collect();
        assert!(
            leftovers.is_empty(),
            "no temp or damaged files: {leftovers:?}"
        );
    }
}
