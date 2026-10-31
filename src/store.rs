//! Saving the game in progress and best times as small JSON files in the
//! user's data directory: `~/.local/share/tudoku` on Linux,
//! `~/Library/Application Support/tudoku` on macOS.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
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

/// Per-level record. Best times, averages and streaks only count games
/// solved without the tutor; files from before averages and streaks read
/// as having none yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelStats {
    pub solved: u32,
    pub best_ms: Option<u64>,
    /// Tutor-free solves, and their times added up, for the average.
    #[serde(default)]
    pub clean: u32,
    #[serde(default)]
    pub clean_ms: u64,
    /// Tutor-free solves in a row, now and at most.
    #[serde(default)]
    pub streak: u32,
    #[serde(default)]
    pub best_streak: u32,
}

impl LevelStats {
    pub fn best(&self) -> Option<Duration> {
        self.best_ms.map(Duration::from_millis)
    }

    /// The mean time of the tutor-free solves.
    pub fn average(&self) -> Option<Duration> {
        (self.clean > 0).then(|| Duration::from_millis(self.clean_ms / u64::from(self.clean)))
    }

    /// Fold in the record of a level that now goes by the same name.
    fn merge(&mut self, other: LevelStats) {
        self.solved = self.solved.saturating_add(other.solved);
        self.best_ms = match (self.best_ms, other.best_ms) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.clean = self.clean.saturating_add(other.clean);
        self.clean_ms = self.clean_ms.saturating_add(other.clean_ms);
        self.streak = self.streak.max(other.streak);
        self.best_streak = self.best_streak.max(other.best_streak);
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
    /// a solve without help (`helped == 0`) can do. A helped solve ends
    /// the streak.
    pub fn record(&mut self, level: Difficulty, time: Duration, helped: u32) -> bool {
        let entry = self.levels.entry(level).or_default();
        entry.solved = entry.solved.saturating_add(1);
        if helped > 0 {
            entry.streak = 0;
            return false;
        }
        let ms = u64::try_from(time.as_millis()).unwrap_or(u64::MAX);
        entry.clean = entry.clean.saturating_add(1);
        entry.clean_ms = entry.clean_ms.saturating_add(ms);
        entry.streak = entry.streak.saturating_add(1);
        entry.best_streak = entry.best_streak.max(entry.streak);
        let is_best = entry.best_ms.is_none_or(|best| ms < best);
        if is_best {
            entry.best_ms = Some(ms);
        }
        is_best
    }

    /// A game at `level` was given up: its streak starts over.
    pub fn abandon(&mut self, level: Difficulty) {
        if let Some(entry) = self.levels.get_mut(&level) {
            entry.streak = 0;
        }
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
                stats.levels.entry(level).or_default().merge(level_stats);
            }
        }
        stats
    }
}

/// What reading a file we wrote turned up.
enum Stored<T> {
    Missing,
    /// Not what we write: not text, not JSON, or the wrong shape.
    Damaged,
    Fine(T),
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

    /// Read and parse `file`. `Err` means it couldn't be read at all.
    fn read<T: for<'de> Deserialize<'de>>(&self, file: &str) -> io::Result<Stored<T>> {
        let Some(path) = self.path(file) else {
            return Ok(Stored::Missing);
        };
        match fs::read_to_string(path) {
            Ok(text) => Ok(serde_json::from_str(&text).map_or(Stored::Damaged, Stored::Fine)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Stored::Missing),
            Err(e) if e.kind() == io::ErrorKind::InvalidData => Ok(Stored::Damaged),
            Err(e) => Err(e),
        }
    }

    fn write<T: Serialize>(&self, file: &str, value: &T, durable: bool) -> io::Result<()> {
        let Some(path) = self.path(file) else {
            return Ok(());
        };
        let json = serde_json::to_string(value).map_err(io::Error::other)?;
        write_atomic(&path, json.as_bytes(), durable)
    }

    /// Move a file we can't use out of the way, keeping it for whoever
    /// wants to look, and never over an earlier one.
    fn set_aside(&self, file: &str) {
        let Some(path) = self.path(file) else { return };
        let free = (1..100)
            .map(|n| match n {
                1 => path.with_extension("json.damaged"),
                n => path.with_extension(format!("json.damaged-{n}")),
            })
            .find(|p| !p.exists());
        if let Some(to) = free {
            let _ = fs::rename(&path, to);
        }
    }

    /// The saved game, if there is one and it checks out. A damaged save,
    /// or one from a newer tudoku, is moved aside as `game.json.damaged`
    /// rather than trusted or overwritten.
    pub fn load_game(&self) -> Option<Game> {
        let game = match self.read::<SavedGame>(GAME_FILE).ok()? {
            Stored::Missing => return None,
            Stored::Damaged => None,
            Stored::Fine(saved) => Game::restore(saved).ok(),
        };
        if game.is_none() {
            self.set_aside(GAME_FILE);
        }
        game
    }

    /// Saved after every move, so this one skips the flush to disk: an OS
    /// crash can lose the last few moves, never the best times.
    pub fn save_game(&self, game: &Game) -> io::Result<()> {
        self.write(GAME_FILE, &game.snapshot(), false)
    }

    pub fn clear_game(&self) -> io::Result<()> {
        match self.path(GAME_FILE).map(fs::remove_file) {
            Some(Err(e)) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Stats as they are on disk now: `Ok(None)` for a store that keeps
    /// nothing, empty ones before the first solve. A damaged file is moved
    /// aside to `stats.json.damaged` rather than silently overwritten by
    /// the next solve. A file that can't be read at all is an error, so
    /// nobody writes over records they never saw.
    pub fn load_stats(&self) -> io::Result<Option<Stats>> {
        if self.dir.is_none() {
            return Ok(None);
        }
        Ok(Some(match self.read::<StatsFile>(STATS_FILE)? {
            Stored::Missing => Stats::default(),
            Stored::Fine(file) => file.into_stats(),
            Stored::Damaged => {
                self.set_aside(STATS_FILE);
                Stats::default()
            }
        }))
    }

    pub fn save_stats(&self, stats: &Stats) -> io::Result<()> {
        self.write(STATS_FILE, &StatsFile::from_stats(stats), true)
    }
}

/// Write to a temporary file and rename it into place, so a crash mid-write
/// never leaves a truncated save behind. Each write gets its own temporary
/// name, so two running copies of tudoku can't clobber each other's. A
/// `durable` write reaches the disk before it replaces the old file, so
/// even a power cut leaves one or the other whole.
fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> io::Result<()> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let n = WRITES.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{}-{n}.tmp", std::process::id()));
    let write = || -> io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        if durable {
            file.sync_all()?;
        }
        Ok(())
    };
    let written = write().and_then(|()| fs::rename(&tmp, path));
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
    fn best_times_count_only_tutor_free_solves() {
        let mut stats = Stats::default();
        let secs = Duration::from_secs;
        assert!(
            stats.record(Difficulty::Hard, secs(300), 0),
            "first clean solve"
        );
        assert!(
            !stats.record(Difficulty::Hard, secs(100), 2),
            "helped solves never count"
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
        assert_eq!(store.load_stats().unwrap(), Some(Stats::default()));

        let mut g = game();
        g.enter_digit(4);
        store.save_game(&g).unwrap();
        let back = store.load_game().expect("saved game loads");
        assert_eq!(back.values, g.values);

        let mut stats = Stats::default();
        stats.record(Difficulty::Zen, Duration::from_secs(61), 0);
        store.save_stats(&stats).unwrap();
        assert_eq!(store.load_stats().unwrap(), Some(stats));
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
        let stats = Store::at(dir.path()).load_stats().unwrap().unwrap();
        assert_eq!(
            stats.level(Difficulty::Easy),
            LevelStats::default(),
            "old Easy (45 clues) is dropped"
        );
        assert_eq!(
            stats.level(Difficulty::Medium),
            LevelStats {
                solved: 1,
                best_ms: Some(594328),
                ..LevelStats::default()
            }
        );
        assert_eq!(stats.level(Difficulty::Hard).solved, 2);
        assert_eq!(
            stats.level(Difficulty::Master),
            LevelStats {
                solved: 32,
                best_ms: Some(330137),
                ..LevelStats::default()
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
        assert_eq!(store.load_stats().unwrap(), Some(stats), "Hard stays Hard");
    }

    #[test]
    fn unknown_level_names_are_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(STATS_FILE), r#"{"version":1,"levels":{"Nope":{"solved":1,"best_ms":null},"Zen":{"solved":4,"best_ms":9}}}"#).unwrap();
        let stats = Store::at(dir.path()).load_stats().unwrap().unwrap();
        assert_eq!(stats.level(Difficulty::Zen).solved, 4);
    }

    #[test]
    fn damaged_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        fs::write(dir.path().join(GAME_FILE), "{ not json").unwrap();
        fs::write(dir.path().join(STATS_FILE), "[1, 2").unwrap();
        assert!(store.load_game().is_none());
        assert_eq!(store.load_stats().unwrap(), Some(Stats::default()));
        assert!(
            dir.path().join("stats.json.damaged").exists(),
            "a damaged stats file is kept, not overwritten"
        );
        assert!(
            dir.path().join("game.json.damaged").exists(),
            "so is a damaged save"
        );
        // A second damaged file never replaces the first one set aside.
        fs::write(dir.path().join(STATS_FILE), "{").unwrap();
        store.load_stats().unwrap();
        let first = fs::read_to_string(dir.path().join("stats.json.damaged")).unwrap();
        assert_eq!(first, "[1, 2");
        assert!(dir.path().join("stats.json.damaged-2").exists());
    }

    #[test]
    fn stats_that_are_not_text_are_set_aside_too() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(STATS_FILE), b"{\"levels\":\xff}").unwrap();
        let store = Store::at(dir.path());
        assert_eq!(store.load_stats().unwrap(), Some(Stats::default()));
        assert!(dir.path().join("stats.json.damaged").exists());
    }

    #[test]
    fn stats_that_cannot_be_read_are_an_error_not_a_blank_slate() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(STATS_FILE)).unwrap();
        assert!(Store::at(dir.path()).load_stats().is_err());
        assert!(dir.path().join(STATS_FILE).is_dir(), "left where it is");
    }

    #[test]
    fn a_save_from_a_newer_tudoku_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        let mut saved = serde_json::to_value(game().snapshot()).unwrap();
        saved["version"] = 99.into();
        fs::write(dir.path().join(GAME_FILE), saved.to_string()).unwrap();
        assert!(store.load_game().is_none());
        assert!(dir.path().join("game.json.damaged").exists());
        assert!(!dir.path().join(GAME_FILE).exists());
    }

    #[test]
    fn averages_and_streaks_count_tutor_free_solves() {
        let mut stats = Stats::default();
        let secs = Duration::from_secs;
        assert_eq!(stats.level(Difficulty::Easy).average(), None);
        stats.record(Difficulty::Easy, secs(100), 0);
        stats.record(Difficulty::Easy, secs(200), 0);
        stats.record(Difficulty::Easy, secs(10), 1); // helped: no average, streak over
        stats.record(Difficulty::Easy, secs(300), 0);
        let easy = stats.level(Difficulty::Easy);
        assert_eq!(easy.solved, 4);
        assert_eq!(easy.average(), Some(secs(200)));
        assert_eq!((easy.streak, easy.best_streak), (1, 2));
        stats.abandon(Difficulty::Easy);
        let easy = stats.level(Difficulty::Easy);
        assert_eq!((easy.streak, easy.best_streak), (0, 2));
        stats.abandon(Difficulty::Zen); // nothing to end
        assert_eq!(stats.level(Difficulty::Zen), LevelStats::default());
    }

    #[test]
    fn stats_from_before_averages_load_with_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(STATS_FILE),
            r#"{"version":1,"levels":{"Zen":{"solved":4,"best_ms":9000}}}"#,
        )
        .unwrap();
        let zen = Store::at(dir.path())
            .load_stats()
            .unwrap()
            .unwrap()
            .level(Difficulty::Zen);
        assert_eq!(
            (zen.solved, zen.best_ms, zen.clean, zen.streak),
            (4, Some(9000), 0, 0)
        );
        assert_eq!(zen.average(), None);
    }

    #[test]
    fn a_store_without_a_directory_keeps_nothing() {
        let store = Store::none();
        store.save_game(&game()).unwrap();
        assert!(store.load_game().is_none());
        assert_eq!(store.load_stats().unwrap(), None);
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
                        assert!(store.load_stats().unwrap().is_some());
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
