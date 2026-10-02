//! Difficulty levels and dealing puzzles that match them.
//!
//! A level is defined by the hardest technique a puzzle needs, as graded
//! by the human-style solver in `logic`. Every puzzle has exactly one
//! solution and can be finished by logic alone — no guessing, ever.

use std::num::NonZero;
use std::ops::RangeInclusive;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::logic::{self, Grade, Technique};
use crate::sudoku::{self, Board, CELLS};

/// Five difficulty levels, ordered easy -> zen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
    Zen,
}

impl Difficulty {
    pub const ALL: [Difficulty; 5] = [
        Difficulty::Easy,
        Difficulty::Medium,
        Difficulty::Hard,
        Difficulty::Expert,
        Difficulty::Zen,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
            Difficulty::Expert => "Expert",
            Difficulty::Zen => "Zen",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Easy => "Warm-up: singles, lots of clues",
            Difficulty::Medium => "Fewer clues, still just singles",
            Difficulty::Hard => "Needs notes: pointing & pairs",
            Difficulty::Expert => "Fish & wings: X-Wing, XY-Wing",
            Difficulty::Zen => "Barely there: Swordfish & XYZ",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// The techniques this level's hardest step must come from.
    pub fn techniques(self) -> RangeInclusive<Technique> {
        use Technique::*;
        match self {
            Difficulty::Easy | Difficulty::Medium => NakedSingle..=HiddenSingle,
            Difficulty::Hard => LockedCandidates..=HiddenTriple,
            Difficulty::Expert => XWing..=HiddenQuad,
            Difficulty::Zen => Swordfish..=Jellyfish,
        }
    }

    /// How many cells to blank. The harder levels dig until no more cells
    /// can go, which leaves ~22-27 clues.
    fn holes(self) -> usize {
        match self {
            Difficulty::Easy => 36,
            Difficulty::Medium => 50,
            _ => CELLS,
        }
    }
}

/// A freshly dealt puzzle.
#[derive(Debug, Clone, Copy)]
pub struct Deal {
    pub puzzle: Board,
    pub solution: Board,
    pub grade: Grade,
}

impl Deal {
    pub fn givens(&self) -> usize {
        self.puzzle.iter().filter(|&&v| v != 0).count()
    }
}

/// One attempt: dig a random grid and keep it if it grades at `level`.
fn try_deal<R: Rng>(level: Difficulty, rng: &mut R) -> Option<Deal> {
    let solution = sudoku::full_solution(rng);
    let puzzle = sudoku::dig(&solution, level.holes(), rng);
    let grade = logic::grade(&puzzle)?;
    level.techniques().contains(&grade.hardest).then_some(Deal {
        puzzle,
        solution,
        grade,
    })
}

/// Deal a puzzle at `level`. Deterministic for a seeded `rng`.
pub fn deal<R: Rng>(level: Difficulty, rng: &mut R) -> Deal {
    loop {
        if let Some(deal) = try_deal(level, rng) {
            return deal;
        }
    }
}

/// [`deal`] on every core at once; the first worker to succeed wins. The
/// rarest level (Zen) needs ~160 attempts of ~2ms each on average.
pub fn deal_fast(level: Difficulty) -> Deal {
    let workers = std::thread::available_parallelism().map_or(1, NonZero::get);
    let done = AtomicBool::new(false);
    let found = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                let mut rng = rand::rng();
                while !done.load(Ordering::Relaxed) {
                    if let Some(deal) = try_deal(level, &mut rng) {
                        if !done.swap(true, Ordering::Relaxed) {
                            *found.lock().unwrap_or_else(|e| e.into_inner()) = Some(deal);
                        }
                        return;
                    }
                }
            });
        }
    });
    found
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .expect("the first worker to succeed stores its deal")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn indices_match_order() {
        for (i, d) in Difficulty::ALL.iter().enumerate() {
            assert_eq!(d.index(), i);
        }
    }

    #[test]
    fn levels_cover_every_technique_once_and_in_order() {
        // Easy and Medium share singles and differ by clue count.
        let bands: Vec<_> = Difficulty::ALL[1..]
            .iter()
            .map(|d| d.techniques())
            .collect();
        for pair in bands.windows(2) {
            assert!(pair[0].end() < pair[1].start(), "{pair:?}");
        }
        assert_eq!(*bands[0].start(), Technique::NakedSingle);
        assert_eq!(*bands.last().unwrap().end(), Technique::Jellyfish);
    }

    #[test]
    fn every_level_deals_a_unique_puzzle_graded_at_that_level() {
        let mut rng = StdRng::seed_from_u64(5);
        for level in Difficulty::ALL {
            let d = deal(level, &mut rng);
            assert!(sudoku::is_valid_solution(&d.solution), "{level:?}");
            assert!(sudoku::is_unique(&d.puzzle), "{level:?}");
            assert_eq!(sudoku::solve_one(&d.puzzle), Some(d.solution));
            assert!(level.techniques().contains(&d.grade.hardest), "{level:?}");
            assert_eq!(logic::grade(&d.puzzle), Some(d.grade));
        }
    }

    #[test]
    fn easier_levels_keep_more_clues() {
        let mut rng = StdRng::seed_from_u64(6);
        assert_eq!(deal(Difficulty::Easy, &mut rng).givens(), 45);
        assert!(deal(Difficulty::Medium, &mut rng).givens() <= 31);
    }

    #[test]
    fn fast_dealing_matches_the_level() {
        let d = deal_fast(Difficulty::Hard);
        assert!(Difficulty::Hard.techniques().contains(&d.grade.hardest));
        assert!(sudoku::is_unique(&d.puzzle));
    }
}
