//! Human-style solving: the techniques people use, easiest first.
//!
//! The generator grades every puzzle by the hardest technique it needs, so
//! a level means "how clever you have to be", not just "how many blanks".
//! Hints use the same solver to find the cell a person could deduce next.

use crate::sudoku::{
    Board, CELLS, HOUSES, N, box_of_cell, candidates_mask, col_of, idx, peers, row_of,
};

/// Solving techniques, ordered from easiest to hardest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Technique {
    NakedSingle,
    HiddenSingle,
    LockedCandidates,
    NakedPair,
    HiddenPair,
    NakedTriple,
    HiddenTriple,
    XWing,
    XYWing,
    NakedQuad,
    HiddenQuad,
    Swordfish,
    XYZWing,
    Jellyfish,
}

impl Technique {
    pub fn name(self) -> &'static str {
        match self {
            Technique::NakedSingle => "naked single",
            Technique::HiddenSingle => "hidden single",
            Technique::LockedCandidates => "locked candidates",
            Technique::NakedPair => "naked pair",
            Technique::HiddenPair => "hidden pair",
            Technique::NakedTriple => "naked triple",
            Technique::HiddenTriple => "hidden triple",
            Technique::XWing => "X-Wing",
            Technique::XYWing => "XY-Wing",
            Technique::NakedQuad => "naked quad",
            Technique::HiddenQuad => "hidden quad",
            Technique::Swordfish => "Swordfish",
            Technique::XYZWing => "XYZ-Wing",
            Technique::Jellyfish => "Jellyfish",
        }
    }
}

/// One deduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// `digit` is the only possibility for `cell`.
    Place {
        cell: usize,
        digit: u8,
        technique: Technique,
    },
    /// Each `(cell, mask)` loses the candidates in `mask`.
    Eliminate {
        technique: Technique,
        removals: Vec<(usize, u16)>,
    },
}

impl Step {
    pub fn technique(&self) -> Technique {
        match self {
            Step::Place { technique, .. } | Step::Eliminate { technique, .. } => *technique,
        }
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

/// A board plus the pencil marks a perfect note-taker would have.
#[derive(Debug, Clone)]
pub struct Grid {
    values: Board,
    cands: [u16; CELLS],
}

impl Grid {
    pub fn new(values: &Board) -> Self {
        let mut cands = [0; CELLS];
        for i in (0..CELLS).filter(|&i| values[i] == 0) {
            cands[i] = candidates_mask(values, i);
        }
        Self {
            values: *values,
            cands,
        }
    }

    pub fn is_solved(&self) -> bool {
        !self.values.contains(&0)
    }

    /// An empty cell with no candidates left: the board has no solution.
    fn is_broken(&self) -> bool {
        (0..CELLS).any(|i| self.values[i] == 0 && self.cands[i] == 0)
    }

    pub fn apply(&mut self, step: &Step) {
        match *step {
            Step::Place { cell, digit, .. } => {
                self.values[cell] = digit;
                self.cands[cell] = 0;
                for p in peers(cell) {
                    self.cands[p] &= !(1 << digit);
                }
            }
            Step::Eliminate { ref removals, .. } => {
                for &(cell, mask) in removals {
                    self.cands[cell] &= !mask;
                }
            }
        }
    }

    /// The easiest available deduction, if any technique finds one.
    pub fn next_step(&self) -> Option<Step> {
        use Technique::*;
        if self.is_broken() {
            return None;
        }
        self.naked_single()
            .or_else(|| self.hidden_single())
            .or_else(|| self.locked_candidates())
            .or_else(|| self.naked_subset(2, NakedPair))
            .or_else(|| self.hidden_subset(2, HiddenPair))
            .or_else(|| self.naked_subset(3, NakedTriple))
            .or_else(|| self.hidden_subset(3, HiddenTriple))
            .or_else(|| self.fish(2, XWing))
            .or_else(|| self.xy_wing())
            .or_else(|| self.naked_subset(4, NakedQuad))
            .or_else(|| self.hidden_subset(4, HiddenQuad))
            .or_else(|| self.fish(3, Swordfish))
            .or_else(|| self.xyz_wing())
            .or_else(|| self.fish(4, Jellyfish))
    }

    /// Remove `mask` from each of `cells`, or `None` if nothing changes.
    fn eliminate(
        &self,
        technique: Technique,
        cells: impl IntoIterator<Item = usize>,
        mask: u16,
    ) -> Option<Step> {
        let removals: Vec<(usize, u16)> = cells
            .into_iter()
            .filter(|&c| self.cands[c] & mask != 0)
            .map(|c| (c, self.cands[c] & mask))
            .collect();
        (!removals.is_empty()).then_some(Step::Eliminate {
            technique,
            removals,
        })
    }

    /// Positions (0..9) within `house` whose cell has any of `mask`.
    fn positions(&self, house: &[u8; N], mask: u16) -> u16 {
        house
            .iter()
            .enumerate()
            .filter(|&(_, &c)| self.cands[usize::from(c)] & mask != 0)
            .fold(0, |m, (p, _)| m | 1 << p)
    }

    fn naked_single(&self) -> Option<Step> {
        let cell = (0..CELLS).find(|&i| self.cands[i].count_ones() == 1)?;
        Some(Step::Place {
            cell,
            digit: self.cands[cell].trailing_zeros() as u8,
            technique: Technique::NakedSingle,
        })
    }

    fn hidden_single(&self) -> Option<Step> {
        for house in &HOUSES {
            for digit in 1..=9u8 {
                let pos = self.positions(house, 1 << digit);
                if pos.count_ones() == 1 {
                    return Some(Step::Place {
                        cell: usize::from(house[pos.trailing_zeros() as usize]),
                        digit,
                        technique: Technique::HiddenSingle,
                    });
                }
            }
        }
        None
    }

    /// Pointing: a digit confined to one line within a box can't appear
    /// elsewhere on that line. Claiming: a digit confined to one box within
    /// a line can't appear elsewhere in that box.
    fn locked_candidates(&self) -> Option<Step> {
        for (h, house) in HOUSES.iter().enumerate() {
            for digit in 1..=9u8 {
                let bit = 1 << digit;
                let cells: Vec<usize> = bits(self.positions(house, bit))
                    .map(|p| usize::from(house[p]))
                    .collect();
                if cells.len() < 2 {
                    continue;
                }
                let same = |f: fn(usize) -> usize| cells.iter().all(|&c| f(c) == f(cells[0]));
                // The other houses all of these cells share.
                let mut targets = Vec::new();
                if h >= 2 * N {
                    if same(row_of) {
                        targets.push(row_of(cells[0]));
                    }
                    if same(col_of) {
                        targets.push(N + col_of(cells[0]));
                    }
                } else if same(box_of_cell) {
                    targets.push(2 * N + box_of_cell(cells[0]));
                }
                for t in targets {
                    let outside = HOUSES[t]
                        .iter()
                        .map(|&c| usize::from(c))
                        .filter(|c| !cells.contains(c));
                    if let Some(step) = self.eliminate(Technique::LockedCandidates, outside, bit) {
                        return Some(step);
                    }
                }
            }
        }
        None
    }

    /// `k` cells in a house whose candidates together are exactly `k`
    /// digits: those digits can go nowhere else in the house.
    fn naked_subset(&self, k: u32, technique: Technique) -> Option<Step> {
        for house in &HOUSES {
            let cell = |p: usize| usize::from(house[p]);
            let eligible = (0..N)
                .filter(|&p| (2..=k).contains(&self.cands[cell(p)].count_ones()))
                .fold(0u16, |m, p| m | 1 << p);
            for set in subsets(eligible, k) {
                let union = bits(set).fold(0, |m, p| m | self.cands[cell(p)]);
                if union.count_ones() != k {
                    continue;
                }
                let others = (0..N).filter(|&p| set & 1 << p == 0).map(cell);
                if let Some(step) = self.eliminate(technique, others, union) {
                    return Some(step);
                }
            }
        }
        None
    }

    /// `k` digits that, within a house, fit only in the same `k` cells:
    /// those cells can hold nothing else.
    fn hidden_subset(&self, k: u32, technique: Technique) -> Option<Step> {
        for house in &HOUSES {
            let pos: [u16; 10] = std::array::from_fn(|d| {
                if d == 0 {
                    0
                } else {
                    self.positions(house, 1 << d)
                }
            });
            let eligible = (1..=9)
                .filter(|&d| (2..=k).contains(&pos[d].count_ones()))
                .fold(0u16, |m, d| m | 1 << d);
            for digits in subsets(eligible, k) {
                let cells = bits(digits).fold(0, |m, d| m | pos[d]);
                if cells.count_ones() != k {
                    continue;
                }
                let removals: Vec<(usize, u16)> = bits(cells)
                    .map(|p| usize::from(house[p]))
                    .filter(|&c| self.cands[c] & !digits != 0)
                    .map(|c| (c, self.cands[c] & !digits))
                    .collect();
                if !removals.is_empty() {
                    return Some(Step::Eliminate {
                        technique,
                        removals,
                    });
                }
            }
        }
        None
    }

    /// X-Wing (k=2), Swordfish (3), Jellyfish (4): a digit whose spots in
    /// `k` rows fall in the same `k` columns must take those columns' only
    /// spots, so it leaves the rest of each column. Same with rows and
    /// columns swapped.
    fn fish(&self, k: u32, technique: Technique) -> Option<Step> {
        for digit in 1..=9u8 {
            let bit = 1 << digit;
            for base in [0, N] {
                // For row houses a position is a column, and vice versa.
                let lines: [u16; N] =
                    std::array::from_fn(|l| self.positions(&HOUSES[base + l], bit));
                let eligible = (0..N)
                    .filter(|&l| (2..=k).contains(&lines[l].count_ones()))
                    .fold(0u16, |m, l| m | 1 << l);
                for set in subsets(eligible, k) {
                    let cover = bits(set).fold(0, |m, l| m | lines[l]);
                    if cover.count_ones() != k {
                        continue;
                    }
                    let cells = bits(cover).flat_map(|c| {
                        (0..N)
                            .filter(move |&l| set & 1 << l == 0)
                            .map(move |l| if base == 0 { idx(l, c) } else { idx(c, l) })
                    });
                    if let Some(step) = self.eliminate(technique, cells, bit) {
                        return Some(step);
                    }
                }
            }
        }
        None
    }

    /// Pivot {x,y} sees pincers {x,z} and {y,z}: whichever value the pivot
    /// takes, one pincer is z, so cells seeing both pincers can't be z.
    fn xy_wing(&self) -> Option<Step> {
        for pivot in (0..CELLS).filter(|&i| self.cands[i].count_ones() == 2) {
            let pc = self.cands[pivot];
            let wings: Vec<usize> = peers(pivot)
                .filter(|&p| {
                    self.cands[p].count_ones() == 2 && (self.cands[p] & pc).count_ones() == 1
                })
                .collect();
            for (n, &a) in wings.iter().enumerate() {
                for &b in &wings[n + 1..] {
                    let (ca, cb) = (self.cands[a], self.cands[b]);
                    let z = ca & cb & !pc;
                    if z.count_ones() != 1 || ca & pc == cb & pc {
                        continue;
                    }
                    let targets = peers(a).filter(|&c| sees(c, b));
                    if let Some(step) = self.eliminate(Technique::XYWing, targets, z) {
                        return Some(step);
                    }
                }
            }
        }
        None
    }

    /// Pivot {x,y,z} sees pincers {x,z} and {y,z}: one of the three is z,
    /// so cells seeing all three can't be z.
    fn xyz_wing(&self) -> Option<Step> {
        for pivot in (0..CELLS).filter(|&i| self.cands[i].count_ones() == 3) {
            let pc = self.cands[pivot];
            let wings: Vec<usize> = peers(pivot)
                .filter(|&p| self.cands[p].count_ones() == 2 && self.cands[p] & !pc == 0)
                .collect();
            for (n, &a) in wings.iter().enumerate() {
                for &b in &wings[n + 1..] {
                    let (ca, cb) = (self.cands[a], self.cands[b]);
                    let z = ca & cb;
                    if z.count_ones() != 1 || ca | cb != pc {
                        continue;
                    }
                    let targets = peers(pivot).filter(|&c| sees(c, a) && sees(c, b));
                    if let Some(step) = self.eliminate(Technique::XYZWing, targets, z) {
                        return Some(step);
                    }
                }
            }
        }
        None
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
        hardest: Technique::NakedSingle,
        steps: 0,
    };
    while !grid.is_solved() {
        let step = grid.next_step()?;
        grade.hardest = grade.hardest.max(step.technique());
        grade.steps += 1;
        grid.apply(&step);
    }
    Some(grade)
}

/// The next cell a person could fill by logic from `values`, with its
/// digit and the hardest technique needed to get there.
pub fn next_placement(values: &Board) -> Option<(usize, u8, Technique)> {
    let mut grid = Grid::new(values);
    let mut hardest = Technique::NakedSingle;
    loop {
        let step = grid.next_step()?;
        hardest = hardest.max(step.technique());
        if let Step::Place { cell, digit, .. } = step {
            return Some((cell, digit, hardest));
        }
        grid.apply(&step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sudoku::{self, parse_board, test_board};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    /// A grid whose candidates are set by hand, to stage one pattern.
    fn staged(cands: &[(usize, &[u8])]) -> Grid {
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

    fn removals(step: Option<Step>) -> Vec<(usize, u16)> {
        match step {
            Some(Step::Eliminate { mut removals, .. }) => {
                removals.sort_unstable();
                removals
            }
            other => panic!("expected an elimination, got {other:?}"),
        }
    }

    #[test]
    fn subsets_and_bits() {
        assert_eq!(
            subsets(0b1011, 2).collect::<Vec<_>>(),
            [0b0011, 0b1001, 0b1010]
        );
        assert_eq!(bits(0b1010_0001).collect::<Vec<_>>(), [0, 5, 7]);
    }

    #[test]
    fn easy_puzzle_falls_to_singles() {
        let (p, _) = test_board();
        let g = grade(&p).unwrap();
        assert!(g.hardest <= Technique::HiddenSingle);
        assert_eq!(g.steps, 51, "one placement per empty cell");
    }

    #[test]
    fn next_placement_is_a_correct_deduction() {
        let (p, s) = test_board();
        let (cell, digit, technique) = next_placement(&p).unwrap();
        assert_eq!(p[cell], 0);
        assert_eq!(s[cell], digit);
        assert!(technique <= Technique::HiddenSingle);

        // On an XY-Wing puzzle the first placements need the wing.
        let p = parse_board(
            "800000046007000020920004005000010800035740000080090004000000060600500000200006013",
        )
        .unwrap();
        let s = sudoku::solve_one(&p).unwrap();
        let mut board = p;
        let mut hardest_seen = Technique::NakedSingle;
        while let Some((cell, digit, technique)) = next_placement(&board) {
            assert_eq!(s[cell], digit);
            hardest_seen = hardest_seen.max(technique);
            board[cell] = digit;
        }
        assert_eq!(board, s, "placements alone finish the puzzle");
        assert_eq!(hardest_seen, Technique::XYWing);
    }

    #[test]
    fn pointing_pair_clears_the_rest_of_the_row() {
        // 5 fits box 0 only in row 0, so it leaves row 0 outside box 0.
        let grid = staged(&[
            (idx(0, 0), &[5, 6]),
            (idx(0, 1), &[5, 7]),
            (idx(1, 0), &[6, 7]),
            (idx(0, 5), &[5, 8]),
        ]);
        let got = removals(grid.locked_candidates());
        assert_eq!(got, [(idx(0, 5), 1 << 5)]);
    }

    #[test]
    fn naked_pair_clears_its_house() {
        let grid = staged(&[
            (idx(4, 0), &[2, 7]),
            (idx(4, 3), &[2, 7]),
            (idx(4, 8), &[2, 3, 7]),
        ]);
        let got = removals(grid.naked_subset(2, Technique::NakedPair));
        assert_eq!(got, [(idx(4, 8), (1 << 2) | (1 << 7))]);
    }

    #[test]
    fn hidden_pair_strips_other_candidates() {
        // In row 2, 1 and 9 only fit columns 0 and 4.
        let grid = staged(&[
            (idx(2, 0), &[1, 4, 9]),
            (idx(2, 4), &[1, 6, 9]),
            (idx(2, 7), &[4, 6]),
        ]);
        let got = removals(grid.hidden_subset(2, Technique::HiddenPair));
        assert_eq!(got, [(idx(2, 0), 1 << 4), (idx(2, 4), 1 << 6)]);
    }

    #[test]
    fn x_wing_clears_its_columns() {
        // 3 sits only in columns 1 and 7 of rows 1 and 6.
        let grid = staged(&[
            (idx(1, 1), &[3, 4]),
            (idx(1, 7), &[3, 5]),
            (idx(6, 1), &[3, 6]),
            (idx(6, 7), &[3, 8]),
            (idx(4, 7), &[3, 9]),
        ]);
        let got = removals(grid.fish(2, Technique::XWing));
        assert_eq!(got, [(idx(4, 7), 1 << 3)]);
    }

    #[test]
    fn xy_wing_clears_the_shared_digit() {
        // Pivot (0,0)={1,2}; pincers (0,5)={1,3} and (4,0)={2,3}.
        // (4,5) sees both pincers, so it can't be 3.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 5), &[1, 3]),
            (idx(4, 0), &[2, 3]),
            (idx(4, 5), &[3, 7]),
        ]);
        let got = removals(grid.xy_wing());
        assert_eq!(got, [(idx(4, 5), 1 << 3)]);
    }

    #[test]
    fn xyz_wing_clears_cells_seeing_all_three() {
        // Pivot (0,1)={1,2,3}; pincers (0,7)={1,3} and (1,0)={2,3}.
        // (0,0) and (0,2) see all three, so they can't be 3.
        let grid = staged(&[
            (idx(0, 1), &[1, 2, 3]),
            (idx(0, 7), &[1, 3]),
            (idx(1, 0), &[2, 3]),
            (idx(0, 0), &[3, 5]),
            (idx(0, 2), &[3, 6]),
            (idx(0, 4), &[3, 8]),
        ]);
        let got = removals(grid.xyz_wing());
        assert_eq!(got, [(idx(0, 0), 1 << 3), (idx(0, 2), 1 << 3)]);
    }

    #[test]
    fn swordfish_clears_its_columns() {
        // 4 sits only in columns {0,3,6} across rows 0, 4 and 8.
        let grid = staged(&[
            (idx(0, 0), &[4, 1]),
            (idx(0, 3), &[4, 2]),
            (idx(4, 3), &[4, 5]),
            (idx(4, 6), &[4, 6]),
            (idx(8, 0), &[4, 7]),
            (idx(8, 6), &[4, 8]),
            (idx(2, 6), &[4, 9]),
        ]);
        assert_eq!(grid.fish(2, Technique::XWing), None);
        let got = removals(grid.fish(3, Technique::Swordfish));
        assert_eq!(got, [(idx(2, 6), 1 << 4)]);
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

    /// Every deduction on random puzzles must agree with the real solution:
    /// placements match it and eliminations never remove its digit.
    #[test]
    fn deductions_never_contradict_the_solution() {
        let mut rng = StdRng::seed_from_u64(99);
        for _ in 0..60 {
            // Minimal puzzles, including ones logic alone can't finish.
            let solution = sudoku::full_solution(&mut rng);
            let puzzle = sudoku::dig(&solution, CELLS, &mut rng);
            let mut grid = Grid::new(&puzzle);
            while let Some(step) = grid.next_step() {
                match &step {
                    Step::Place { cell, digit, .. } => assert_eq!(solution[*cell], *digit),
                    Step::Eliminate {
                        removals,
                        technique,
                    } => {
                        for &(cell, mask) in removals {
                            assert_eq!(
                                mask & (1 << solution[cell]),
                                0,
                                "{technique:?} removed the answer at {cell}"
                            );
                        }
                    }
                }
                grid.apply(&step);
            }
        }
    }
}
