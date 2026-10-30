//! Difficulty levels and dealing puzzles that match them.
//!
//! A level is defined by the hardest technique a puzzle needs, as graded
//! by the human-style solver in `logic`. Every puzzle has exactly one
//! solution and can be finished by logic alone — no guessing, ever.

use std::num::NonZero;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rand::Rng;

use crate::logic::{self, Grade, Technique};
use crate::sudoku::{self, Board, CELLS};

/// Five difficulty levels, ordered easy -> zen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Master,
    Zen,
}

impl Difficulty {
    pub const ALL: [Difficulty; 5] = [
        Difficulty::Easy,
        Difficulty::Medium,
        Difficulty::Hard,
        Difficulty::Master,
        Difficulty::Zen,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
            Difficulty::Master => "Master",
            Difficulty::Zen => "Zen",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Easy => "Warm-up: just singles",
            Difficulty::Medium => "Needs notes: pointing & pairs",
            Difficulty::Hard => "X-Wing, XY-Wing & rectangles",
            Difficulty::Master => "Big fish, wings & colouring",
            Difficulty::Zen => "Chains & sets",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether a puzzle whose hardest step is `hardest` belongs at this
    /// level.
    pub fn accepts(self, hardest: Technique) -> bool {
        level_of(hardest) == self
    }

    /// The level with this name (as shown and as saved).
    pub fn from_name(name: &str) -> Option<Difficulty> {
        Difficulty::ALL.into_iter().find(|d| d.name() == name)
    }

    /// The level a name from before the tutor (Easy, Medium, Hard, Expert,
    /// Zen) stands for now, keeping the same puzzles.
    pub fn from_legacy_name(name: &str) -> Option<Difficulty> {
        match name {
            "Easy" => Some(Difficulty::Easy),
            "Medium" => Some(Difficulty::Easy),
            "Hard" => Some(Difficulty::Medium),
            "Expert" => Some(Difficulty::Hard),
            "Zen" => Some(Difficulty::Master),
            _ => None,
        }
    }

    /// The hardest technique this level's puzzles may need.
    fn ceiling(self) -> Technique {
        *Technique::ALL
            .iter()
            .rfind(|&&t| level_of(t) == self)
            .expect("every level has a technique")
    }

    /// How many cells to blank. The harder levels dig until no more cells
    /// can go, which leaves ~22-27 clues.
    fn holes(self) -> usize {
        match self {
            Difficulty::Easy => 50,
            _ => CELLS,
        }
    }
}

/// The level whose puzzles have `t` as their hardest step.
pub fn level_of(t: Technique) -> Difficulty {
    use Technique::*;
    match t {
        FullHouse | NakedSingle | HiddenSingle => Difficulty::Easy,
        Pointing | Claiming | NakedPair | HiddenPair | NakedTriple | HiddenTriple => {
            Difficulty::Medium
        }
        NakedQuad | HiddenQuad | XWing | Skyscraper | TwoStringKite | Crane | EmptyRectangle
        | XYWing | UniqueRectangle1 | UniqueRectangle2 | UniqueRectangle3 | UniqueRectangle4 => {
            Difficulty::Hard
        }
        Swordfish | XYZWing | WWing | FinnedXWing | RemotePair | SimpleColouring | BugPlusOne
        | Jellyfish | FinnedSwordfish | WXYZWing | FinnedJellyfish => Difficulty::Master,
        XChain | XYChain | Aic | AlsXz | AlsXyWing | SueDeCoq | DeathBlossom => Difficulty::Zen,
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
    // Grading gives up at the first step too hard for the level, so a
    // puzzle that belongs higher up never runs the hardest techniques.
    let grade = logic::grade_within(&puzzle, level.ceiling())?;
    level.accepts(grade.hardest).then_some(Deal {
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
/// rarest level (Master) needs ~10 attempts of ~0.8ms each on average.
/// The others finish their attempts before this returns, a millisecond
/// or two, so a worker that panics takes the deal down with it, where the
/// caller sees it, rather than printing over the game later.
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
    fn five_levels_in_order() {
        assert_eq!(
            Difficulty::ALL.map(Difficulty::name),
            ["Easy", "Medium", "Hard", "Master", "Zen"]
        );
    }

    #[test]
    fn technique_levels_never_go_down() {
        let levels: Vec<_> = Technique::ALL.iter().map(|&t| level_of(t)).collect();
        assert!(levels.windows(2).all(|w| w[0] <= w[1]), "{levels:?}");
        assert_eq!(level_of(Technique::XWing), Difficulty::Hard);
        assert_eq!(level_of(Technique::Swordfish), Difficulty::Master);
    }

    #[test]
    fn zen_deals_only_chain_and_set_puzzles() {
        assert!(!Difficulty::Zen.accepts(Technique::Jellyfish));
        assert!(Difficulty::Zen.accepts(Technique::XChain));
        assert!(Difficulty::Zen.accepts(Technique::DeathBlossom));
        assert!(Difficulty::Master.accepts(Technique::Jellyfish));
        assert!(!Difficulty::Master.accepts(Technique::XWing));
    }

    #[test]
    fn a_level_s_ceiling_is_its_hardest_technique() {
        for level in Difficulty::ALL {
            let ceiling = level.ceiling();
            assert!(level.accepts(ceiling), "{level:?}");
            let harder = &Technique::ALL[ceiling as usize + 1..];
            assert!(harder.iter().all(|&t| level_of(t) > level), "{level:?}");
        }
    }

    #[test]
    fn grading_up_to_a_level_decides_like_grading_in_full() {
        let mut rng = StdRng::seed_from_u64(31337);
        for _ in 0..25 {
            let solution = sudoku::full_solution(&mut rng);
            let puzzle = sudoku::dig(&solution, CELLS, &mut rng);
            let full = logic::grade(&puzzle);
            for level in Difficulty::ALL {
                let capped = logic::grade_within(&puzzle, level.ceiling());
                let fits = |g: &Grade| level.accepts(g.hardest);
                assert_eq!(capped.filter(fits), full.filter(fits), "{level:?}");
            }
        }
    }

    #[test]
    fn legacy_names_map_to_the_same_puzzles() {
        use Difficulty::*;
        for (old, new) in [
            ("Easy", Easy),
            ("Medium", Easy),
            ("Hard", Medium),
            ("Expert", Hard),
            ("Zen", Master),
        ] {
            assert_eq!(Difficulty::from_legacy_name(old), Some(new), "{old}");
        }
        assert_eq!(Difficulty::from_legacy_name("Nope"), None);
    }

    #[test]
    fn every_level_deals_a_unique_puzzle_graded_at_that_level() {
        let mut rng = StdRng::seed_from_u64(5);
        for level in Difficulty::ALL {
            let d = deal(level, &mut rng);
            assert!(sudoku::is_valid_solution(&d.solution), "{level:?}");
            assert!(sudoku::is_unique(&d.puzzle), "{level:?}");
            assert_eq!(sudoku::solve_one(&d.puzzle), Some(d.solution));
            assert!(level.accepts(d.grade.hardest), "{level:?}");
            assert_eq!(logic::grade(&d.puzzle), Some(d.grade));
        }
    }

    #[test]
    fn easier_levels_keep_more_clues() {
        let mut rng = StdRng::seed_from_u64(6);
        let easy = deal(Difficulty::Easy, &mut rng).givens();
        assert!(easy <= 31);
        assert!(deal(Difficulty::Medium, &mut rng).givens() < easy);
    }

    #[test]
    fn fast_dealing_matches_the_level() {
        let d = deal_fast(Difficulty::Medium);
        assert!(Difficulty::Medium.accepts(d.grade.hardest));
        assert!(sudoku::is_unique(&d.puzzle));
    }
}
