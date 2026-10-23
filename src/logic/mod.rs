//! Human-style solving: the techniques people use, easiest first.
//!
//! The generator grades every puzzle by the hardest technique it needs, so
//! a level means "how clever you have to be", not just "how many blanks".
//! The tutor uses the same solver to name the step a person could take next.
//! Every step carries the pattern behind it and a reason in plain words,
//! so it can be named, explained and drawn on the board.

mod basic;
mod fish;
mod single_digit;
mod wings;

use crate::sudoku::{
    ALL_DIGITS, Board, CELLS, HOUSES, N, box_of_cell, candidates_mask, col_of, peers, row_of,
};

/// Solving techniques, ordered from easiest to hardest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Technique {
    FullHouse,
    NakedSingle,
    HiddenSingle,
    Pointing,
    Claiming,
    NakedPair,
    HiddenPair,
    NakedTriple,
    HiddenTriple,
    NakedQuad,
    HiddenQuad,
    XWing,
    Skyscraper,
    TwoStringKite,
    Crane,
    EmptyRectangle,
    XYWing,
    Swordfish,
    XYZWing,
    FinnedXWing,
    Jellyfish,
    FinnedSwordfish,
    FinnedJellyfish,
}

impl Technique {
    pub const ALL: &'static [Technique] = &[
        Technique::FullHouse,
        Technique::NakedSingle,
        Technique::HiddenSingle,
        Technique::Pointing,
        Technique::Claiming,
        Technique::NakedPair,
        Technique::HiddenPair,
        Technique::NakedTriple,
        Technique::HiddenTriple,
        Technique::NakedQuad,
        Technique::HiddenQuad,
        Technique::XWing,
        Technique::Skyscraper,
        Technique::TwoStringKite,
        Technique::Crane,
        Technique::EmptyRectangle,
        Technique::XYWing,
        Technique::Swordfish,
        Technique::XYZWing,
        Technique::FinnedXWing,
        Technique::Jellyfish,
        Technique::FinnedSwordfish,
        Technique::FinnedJellyfish,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Technique::FullHouse => "full house",
            Technique::NakedSingle => "naked single",
            Technique::HiddenSingle => "hidden single",
            Technique::Pointing => "pointing",
            Technique::Claiming => "claiming",
            Technique::NakedPair => "naked pair",
            Technique::HiddenPair => "hidden pair",
            Technique::NakedTriple => "naked triple",
            Technique::HiddenTriple => "hidden triple",
            Technique::NakedQuad => "naked quad",
            Technique::HiddenQuad => "hidden quad",
            Technique::XWing => "X-Wing",
            Technique::Skyscraper => "Skyscraper",
            Technique::TwoStringKite => "2-String Kite",
            Technique::Crane => "Crane",
            Technique::EmptyRectangle => "Empty Rectangle",
            Technique::XYWing => "XY-Wing",
            Technique::Swordfish => "Swordfish",
            Technique::XYZWing => "XYZ-Wing",
            Technique::FinnedXWing => "finned X-Wing",
            Technique::Jellyfish => "Jellyfish",
            Technique::FinnedSwordfish => "finned Swordfish",
            Technique::FinnedJellyfish => "finned Jellyfish",
        }
    }

    /// The technique whose `name()` is `name`, in any case.
    pub fn from_name(name: &str) -> Option<Technique> {
        Technique::ALL
            .iter()
            .copied()
            .find(|t| t.name().eq_ignore_ascii_case(name))
    }
}

/// What a step does to the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// `digit` goes in `cell`.
    Place { cell: usize, digit: u8 },
    /// Each `(cell, mask)` loses the candidates in `mask`.
    Eliminate(Vec<(usize, u16)>),
}

/// How a marked candidate is drawn: `Key` for the ones the step turns
/// on, `Other` for the ones that support them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Key,
    Other,
}

/// One candidate a pattern points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub cell: usize,
    pub digit: u8,
    pub role: Role,
}

/// `Strong`: if one end is false, the other is true. `Weak`: the two
/// ends can't both be true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Strong,
    Weak,
}

/// A line between two candidates, each a `(cell, digit)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub from: (usize, u8),
    pub to: (usize, u8),
    pub kind: LinkKind,
}

/// What to show on the board to explain a step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pattern {
    /// The one digit the pattern is about, if it is about one.
    pub digit: Option<u8>,
    /// For patterns on several digits, the cells whose notes stay visible.
    pub cells: Vec<usize>,
    /// Indexes into `HOUSES`, tinted on the board.
    pub houses: Vec<usize>,
    pub marks: Vec<Mark>,
    pub links: Vec<Link>,
}

/// One deduction: what it does, the pattern behind it and why it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub technique: Technique,
    pub effect: Effect,
    pub pattern: Pattern,
    pub why: String,
}

/// A cell as players name it: "r3c4".
pub fn rc(cell: usize) -> String {
    format!("r{}c{}", row_of(cell) + 1, col_of(cell) + 1)
}

/// A house as players name it: "row 3", "column 4" or "box 5".
pub fn house_name(house: usize) -> String {
    let kind = ["row", "column", "box"][house / N];
    format!("{kind} {}", house % N + 1)
}

/// The digits in `mask`, smallest first: "7", "2 and 7", "1, 4 and 9".
pub fn digit_list(mask: u16) -> String {
    listing(bits(mask).map(|d| d.to_string()), "and")
}

/// The digits in `mask` as a choice: "2 or 7", "1, 4 or 9".
fn digit_choice(mask: u16) -> String {
    listing(bits(mask).map(|d| d.to_string()), "or")
}

/// Cells by name: "r1c2 and r1c5".
fn cell_list(cells: &[usize]) -> String {
    listing(cells.iter().map(|&c| rc(c)), "and")
}

/// `items` as an English list whose last two are joined by `last`.
fn listing(items: impl IntoIterator<Item = String>, last: &str) -> String {
    let items: Vec<String> = items.into_iter().collect();
    match items.split_last() {
        Some((tail, rest)) if !rest.is_empty() => format!("{} {last} {tail}", rest.join(", ")),
        _ => items.concat(),
    }
}

/// Every `k`-element subset of the set bits in `universe`, as bitmasks.
fn subsets(universe: u16, k: u32) -> impl Iterator<Item = u16> {
    (0..=universe).filter(move |&s| s & !universe == 0 && s.count_ones() == k)
}

/// The indices of the set bits in `mask`, lowest first.
fn bits(mut mask: u16) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        (mask != 0).then(|| {
            let b = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            b
        })
    })
}

fn sees(a: usize, b: usize) -> bool {
    a != b && (row_of(a) == row_of(b) || col_of(a) == col_of(b) || box_of_cell(a) == box_of_cell(b))
}

/// A board plus pencil marks: the ones a perfect note-taker would have,
/// or ones someone noted by hand.
#[derive(Debug, Clone)]
pub struct Grid {
    values: Board,
    cands: [u16; CELLS],
}

impl Grid {
    pub fn new(values: &Board) -> Grid {
        let cands = std::array::from_fn(|i| candidates_mask(values, i));
        Grid::with_candidates(values, &cands)
    }

    /// A board with the given candidates. Filled cells keep none.
    pub fn with_candidates(values: &Board, cands: &[u16; CELLS]) -> Grid {
        Grid {
            values: *values,
            cands: std::array::from_fn(|i| {
                if values[i] == 0 {
                    cands[i] & ALL_DIGITS
                } else {
                    0
                }
            }),
        }
    }

    pub fn values(&self) -> &Board {
        &self.values
    }

    pub fn cands(&self) -> &[u16; CELLS] {
        &self.cands
    }

    pub fn is_solved(&self) -> bool {
        !self.values.contains(&0)
    }

    /// An empty cell with no candidates left: the board has no solution.
    fn is_broken(&self) -> bool {
        (0..CELLS).any(|i| self.values[i] == 0 && self.cands[i] == 0)
    }

    pub fn apply(&mut self, step: &Step) {
        match step.effect {
            Effect::Place { cell, digit } => {
                self.values[cell] = digit;
                self.cands[cell] = 0;
                for p in peers(cell) {
                    self.cands[p] &= !(1 << digit);
                }
            }
            Effect::Eliminate(ref removals) => {
                for &(cell, mask) in removals {
                    self.cands[cell] &= !mask;
                }
            }
        }
    }

    /// The easiest available deduction, if any technique finds one.
    pub fn next_step(&self) -> Option<Step> {
        if self.is_broken() {
            return None;
        }
        Technique::ALL.iter().find_map(|&t| self.find(t))
    }

    /// The first deduction technique `t` finds, if any.
    pub fn find(&self, t: Technique) -> Option<Step> {
        use Technique::*;
        match t {
            FullHouse => self.full_house(),
            NakedSingle => self.naked_single(),
            HiddenSingle => self.hidden_single(),
            Pointing => self.pointing(),
            Claiming => self.claiming(),
            NakedPair => self.naked_subset(2, t),
            HiddenPair => self.hidden_subset(2, t),
            NakedTriple => self.naked_subset(3, t),
            HiddenTriple => self.hidden_subset(3, t),
            NakedQuad => self.naked_subset(4, t),
            HiddenQuad => self.hidden_subset(4, t),
            XWing => self.fish(2, t),
            Skyscraper => self.skyscraper(),
            TwoStringKite => self.two_string_kite(),
            Crane => self.crane(),
            EmptyRectangle => self.empty_rectangle(),
            XYWing => self.xy_wing(),
            Swordfish => self.fish(3, t),
            XYZWing => self.xyz_wing(),
            FinnedXWing => self.finned_fish(2, t),
            Jellyfish => self.fish(4, t),
            FinnedSwordfish => self.finned_fish(3, t),
            FinnedJellyfish => self.finned_fish(4, t),
        }
    }

    /// A step removing each `(cell, mask)`, cut down to the candidates
    /// really there, or `None` if that leaves nothing to remove.
    fn eliminating(
        &self,
        technique: Technique,
        removals: Vec<(usize, u16)>,
        pattern: Pattern,
        why: String,
    ) -> Option<Step> {
        let removals: Vec<(usize, u16)> = removals
            .into_iter()
            .map(|(c, mask)| (c, mask & self.cands[c]))
            .filter(|&(_, mask)| mask != 0)
            .collect();
        (!removals.is_empty()).then_some(Step {
            technique,
            effect: Effect::Eliminate(removals),
            pattern,
            why,
        })
    }

    fn placing(
        &self,
        technique: Technique,
        cell: usize,
        digit: u8,
        pattern: Pattern,
        why: String,
    ) -> Step {
        Step {
            technique,
            effect: Effect::Place { cell, digit },
            pattern,
            why,
        }
    }

    /// Positions (0..9) within `house` whose cell has any of `mask`.
    fn positions(&self, house: &[u8; N], mask: u16) -> u16 {
        house
            .iter()
            .enumerate()
            .filter(|&(_, &c)| self.cands[usize::from(c)] & mask != 0)
            .fold(0, |m, (p, _)| m | 1 << p)
    }

    /// The two cells of house `house` where `digit` can go, if there are
    /// exactly two.
    fn conjugates(&self, house: usize, digit: u8) -> Option<(usize, usize)> {
        let pos = self.positions(&HOUSES[house], 1 << digit);
        if pos.count_ones() != 2 {
            return None;
        }
        let mut cells = bits(pos).map(|p| usize::from(HOUSES[house][p]));
        Some((cells.next()?, cells.next()?))
    }

    /// A mark for each candidate in `mask` that `cells` hold.
    fn marks(&self, cells: impl IntoIterator<Item = usize>, mask: u16, role: Role) -> Vec<Mark> {
        cells
            .into_iter()
            .flat_map(|cell| {
                bits(self.cands[cell] & mask).map(move |d| Mark {
                    cell,
                    digit: d as u8,
                    role,
                })
            })
            .collect()
    }
}

/// How a puzzle yields to logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grade {
    /// The hardest technique the solve needed.
    pub hardest: Technique,
    /// Total deductions, a rough measure of length.
    pub steps: u32,
}

/// Solve with logic alone. `None` means the puzzle needs a technique
/// beyond this solver (or guessing).
pub fn grade(puzzle: &Board) -> Option<Grade> {
    let mut grid = Grid::new(puzzle);
    let mut grade = Grade {
        hardest: Technique::FullHouse,
        steps: 0,
    };
    while !grid.is_solved() {
        let step = grid.next_step()?;
        grade.hardest = grade.hardest.max(step.technique);
        grade.steps += 1;
        grid.apply(&step);
    }
    Some(grade)
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// A grid whose candidates are set by hand, to stage one pattern.
    pub fn staged(cands: &[(usize, &[u8])]) -> Grid {
        // Fill every cell with a dummy value, then open the staged ones:
        // the techniques only look at candidate masks.
        let mut grid = Grid {
            values: [1; CELLS],
            cands: [0; CELLS],
        };
        for &(cell, digits) in cands {
            grid.values[cell] = 0;
            grid.cands[cell] = digits.iter().fold(0, |m, &d| m | 1 << d);
        }
        grid
    }

    /// The removals of an elimination step, sorted.
    pub fn removals(step: Option<Step>) -> Vec<(usize, u16)> {
        match step {
            Some(Step {
                effect: Effect::Eliminate(mut removals),
                ..
            }) => {
                removals.sort_unstable();
                removals
            }
            other => panic!("expected an elimination, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::staged;
    use super::*;
    use crate::sudoku::{self, idx, parse_board, test_board};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn subsets_and_bits() {
        assert_eq!(
            subsets(0b1011, 2).collect::<Vec<_>>(),
            [0b0011, 0b1001, 0b1010]
        );
        assert_eq!(bits(0b1010_0001).collect::<Vec<_>>(), [0, 5, 7]);
    }

    #[test]
    fn names_read_the_way_players_count() {
        assert_eq!(rc(idx(2, 3)), "r3c4");
        assert_eq!(house_name(2), "row 3");
        assert_eq!(house_name(N + 3), "column 4");
        assert_eq!(house_name(2 * N + 4), "box 5");
        assert_eq!(digit_list(1 << 7), "7");
        assert_eq!(digit_list((1 << 2) | (1 << 7)), "2 and 7");
        assert_eq!(digit_list((1 << 1) | (1 << 4) | (1 << 9)), "1, 4 and 9");
    }

    #[test]
    fn technique_list_is_complete() {
        // ALL lists every variant in declaration order with no gaps, so the
        // derived Ord (easiest first) agrees with it.
        for (i, &t) in Technique::ALL.iter().enumerate() {
            assert_eq!(t as usize, i, "{t:?}");
        }
        assert_eq!(*Technique::ALL.last().unwrap(), Technique::FinnedJellyfish);
    }

    #[test]
    fn techniques_are_found_by_name_in_any_case() {
        for &t in Technique::ALL {
            assert_eq!(Technique::from_name(t.name()), Some(t));
            assert_eq!(Technique::from_name(&t.name().to_uppercase()), Some(t));
        }
        assert_eq!(Technique::from_name("x-wing"), Some(Technique::XWing));
        assert_eq!(
            Technique::from_name("Naked Single"),
            Some(Technique::NakedSingle)
        );
        assert_eq!(Technique::from_name("dragon"), None);
        assert_eq!(Technique::from_name(""), None);
    }

    #[test]
    fn find_runs_only_the_asked_technique() {
        let (p, _) = test_board();
        let g = Grid::new(&p);
        for &t in Technique::ALL {
            assert!(g.find(t).is_none_or(|s| s.technique == t), "{t:?}");
        }
        let s = g.next_step().unwrap();
        assert_eq!(g.find(s.technique), Some(s));
    }

    #[test]
    fn full_house_fills_the_last_cell_of_a_house() {
        let (_, s) = test_board();
        let mut b = s;
        b[0] = 0;
        let step = Grid::new(&b).find(Technique::FullHouse).unwrap();
        assert_eq!(
            step.effect,
            Effect::Place {
                cell: 0,
                digit: s[0]
            }
        );
        assert_eq!(
            step.why,
            format!("r1c1 is the last empty cell in row 1, so it's a {}.", s[0])
        );
    }

    #[test]
    fn conjugates_need_exactly_two_spots() {
        let grid = staged(&[
            (idx(0, 1), &[3, 4]),
            (idx(0, 6), &[3, 5]),
            (idx(4, 1), &[3, 4]),
            (idx(7, 1), &[3, 6]),
        ]);
        assert_eq!(grid.conjugates(0, 3), Some((idx(0, 1), idx(0, 6))));
        assert_eq!(grid.conjugates(N + 1, 3), None, "three spots");
        assert_eq!(grid.conjugates(0, 5), None, "one spot");
    }

    #[test]
    fn easy_puzzle_falls_to_singles() {
        let (p, _) = test_board();
        let g = grade(&p).unwrap();
        assert!(g.hardest <= Technique::HiddenSingle);
        assert_eq!(g.steps, 51, "one placement per empty cell");
    }

    #[test]
    fn real_puzzles_grade_by_their_hardest_technique() {
        for (want, puzzle) in [
            (
                Technique::XWing,
                "087000500003000060150600000200500003000089002000073900000430000900005706040007000",
            ),
            (
                Technique::XYWing,
                "800000046007000020920004005000010800035740000080090004000000060600500000200006013",
            ),
            (
                Technique::Swordfish,
                "056000003200040008401000009084006302030090000100870000000000105019008000000250000",
            ),
        ] {
            let p = parse_board(puzzle).unwrap();
            assert!(sudoku::is_unique(&p));
            assert_eq!(grade(&p).map(|g| g.hardest), Some(want), "{puzzle}");
        }
    }

    /// Walk 20 seeded minimal puzzles, including ones logic alone can't
    /// finish, calling `visit` at every state with the step `next_step`
    /// takes there and the puzzle's solution.
    fn dealt_states(mut visit: impl FnMut(&Grid, &Step, &Board)) {
        let mut rng = StdRng::seed_from_u64(99);
        for _ in 0..20 {
            let solution = sudoku::full_solution(&mut rng);
            let puzzle = sudoku::dig(&solution, CELLS, &mut rng);
            let mut grid = Grid::new(&puzzle);
            while let Some(step) = grid.next_step() {
                visit(&grid, &step, &solution);
                grid.apply(&step);
            }
        }
    }

    /// Every detector, run at every state of the dealt puzzles, must agree
    /// with the real solution: placements match it and eliminations never
    /// remove its digit.
    #[test]
    fn every_detector_is_sound_on_dealt_puzzles() {
        let mut found = [0; Technique::ALL.len()];
        dealt_states(|grid, _, solution| {
            for &t in Technique::ALL {
                let Some(step) = grid.find(t) else { continue };
                assert_eq!(step.technique, t);
                assert_consistent(grid, &step);
                found[t as usize] += 1;
                match step.effect {
                    Effect::Place { cell, digit } => {
                        assert_eq!(
                            solution[cell],
                            digit,
                            "{t:?} placed {digit} at {}",
                            rc(cell)
                        );
                    }
                    Effect::Eliminate(removals) => {
                        for (cell, mask) in removals {
                            assert_eq!(
                                mask & (1 << solution[cell]),
                                0,
                                "{t:?} removed the answer at {}",
                                rc(cell)
                            );
                        }
                    }
                }
            }
        });
        // The run reaches well past the singles.
        use Technique::*;
        for t in [
            Pointing,
            Claiming,
            HiddenQuad,
            XWing,
            Skyscraper,
            TwoStringKite,
            Crane,
            EmptyRectangle,
            XYZWing,
            FinnedXWing,
            Jellyfish,
            FinnedSwordfish,
            FinnedJellyfish,
        ] {
            assert!(found[t as usize] > 0, "{t:?} never fired: {found:?}");
        }
    }

    /// A step's pattern points at the board as it stood before the step.
    fn assert_consistent(g: &Grid, step: &Step) {
        let (t, p) = (step.technique, &step.pattern);
        let removals: &[(usize, u16)] = match &step.effect {
            Effect::Place { cell, .. } => {
                assert_eq!(g.values[*cell], 0, "{t:?} places on a filled cell");
                &[]
            }
            Effect::Eliminate(removals) => removals,
        };
        let marked = |cell, digit| p.marks.iter().any(|m| (m.cell, m.digit) == (cell, digit));
        let removed = |cell, digit: u8| {
            removals
                .iter()
                .any(|&(c, mask)| c == cell && mask & (1 << digit) != 0)
        };
        for m in &p.marks {
            assert_eq!(g.values[m.cell], 0, "{t:?} marks filled {}", rc(m.cell));
            assert_ne!(
                g.cands[m.cell] & (1 << m.digit),
                0,
                "{t:?} marks a non-candidate"
            );
        }
        for l in &p.links {
            for (cell, digit) in [l.from, l.to] {
                assert!(marked(cell, digit) || removed(cell, digit), "{t:?}: {l:?}");
            }
        }
        for &(cell, mask) in removals {
            assert_ne!(mask, 0, "{t:?} removes nothing at {}", rc(cell));
            assert_eq!(mask & !g.cands[cell], 0, "{t:?} removes a non-candidate");
            for d in bits(mask) {
                assert!(!marked(cell, d as u8), "{t:?} removes its own mark");
            }
        }
        if let Some(d) = p.digit {
            assert!(p.marks.iter().all(|m| m.digit == d), "{t:?}");
            assert!(removals.iter().all(|&(_, mask)| mask == 1 << d), "{t:?}");
            if let Effect::Place { digit, .. } = step.effect {
                assert_eq!(digit, d, "{t:?}");
            }
        }
        assert!(p.houses.iter().all(|&h| h < HOUSES.len()), "{t:?}");
        assert!(p.cells.iter().all(|&c| g.values[c] == 0), "{t:?}");
        assert!(!step.why.is_empty(), "{t:?}");
    }

    /// Every step the solver takes is consistent.
    #[test]
    fn patterns_are_consistent() {
        dealt_states(|g, step, _| assert_consistent(g, step));
    }
}
