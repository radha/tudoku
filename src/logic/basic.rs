//! The steps inside one house, or two that overlap: singles, pointing
//! and claiming, and naked and hidden subsets.

use super::{
    Grid, Pattern, Role, Step, Technique, a_digit, bits, cell_list, digit_choice, digit_list,
    house_name, rc, subsets,
};
use crate::sudoku::{ALL_DIGITS, CELLS, HOUSES, N, box_of_cell, col_of, row_of};

impl Grid {
    /// The last empty cell of a house takes the one digit the house lacks.
    pub(super) fn full_house(&self) -> Option<Step> {
        for (h, house) in HOUSES.iter().enumerate() {
            let mut empty = house
                .iter()
                .map(|&c| usize::from(c))
                .filter(|&c| self.values[c] == 0);
            let (Some(cell), None) = (empty.next(), empty.next()) else {
                continue;
            };
            let placed = house
                .iter()
                .fold(0u16, |m, &c| m | 1 << self.values[usize::from(c)]);
            let missing = ALL_DIGITS & !placed;
            if missing.count_ones() != 1 {
                continue;
            }
            let digit = missing.trailing_zeros() as u8;
            let pattern = Pattern {
                digit: Some(digit),
                houses: vec![h],
                ..Pattern::default()
            };
            let why = format!(
                "{} is the last empty cell in {}, so it's {}.",
                rc(cell),
                house_name(h),
                a_digit(digit),
            );
            return Some(self.placing(Technique::FullHouse, cell, digit, pattern, why));
        }
        None
    }

    /// A cell with one candidate left takes it.
    pub(super) fn naked_single(&self) -> Option<Step> {
        let cell = (0..CELLS).find(|&i| self.cands[i].count_ones() == 1)?;
        let digit = self.cands[cell].trailing_zeros() as u8;
        let pattern = Pattern {
            cells: vec![cell],
            ..Pattern::default()
        };
        let why = format!("{} has only one candidate left: {digit}.", rc(cell));
        Some(self.placing(Technique::NakedSingle, cell, digit, pattern, why))
    }

    /// A digit with one spot left in a house goes there.
    pub(super) fn hidden_single(&self) -> Option<Step> {
        for (h, house) in HOUSES.iter().enumerate() {
            for digit in 1..=9u8 {
                let pos = self.positions(house, 1 << digit);
                if pos.count_ones() != 1 {
                    continue;
                }
                let cell = usize::from(house[pos.trailing_zeros() as usize]);
                let pattern = Pattern {
                    digit: Some(digit),
                    houses: vec![h],
                    ..Pattern::default()
                };
                let why = format!("In {}, {digit} fits only in {}.", house_name(h), rc(cell));
                return Some(self.placing(Technique::HiddenSingle, cell, digit, pattern, why));
            }
        }
        None
    }

    /// Pointing: a digit confined to one line within a box can't appear
    /// elsewhere on that line.
    pub(super) fn pointing(&self) -> Option<Step> {
        (2 * N..3 * N).find_map(|b| self.locked(b, Technique::Pointing))
    }

    /// Claiming: a digit confined to one box within a line can't appear
    /// elsewhere in that box.
    pub(super) fn claiming(&self) -> Option<Step> {
        (0..2 * N).find_map(|l| self.locked(l, Technique::Claiming))
    }

    /// A digit whose spots in house `h` all lie in one other house leaves
    /// the rest of that house: a box points at a line, a line claims a box.
    fn locked(&self, h: usize, technique: Technique) -> Option<Step> {
        for digit in 1..=9u8 {
            let bit = 1 << digit;
            let cells: Vec<usize> = bits(self.positions(&HOUSES[h], bit))
                .map(|p| usize::from(HOUSES[h][p]))
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
                let removals = HOUSES[t]
                    .iter()
                    .map(|&c| usize::from(c))
                    .filter(|c| !cells.contains(c))
                    .map(|c| (c, bit))
                    .collect();
                let pattern = Pattern {
                    digit: Some(digit),
                    houses: vec![h, t],
                    marks: self.marks(cells.iter().copied(), bit, Role::Key),
                    ..Pattern::default()
                };
                let why = format!(
                    "In {}, every {digit} is in {}, so the rest of {} can't be {digit}.",
                    house_name(h),
                    house_name(t),
                    house_name(t)
                );
                if let Some(step) = self.eliminating(technique, removals, pattern, why) {
                    return Some(step);
                }
            }
        }
        None
    }

    /// `k` cells in a house whose candidates together are exactly `k`
    /// digits: those digits can go nowhere else in the house.
    pub(super) fn naked_subset(&self, k: u32, technique: Technique) -> Option<Step> {
        for (h, house) in HOUSES.iter().enumerate() {
            let cell = |p: usize| usize::from(house[p]);
            let eligible = (0..N)
                .filter(|&p| (2..=k).contains(&self.cands[cell(p)].count_ones()))
                .fold(0u16, |m, p| m | 1 << p);
            for set in subsets(eligible, k) {
                let union = bits(set).fold(0, |m, p| m | self.cands[cell(p)]);
                if union.count_ones() != k {
                    continue;
                }
                let removals = (0..N)
                    .filter(|&p| set & 1 << p == 0)
                    .map(|p| (cell(p), union))
                    .collect();
                let cells: Vec<usize> = bits(set).map(cell).collect();
                let why = format!(
                    "{} hold only {} between them, so nothing else in {} can be {}.",
                    cell_list(&cells),
                    digit_list(union),
                    house_name(h),
                    digit_choice(union)
                );
                let pattern = Pattern {
                    marks: self.marks(cells.iter().copied(), union, Role::Key),
                    cells,
                    houses: vec![h],
                    ..Pattern::default()
                };
                if let Some(step) = self.eliminating(technique, removals, pattern, why) {
                    return Some(step);
                }
            }
        }
        None
    }

    /// `k` digits that, within a house, fit only in the same `k` cells:
    /// those cells can hold nothing else.
    pub(super) fn hidden_subset(&self, k: u32, technique: Technique) -> Option<Step> {
        for (h, house) in HOUSES.iter().enumerate() {
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
                let spots = bits(digits).fold(0, |m, d| m | pos[d]);
                if spots.count_ones() != k {
                    continue;
                }
                let cells: Vec<usize> = bits(spots).map(|p| usize::from(house[p])).collect();
                let removals = cells.iter().map(|&c| (c, !digits)).collect();
                let why = format!(
                    "In {}, {} fit only in {}, so those cells can't hold anything else.",
                    house_name(h),
                    digit_list(digits),
                    cell_list(&cells)
                );
                let pattern = Pattern {
                    marks: self.marks(cells.iter().copied(), digits, Role::Key),
                    cells,
                    houses: vec![h],
                    ..Pattern::default()
                };
                if let Some(step) = self.eliminating(technique, removals, pattern, why) {
                    return Some(step);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::testing::{removals, staged};
    use crate::logic::{Effect, Technique};
    use crate::sudoku::{N, idx};

    #[test]
    fn naked_single_names_its_digit() {
        let step = staged(&[(idx(2, 3), &[7])]).naked_single().unwrap();
        assert_eq!(
            step.effect,
            Effect::Place {
                cell: idx(2, 3),
                digit: 7
            }
        );
        assert_eq!(step.why, "r3c4 has only one candidate left: 7.");
    }

    #[test]
    fn hidden_single_names_its_house() {
        // 7 fits row 4 only at column 3.
        let grid = staged(&[
            (idx(4, 3), &[7, 8]),
            (idx(4, 4), &[8, 9]),
            (idx(4, 5), &[8, 9]),
        ]);
        let step = grid.hidden_single().unwrap();
        assert_eq!(
            step.effect,
            Effect::Place {
                cell: idx(4, 3),
                digit: 7
            }
        );
        assert_eq!(step.pattern.houses, [4]);
        assert_eq!(step.why, "In row 5, 7 fits only in r5c4.");
    }

    #[test]
    fn pointing_clears_the_rest_of_the_line() {
        // 5 fits box 0 only in row 0, so it leaves row 0 outside box 0.
        let grid = staged(&[
            (idx(0, 0), &[5, 6]),
            (idx(0, 1), &[5, 7]),
            (idx(1, 0), &[6, 7]),
            (idx(0, 5), &[5, 8]),
        ]);
        let step = grid.pointing();
        assert_eq!(step.as_ref().unwrap().pattern.houses, [2 * N, 0]);
        assert_eq!(
            step.as_ref().unwrap().why,
            "In box 1, every 5 is in row 1, so the rest of row 1 can't be 5."
        );
        assert_eq!(removals(step), [(idx(0, 5), 1 << 5)]);
        assert_eq!(grid.claiming(), None);
    }

    #[test]
    fn claiming_clears_the_rest_of_the_box() {
        // 5 fits row 0 only in box 0, so it leaves box 0 outside row 0.
        let grid = staged(&[
            (idx(0, 0), &[5, 6]),
            (idx(0, 1), &[5, 7]),
            (idx(1, 2), &[5, 8]),
        ]);
        let step = grid.claiming();
        assert_eq!(step.as_ref().unwrap().pattern.houses, [0, 2 * N]);
        assert_eq!(
            step.as_ref().unwrap().why,
            "In row 1, every 5 is in box 1, so the rest of box 1 can't be 5."
        );
        assert_eq!(removals(step), [(idx(1, 2), 1 << 5)]);
        assert_eq!(grid.pointing(), None);
    }

    #[test]
    fn naked_pair_clears_its_house() {
        let grid = staged(&[
            (idx(4, 0), &[2, 7]),
            (idx(4, 3), &[2, 7]),
            (idx(4, 8), &[2, 3, 7]),
        ]);
        let step = grid.naked_subset(2, Technique::NakedPair);
        assert_eq!(step.as_ref().unwrap().pattern.cells, [idx(4, 0), idx(4, 3)]);
        assert_eq!(
            step.as_ref().unwrap().why,
            "r5c1 and r5c4 hold only 2 and 7 between them, \
             so nothing else in row 5 can be 2 or 7."
        );
        assert_eq!(removals(step), [(idx(4, 8), (1 << 2) | (1 << 7))]);
    }

    #[test]
    fn hidden_pair_strips_other_candidates() {
        // In row 2, 1 and 9 only fit columns 0 and 4.
        let grid = staged(&[
            (idx(2, 0), &[1, 4, 9]),
            (idx(2, 4), &[1, 6, 9]),
            (idx(2, 7), &[4, 6]),
        ]);
        let step = grid.hidden_subset(2, Technique::HiddenPair);
        assert_eq!(
            step.as_ref().unwrap().why,
            "In row 3, 1 and 9 fit only in r3c1 and r3c5, \
             so those cells can't hold anything else."
        );
        assert_eq!(removals(step), [(idx(2, 0), 1 << 4), (idx(2, 4), 1 << 6)]);
    }
}
