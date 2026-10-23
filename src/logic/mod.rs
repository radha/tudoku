//! Human-style solving: the techniques people use, easiest first.
//!
//! The generator grades every puzzle by the hardest technique it needs, so
//! a level means "how clever you have to be", not just "how many blanks".
//! The tutor uses the same solver to name the step a person could take next.
//! Every step carries the pattern behind it and a reason in plain words,
//! so it can be named, explained and drawn on the board.

mod basic;
mod chains;
mod colouring;
mod fish;
mod sets;
mod single_digit;
mod uniqueness;
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
    WWing,
    FinnedXWing,
    RemotePair,
    SimpleColouring,
    UniqueRectangle,
    BugPlusOne,
    Jellyfish,
    FinnedSwordfish,
    WXYZWing,
    FinnedJellyfish,
    XChain,
    XYChain,
    Aic,
    AlsXz,
    AlsXyWing,
    SueDeCoq,
    DeathBlossom,
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
        Technique::WWing,
        Technique::FinnedXWing,
        Technique::RemotePair,
        Technique::SimpleColouring,
        Technique::UniqueRectangle,
        Technique::BugPlusOne,
        Technique::Jellyfish,
        Technique::FinnedSwordfish,
        Technique::WXYZWing,
        Technique::FinnedJellyfish,
        Technique::XChain,
        Technique::XYChain,
        Technique::Aic,
        Technique::AlsXz,
        Technique::AlsXyWing,
        Technique::SueDeCoq,
        Technique::DeathBlossom,
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
            Technique::WWing => "W-Wing",
            Technique::FinnedXWing => "finned X-Wing",
            Technique::SimpleColouring => "Simple Colouring",
            Technique::UniqueRectangle => "Unique Rectangle",
            Technique::BugPlusOne => "BUG+1",
            Technique::Jellyfish => "Jellyfish",
            Technique::FinnedSwordfish => "finned Swordfish",
            Technique::WXYZWing => "WXYZ-Wing",
            Technique::FinnedJellyfish => "finned Jellyfish",
            Technique::RemotePair => "Remote Pair",
            Technique::XChain => "X-Chain",
            Technique::XYChain => "XY-Chain",
            Technique::Aic => "AIC",
            Technique::AlsXz => "ALS-XZ",
            Technique::AlsXyWing => "ALS-XY-Wing",
            Technique::SueDeCoq => "Sue de Coq",
            Technique::DeathBlossom => "Death Blossom",
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

/// A digit with its article: "a 4", but "an 8".
fn a_digit(digit: u8) -> String {
    let article = if digit == 8 { "an" } else { "a" };
    format!("{article} {digit}")
}

/// "is" for one cell, "are" for several.
fn are(cells: &[usize]) -> &'static str {
    if cells.len() == 1 { "is" } else { "are" }
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
    // Step from one subset of `universe` to the next larger one, so only
    // its subsets are visited rather than every number up to it.
    let mut next = Some(0u16);
    std::iter::from_fn(move || {
        let s = next?;
        let after = (s | !universe).wrapping_add(1) & universe;
        next = (after != 0).then_some(after);
        Some(s)
    })
    .filter(move |s| s.count_ones() == k)
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

/// A set of cells, one bit each.
type Cells = u128;

/// For each cell, the cells it sees.
static SEEN: [Cells; CELLS] = seen_masks();

/// For each house (rows, columns, boxes), its cells.
static HOUSE_CELLS: [Cells; 3 * N] = house_masks();

const fn seen_masks() -> [Cells; CELLS] {
    let mut masks = [0; CELLS];
    let mut a = 0;
    while a < CELLS {
        let mut b = 0;
        while b < CELLS {
            if a != b
                && (row_of(a) == row_of(b)
                    || col_of(a) == col_of(b)
                    || box_of_cell(a) == box_of_cell(b))
            {
                masks[a] |= 1 << b;
            }
            b += 1;
        }
        a += 1;
    }
    masks
}

const fn house_masks() -> [Cells; 3 * N] {
    let mut masks = [0; 3 * N];
    let mut c = 0;
    while c < CELLS {
        masks[row_of(c)] |= 1 << c;
        masks[N + col_of(c)] |= 1 << c;
        masks[2 * N + box_of_cell(c)] |= 1 << c;
        c += 1;
    }
    masks
}

/// The cells in `set`, lowest first.
fn cells_in(mut set: Cells) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        (set != 0).then(|| {
            let c = set.trailing_zeros() as usize;
            set &= set - 1;
            c
        })
    })
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
            WWing => self.w_wing(),
            FinnedXWing => self.finned_fish(2, t),
            SimpleColouring => self.colouring(),
            UniqueRectangle => self.unique_rectangle(),
            BugPlusOne => self.bug_plus_one(),
            Jellyfish => self.fish(4, t),
            FinnedSwordfish => self.finned_fish(3, t),
            WXYZWing => self.wxyz_wing(),
            FinnedJellyfish => self.finned_fish(4, t),
            RemotePair => self.remote_pair(),
            XChain => self.x_chain(),
            XYChain => self.xy_chain(),
            Aic => self.aic(),
            AlsXz => self.als_xz(),
            AlsXyWing => self.als_xy_wing(),
            SueDeCoq => self.sue_de_coq(),
            DeathBlossom => self.death_blossom(),
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
    use crate::sudoku;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

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

    /// Walk 20 seeded minimal puzzles, including ones logic alone can't
    /// finish, calling `visit` at every state with the step `next_step`
    /// takes there and the puzzle's solution.
    pub fn dealt_states(mut visit: impl FnMut(&Grid, &Step, &Board)) {
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
}

#[cfg(test)]
mod tests {
    use super::testing::{dealt_states, staged};
    use super::*;
    use crate::sudoku::{self, idx, parse_board, test_board};

    #[test]
    fn subsets_and_bits() {
        assert_eq!(
            subsets(0b1011, 2).collect::<Vec<_>>(),
            [0b0011, 0b1001, 0b1010]
        );
        assert_eq!(bits(0b1010_0001).collect::<Vec<_>>(), [0, 5, 7]);
    }

    #[test]
    fn subsets_are_every_k_subset_smallest_first() {
        for universe in 0..=ALL_DIGITS {
            for k in 0..=4 {
                let every: Vec<u16> = (0..=universe)
                    .filter(|&s| s & !universe == 0 && s.count_ones() == k)
                    .collect();
                assert_eq!(
                    subsets(universe, k).collect::<Vec<_>>(),
                    every,
                    "{universe:#b}"
                );
            }
        }
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
        assert_eq!(a_digit(4), "a 4");
        assert_eq!(a_digit(8), "an 8");
    }

    #[test]
    fn technique_list_is_complete() {
        // ALL lists every variant in declaration order with no gaps, so the
        // derived Ord (easiest first) agrees with it.
        for (i, &t) in Technique::ALL.iter().enumerate() {
            assert_eq!(t as usize, i, "{t:?}");
        }
        assert_eq!(*Technique::ALL.last().unwrap(), Technique::DeathBlossom);
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
        assert_eq!(s[0], 5);
        assert_eq!(
            step.why,
            "r1c1 is the last empty cell in row 1, so it's a 5."
        );
        // An 8 takes "an".
        let mut b = s;
        let eight = (0..CELLS).find(|&i| s[i] == 8).unwrap();
        b[eight] = 0;
        let step = Grid::new(&b).find(Technique::FullHouse).unwrap();
        assert!(step.why.ends_with("so it's an 8."), "{}", step.why);
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

    /// Every detector, run at every state of the dealt puzzles, must agree
    /// with the real solution: placements match it and eliminations never
    /// remove its digit.
    #[test]
    fn every_detector_is_sound_on_dealt_puzzles() {
        let mut found = [0; Technique::ALL.len()];
        dealt_states(|grid, _, solution| assert_sound(grid, solution, &mut found));
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
            WWing,
            FinnedXWing,
            SimpleColouring,
            UniqueRectangle,
            BugPlusOne,
            Jellyfish,
            FinnedSwordfish,
            WXYZWing,
            FinnedJellyfish,
            RemotePair,
            XChain,
            XYChain,
            Aic,
            AlsXz,
            AlsXyWing,
            SueDeCoq,
            DeathBlossom,
        ] {
            assert!(found[t as usize] > 0, "{t:?} never fired: {found:?}");
        }
    }

    /// Run every detector on `grid` and check what each finds against
    /// `solution`: placements match it and eliminations never remove its
    /// digit. Counts the steps found in `found`.
    fn assert_sound(grid: &Grid, solution: &Board, found: &mut [usize; Technique::ALL.len()]) {
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
    }

    /// The detectors stay sound on notes a player might keep, not just the
    /// solver's own candidates. The tutor reads full notes as candidates
    /// once it has checked they keep every cell's answer, so at the
    /// solver's states of seeded puzzles the notes here are any candidates
    /// the placed digits leave open that keep each answer: the solver's
    /// with some of what it cleared put back, or a random pick.
    #[test]
    fn every_detector_is_sound_on_player_notes() {
        use rand::rngs::StdRng;
        use rand::{RngExt, SeedableRng};
        let mut rng = StdRng::seed_from_u64(424242);
        let mut found = [0; Technique::ALL.len()];
        for _ in 0..10 {
            let solution = sudoku::full_solution(&mut rng);
            let puzzle = sudoku::dig(&solution, CELLS, &mut rng);
            let mut grid = Grid::new(&puzzle);
            let mut states = Vec::new();
            while let Some(step) = grid.next_step() {
                states.push(grid.clone());
                grid.apply(&step);
            }
            for state in states.iter().step_by(2) {
                for put_back in [true, false] {
                    let notes = std::array::from_fn(|i| {
                        let open = candidates_mask(&state.values, i);
                        let answer = 1 << solution[i];
                        let (sure, maybe) = if put_back {
                            (state.cands[i], open & !state.cands[i])
                        } else {
                            (answer, open & !answer)
                        };
                        bits(maybe)
                            .filter(|_| rng.random_bool(0.5))
                            .fold(sure, |m, d| m | 1 << d)
                    });
                    let noted = Grid::with_candidates(&state.values, &notes);
                    assert_sound(&noted, &solution, &mut found);
                }
            }
        }
        // Every detector but BUG+1 finds something to check. BUG+1 needs
        // all but one cell down to two candidates, which random notes
        // don't leave.
        for &t in Technique::ALL {
            if t != Technique::BugPlusOne {
                assert!(found[t as usize] > 0, "{t:?} never fired: {found:?}");
            }
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
