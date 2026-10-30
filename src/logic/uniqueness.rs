//! Uniqueness: deductions that are safe only because every dealt puzzle
//! has exactly one solution. Each rules out a layout of candidates that
//! would let the solution be built two ways (or not at all), so the
//! puzzle's one solution can't be using it.

use super::{
    Grid, Pattern, Role, Step, Technique, bits, box_of_cell, cell_list, digit_choice, digit_list,
    house_name, rc, sees, subsets,
};
use crate::sudoku::{CELLS, HOUSES, N, idx, peers};

impl Grid {
    /// Four cells in two rows, two columns and two boxes, all fitting
    /// the same pair of digits. If they held only that pair, swapping
    /// the digits would give a second solution, so one of them must
    /// take something else. Types 1 to 4 are the ways that shows,
    /// and `kind` is the one to look for.
    pub(super) fn unique_rectangle(&self, kind: Technique) -> Option<Step> {
        for r1 in 0..N {
            for r2 in r1 + 1..N {
                for c1 in 0..N {
                    for c2 in c1 + 1..N {
                        // Corners in reading order: 0 1 / 2 3.
                        let corners = [idx(r1, c1), idx(r1, c2), idx(r2, c1), idx(r2, c2)];
                        if let Some(step) = self.rectangle(corners, kind) {
                            return Some(step);
                        }
                    }
                }
            }
        }
        None
    }

    fn rectangle(&self, corners: [usize; 4], kind: Technique) -> Option<Step> {
        let mut boxes = corners.map(box_of_cell);
        boxes.sort_unstable();
        let spread = 1 + boxes.windows(2).filter(|w| w[0] != w[1]).count();
        if spread != 2 || corners.iter().any(|&c| self.values[c] != 0) {
            return None;
        }
        let common = corners.iter().fold(!0, |m, &c| m & self.cands[c]);
        for x in bits(common) {
            for y in bits(common).filter(|&y| y > x) {
                let pair = 1 << x | 1 << y;
                let floor: Vec<usize> =
                    (0..4).filter(|&k| self.cands[corners[k]] == pair).collect();
                let roof: Vec<usize> = (0..4).filter(|k| !floor.contains(k)).collect();
                // Two corners side by side, not across a diagonal.
                let side_by_side = floor.len() == 2 && floor[0] ^ floor[1] != 3;
                let step = match kind {
                    Technique::UniqueRectangle1 if floor.len() == 3 => {
                        self.rectangle_type_1(corners, &floor, roof[0], pair)
                    }
                    Technique::UniqueRectangle2 if side_by_side => {
                        self.rectangle_type_2(corners, &floor, &roof, pair)
                    }
                    Technique::UniqueRectangle3 if side_by_side => {
                        self.rectangle_type_3(corners, &floor, &roof, pair)
                    }
                    Technique::UniqueRectangle4 if side_by_side => self
                        .rectangle_type_4(corners, &floor, &roof, [x, y])
                        .or_else(|| self.rectangle_type_4(corners, &floor, &roof, [y, x])),
                    _ => None,
                };
                if step.is_some() {
                    return step;
                }
            }
        }
        None
    }

    /// The pattern for a rectangle: the pair in the floor corners as
    /// supporting marks, and `key` for the digits the roof corners show.
    fn rectangle_pattern(
        &self,
        corners: [usize; 4],
        floor: &[usize],
        pair: u16,
        roof: &[usize],
        key: u16,
        houses: Vec<usize>,
    ) -> Pattern {
        let cells_of = |ks: &[usize]| -> Vec<usize> { ks.iter().map(|&k| corners[k]).collect() };
        let mut marks = self.marks(cells_of(floor), pair, Role::Other);
        marks.extend(self.marks(cells_of(roof), key, Role::Key));
        Pattern {
            cells: corners.to_vec(),
            houses,
            marks,
            ..Pattern::default()
        }
    }

    /// Three corners are just the pair: the fourth, corner `odd`, can't
    /// be either, or all four would be the pair.
    fn rectangle_type_1(
        &self,
        corners: [usize; 4],
        floor: &[usize],
        odd: usize,
        pair: u16,
    ) -> Option<Step> {
        let odd = corners[odd];
        let why = format!(
            "{} are only {}. If {} were {} too, the four cells could swap {} for a \
             second solution, but the puzzle has just one (Unique Rectangle type 1). \
             So {} can't be {}.",
            cell_list(&floor.iter().map(|&k| corners[k]).collect::<Vec<_>>()),
            digit_choice(pair),
            rc(odd),
            digit_choice(pair),
            digit_list(pair),
            rc(odd),
            digit_choice(pair),
        );
        let pattern = self.rectangle_pattern(corners, floor, pair, &[], 0, Vec::new());
        self.eliminating(Technique::UniqueRectangle1, vec![(odd, pair)], pattern, why)
    }

    /// The roof corners share one extra digit z. If neither were z, all
    /// four would be the pair, so one is z.
    fn rectangle_type_2(
        &self,
        corners: [usize; 4],
        floor: &[usize],
        roof: &[usize],
        pair: u16,
    ) -> Option<Step> {
        let [a, b] = [corners[roof[0]], corners[roof[1]]];
        let z = self.cands[a] & !pair;
        if z.count_ones() != 1 || self.cands[b] != pair | z {
            return None;
        }
        let targets: Vec<usize> = peers(a)
            .filter(|&c| sees(c, b) && self.cands[c] & z != 0)
            .collect();
        if targets.is_empty() {
            return None;
        }
        let why = format!(
            "{} are only {}. If neither of {} and {} were {}, the four cells could swap {} \
             for a second solution, but the puzzle has just one (Unique Rectangle type 2). \
             So one of them is {}, and {} can't be {}.",
            cell_list(&floor.iter().map(|&k| corners[k]).collect::<Vec<_>>()),
            digit_choice(pair),
            rc(a),
            rc(b),
            digit_list(z),
            digit_list(pair),
            digit_list(z),
            cell_list(&targets),
            digit_list(z),
        );
        let pattern = self.rectangle_pattern(corners, floor, pair, roof, pair, Vec::new());
        let removals = targets.into_iter().map(|c| (c, z)).collect();
        self.eliminating(Technique::UniqueRectangle2, removals, pattern, why)
    }

    /// The roof corners fit extra digits. One of them takes an extra, so
    /// together they act as one cell fitting those extras. With other
    /// cells of a shared house that fill as many digits as there are
    /// cells, those digits are used up there.
    fn rectangle_type_3(
        &self,
        corners: [usize; 4],
        floor: &[usize],
        roof: &[usize],
        pair: u16,
    ) -> Option<Step> {
        let [a, b] = [corners[roof[0]], corners[roof[1]]];
        let (extra_a, extra_b) = (self.cands[a] & !pair, self.cands[b] & !pair);
        if extra_a == 0 || extra_b == 0 {
            return None;
        }
        let extras = extra_a | extra_b;
        for house in (0..HOUSES.len())
            .filter(|&h| HOUSES[h].contains(&(a as u8)) && HOUSES[h].contains(&(b as u8)))
        {
            let others: Vec<usize> = HOUSES[house]
                .iter()
                .map(|&c| usize::from(c))
                .filter(|&c| !corners.contains(&c) && self.values[c] == 0)
                .collect();
            // `m` cells and the roof make m + 1, which can't fill fewer
            // digits than the extras alone.
            for m in (1..=3).filter(|&m| extras.count_ones() <= m + 1) {
                for chosen in subsets((1 << others.len()) - 1, m) {
                    let group: Vec<usize> = bits(chosen).map(|i| others[i]).collect();
                    let digits = group.iter().fold(extras, |u, &c| u | self.cands[c]);
                    if digits.count_ones() != m + 1 {
                        continue;
                    }
                    let targets: Vec<usize> = others
                        .iter()
                        .copied()
                        .filter(|&c| !group.contains(&c) && self.cands[c] & digits != 0)
                        .collect();
                    if targets.is_empty() {
                        continue;
                    }
                    let why = format!(
                        "{} are only {}. If neither of {} and {} were {}, the four cells could \
                         swap {} for a second solution, but the puzzle has just one \
                         (Unique Rectangle type 3). So one of them is {}. As one cell, with \
                         {} they make {} cells fitting only {}. No other cell in {} can be {}.",
                        cell_list(&floor.iter().map(|&k| corners[k]).collect::<Vec<_>>()),
                        digit_choice(pair),
                        rc(a),
                        rc(b),
                        digit_choice(extras),
                        digit_list(pair),
                        digit_choice(extras),
                        cell_list(&group),
                        m + 1,
                        digit_list(digits),
                        house_name(house),
                        digit_choice(digits),
                    );
                    let mut pattern =
                        self.rectangle_pattern(corners, floor, pair, &[], 0, vec![house]);
                    pattern.marks.extend(self.marks([a], extra_a, Role::Key));
                    pattern.marks.extend(self.marks([b], extra_b, Role::Key));
                    pattern
                        .marks
                        .extend(self.marks(group.iter().copied(), digits, Role::Key));
                    pattern.cells.extend(&group);
                    let removals = targets.into_iter().map(|c| (c, digits)).collect();
                    return self.eliminating(Technique::UniqueRectangle3, removals, pattern, why);
                }
            }
        }
        None
    }

    /// In a house the roof corners share, `x` fits only in them, so one
    /// is x. If either were y, the four cells could swap, so neither is.
    fn rectangle_type_4(
        &self,
        corners: [usize; 4],
        floor: &[usize],
        roof: &[usize],
        [x, y]: [usize; 2],
    ) -> Option<Step> {
        let [a, b] = [corners[roof[0]], corners[roof[1]]];
        let house = (0..HOUSES.len()).find(|&h| {
            self.conjugates(h, x as u8)
                .is_some_and(|(p, q)| (p, q) == (a, b) || (p, q) == (b, a))
        })?;
        let pair = 1 << x | 1 << y;
        let why = format!(
            "{} are only {}. In {}, {x} fits only in {} and {}, so one of them is {x}. \
             If either were {y}, the four cells could swap {} for a second solution, \
             but the puzzle has just one (Unique Rectangle type 4). So neither is {y}.",
            cell_list(&floor.iter().map(|&k| corners[k]).collect::<Vec<_>>()),
            digit_choice(pair),
            house_name(house),
            rc(a),
            rc(b),
            digit_list(pair),
        );
        let pattern = self.rectangle_pattern(corners, floor, pair, roof, 1 << x, vec![house]);
        self.eliminating(
            Technique::UniqueRectangle4,
            vec![(a, 1 << y), (b, 1 << y)],
            pattern,
            why,
        )
    }

    /// Every empty cell fits two digits except one that fits three. If
    /// that one weren't the digit that fits three times in each of its
    /// houses, every digit would fit exactly twice in every house, and
    /// such a grid has no solution or several.
    pub(super) fn bug_plus_one(&self) -> Option<Step> {
        let mut odd = None;
        for c in (0..CELLS).filter(|&c| self.values[c] == 0) {
            match self.cands[c].count_ones() {
                2 => {}
                3 if odd.is_none() => odd = Some(c),
                _ => return None,
            }
        }
        let odd = odd?;
        let mut count = [[0u8; N + 1]; HOUSES.len()];
        for (h, house) in HOUSES.iter().enumerate() {
            for &c in house {
                for d in bits(self.cands[usize::from(c)]) {
                    count[h][d] += 1;
                }
            }
        }
        // Without digit `d` in the odd cell, each digit must fit twice
        // (or not at all) in every house.
        let digit = bits(self.cands[odd]).find(|&d| {
            HOUSES.iter().enumerate().all(|(h, house)| {
                let here = house.iter().any(|&c| usize::from(c) == odd);
                (1..=N).all(|e| matches!(count[h][e] - u8::from(here && e == d), 0 | 2))
            })
        })?;
        let why = format!(
            "Every empty cell fits two digits, except {} with {}. If {} weren't {digit}, \
             each digit would fit exactly twice in every row, column and box left, \
             and such a puzzle has no solution or several. So {} is {digit}.",
            rc(odd),
            digit_list(self.cands[odd]),
            rc(odd),
            rc(odd),
        );
        let pattern = Pattern {
            cells: vec![odd],
            marks: self.marks([odd], self.cands[odd], Role::Key),
            ..Pattern::default()
        };
        Some(self.placing(Technique::BugPlusOne, odd, digit as u8, pattern, why))
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::testing::{removals, staged};
    use crate::logic::{Effect, Grid, Mark, Role, Technique};
    use crate::sudoku::{N, idx};

    fn marks(cells: &[usize], digits: &[u8], role: Role) -> Vec<Mark> {
        cells
            .iter()
            .flat_map(|&cell| digits.iter().map(move |&digit| Mark { cell, digit, role }))
            .collect()
    }

    #[test]
    fn unique_rectangle_type_1() {
        // Rows 1-2, columns 1 and 4 (boxes 1 and 2): three corners are
        // exactly {1,2}, so the fourth can't be 1 or 2.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[1, 2]),
            (idx(1, 0), &[1, 2]),
            (idx(1, 3), &[1, 2, 5]),
        ]);
        let step = grid.unique_rectangle(Technique::UniqueRectangle1);
        let s = step.as_ref().unwrap();
        assert_eq!(s.technique, Technique::UniqueRectangle1);
        assert_eq!(
            s.pattern.cells,
            [idx(0, 0), idx(0, 3), idx(1, 0), idx(1, 3)]
        );
        assert_eq!(
            s.pattern.marks,
            marks(&[idx(0, 0), idx(0, 3), idx(1, 0)], &[1, 2], Role::Other)
        );
        assert_eq!(
            s.why,
            "r1c1, r1c4 and r2c1 are only 1 or 2. If r2c4 were 1 or 2 too, the four cells \
             could swap 1 and 2 for a second solution, but the puzzle has just one \
             (Unique Rectangle type 1). So r2c4 can't be 1 or 2."
        );
        assert_eq!(removals(step), [(idx(1, 3), 1 << 1 | 1 << 2)]);
    }

    #[test]
    fn unique_rectangle_type_2() {
        // The roof (row 2) is {1,2,5} twice, so one roof cell is 5 and
        // row 2's other cells that see both can't be. (2,5) sees only one.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[1, 2]),
            (idx(1, 0), &[1, 2, 5]),
            (idx(1, 3), &[1, 2, 5]),
            (idx(1, 5), &[5, 7]),
            (idx(2, 5), &[5, 6]),
        ]);
        let step = grid.unique_rectangle(Technique::UniqueRectangle2);
        let s = step.as_ref().unwrap();
        assert_eq!(s.technique, Technique::UniqueRectangle2);
        let mut want = marks(&[idx(0, 0), idx(0, 3)], &[1, 2], Role::Other);
        want.extend(marks(&[idx(1, 0), idx(1, 3)], &[1, 2], Role::Key));
        assert_eq!(s.pattern.marks, want);
        assert_eq!(
            s.why,
            "r1c1 and r1c4 are only 1 or 2. If neither of r2c1 and r2c4 were 5, the four \
             cells could swap 1 and 2 for a second solution, but the puzzle has just one \
             (Unique Rectangle type 2). So one of them is 5, and r2c6 can't be 5."
        );
        assert_eq!(removals(step), [(idx(1, 5), 1 << 5)]);
    }

    #[test]
    fn unique_rectangle_type_3() {
        // The roof (row 2) is {1,2,5} and {1,2,7}. Row 2's r2c7 is {5,7},
        // so with the roof's extras that is two cells for two digits:
        // the rest of row 2 can't be 5 or 7.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[1, 2]),
            (idx(1, 0), &[1, 2, 5]),
            (idx(1, 3), &[1, 2, 7]),
            (idx(1, 6), &[5, 7]),
            (idx(1, 7), &[5, 8]),
            (idx(1, 8), &[3, 7]),
        ]);
        let step = grid.unique_rectangle(Technique::UniqueRectangle3);
        let s = step.as_ref().unwrap();
        assert_eq!(s.technique, Technique::UniqueRectangle3);
        assert_eq!(
            s.pattern.cells,
            [idx(0, 0), idx(0, 3), idx(1, 0), idx(1, 3), idx(1, 6)]
        );
        assert_eq!(s.pattern.houses, [1]);
        let mut want = marks(&[idx(0, 0), idx(0, 3)], &[1, 2], Role::Other);
        want.extend(marks(&[idx(1, 0)], &[5], Role::Key));
        want.extend(marks(&[idx(1, 3)], &[7], Role::Key));
        want.extend(marks(&[idx(1, 6)], &[5, 7], Role::Key));
        assert_eq!(s.pattern.marks, want);
        assert_eq!(
            s.why,
            "r1c1 and r1c4 are only 1 or 2. If neither of r2c1 and r2c4 were 5 or 7, the \
             four cells could swap 1 and 2 for a second solution, but the puzzle has just \
             one (Unique Rectangle type 3). So one of them is 5 or 7. As one cell, with \
             r2c7 they make 2 cells fitting only 5 and 7. No other cell in row 2 can be \
             5 or 7."
        );
        assert_eq!(removals(step), [(idx(1, 7), 1 << 5), (idx(1, 8), 1 << 7)]);
    }

    #[test]
    fn unique_rectangle_type_3_in_a_box() {
        // The roof (r4c1, r4c2) shares box 4 as well as row 4. Row 4 has
        // nothing else open, but in box 4 r5c3 is {5,7}: r6c3 can't be 5.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 1), &[1, 2]),
            (idx(3, 0), &[1, 2, 5]),
            (idx(3, 1), &[1, 2, 7]),
            (idx(4, 2), &[5, 7]),
            (idx(5, 2), &[5, 9]),
        ]);
        let step = grid.unique_rectangle(Technique::UniqueRectangle3);
        let s = step.as_ref().unwrap();
        assert_eq!(s.pattern.houses, [2 * N + 3]);
        assert!(
            s.why.ends_with("No other cell in box 4 can be 5 or 7."),
            "{}",
            s.why
        );
        assert_eq!(removals(step), [(idx(5, 2), 1 << 5)]);
    }

    #[test]
    fn unique_rectangle_type_3_with_a_triple() {
        // No one cell joins the roof's 5 and 7, but r2c7 {5,8} and r2c8
        // {7,8} do: with the roof, three cells for 5, 7 and 8.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[1, 2]),
            (idx(1, 0), &[1, 2, 5]),
            (idx(1, 3), &[1, 2, 7]),
            (idx(1, 5), &[5, 9]),
            (idx(1, 6), &[5, 8]),
            (idx(1, 7), &[7, 8]),
            (idx(1, 8), &[8, 9]),
        ]);
        let step = grid.unique_rectangle(Technique::UniqueRectangle3);
        let s = step.as_ref().unwrap();
        assert!(
            s.why
                .contains("with r2c7 and r2c8 they make 3 cells fitting only 5, 7 and 8"),
            "{}",
            s.why
        );
        assert_eq!(removals(step), [(idx(1, 5), 1 << 5), (idx(1, 8), 1 << 8)]);
    }

    #[test]
    fn unique_rectangle_type_3_needs_cells_that_fill_the_digits() {
        // No cell in row 2 fits only the extras: nothing is used up.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[1, 2]),
            (idx(1, 0), &[1, 2, 5]),
            (idx(1, 3), &[1, 2, 7]),
            (idx(1, 6), &[5, 8, 9]),
            (idx(1, 7), &[7, 8, 9]),
        ]);
        assert!(grid.unique_rectangle(Technique::UniqueRectangle3).is_none());
    }

    #[test]
    fn unique_rectangle_type_4() {
        // In row 2, 1 fits only in the roof cells, so one of them is 1.
        // Neither can be 2, or the four cells could swap 1 and 2.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[1, 2]),
            (idx(1, 0), &[1, 2, 6]),
            (idx(1, 3), &[1, 2, 7]),
            (idx(1, 5), &[2, 6]),
        ]);
        let step = grid.unique_rectangle(Technique::UniqueRectangle4);
        let s = step.as_ref().unwrap();
        assert_eq!(s.technique, Technique::UniqueRectangle4);
        let mut want = marks(&[idx(0, 0), idx(0, 3)], &[1, 2], Role::Other);
        want.extend(marks(&[idx(1, 0), idx(1, 3)], &[1], Role::Key));
        assert_eq!(s.pattern.marks, want);
        assert_eq!(
            s.why,
            "r1c1 and r1c4 are only 1 or 2. In row 2, 1 fits only in r2c1 and r2c4, so one \
             of them is 1. If either were 2, the four cells could swap 1 and 2 for a second \
             solution, but the puzzle has just one (Unique Rectangle type 4). \
             So neither is 2."
        );
        assert_eq!(removals(step), [(idx(1, 0), 1 << 2), (idx(1, 3), 1 << 2)]);
    }

    #[test]
    fn unique_rectangle_type_4_needs_the_digit_confined_to_the_roof() {
        // As above, but 1 also fits elsewhere in row 2.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[1, 2]),
            (idx(1, 0), &[1, 2, 6]),
            (idx(1, 3), &[1, 2, 7]),
            (idx(1, 5), &[1, 6]),
            (idx(1, 6), &[2, 6]),
        ]);
        assert!(grid.unique_rectangle(Technique::UniqueRectangle4).is_none());
    }

    #[test]
    fn unique_rectangle_needs_exactly_two_boxes() {
        // One box, and four boxes: the swap isn't a second solution.
        for cells in [
            [idx(0, 0), idx(0, 1), idx(1, 0), idx(1, 1)],
            [idx(0, 0), idx(0, 3), idx(3, 0), idx(3, 3)],
        ] {
            let mut staging: Vec<(usize, &[u8])> =
                cells.iter().map(|&c| (c, &[1, 2][..])).collect();
            staging[3].1 = &[1, 2, 5];
            let grid = staged(&staging);
            assert!(
                grid.unique_rectangle(Technique::UniqueRectangle1).is_none(),
                "{cells:?}"
            );
        }
    }

    /// Cells whose every digit fits exactly twice in each house they
    /// touch, except 9 in r1c1's row, column and box (three times).
    fn bug_grid() -> Grid {
        staged(&[
            (idx(0, 0), &[1, 2, 9]),
            (idx(0, 1), &[1, 9]),
            (idx(0, 2), &[2, 9]),
            (idx(3, 0), &[1, 9]),
            (idx(3, 1), &[1, 9]),
            (idx(6, 0), &[2, 9]),
            (idx(6, 2), &[2, 9]),
        ])
    }

    #[test]
    fn bug_plus_one_places_the_odd_digit() {
        let step = bug_grid().bug_plus_one().unwrap();
        assert_eq!(
            step.effect,
            Effect::Place {
                cell: idx(0, 0),
                digit: 9
            }
        );
        assert_eq!(step.pattern.cells, [idx(0, 0)]);
        assert_eq!(
            step.pattern.marks,
            marks(&[idx(0, 0)], &[1, 2, 9], Role::Key)
        );
        assert_eq!(
            step.why,
            "Every empty cell fits two digits, except r1c1 with 1, 2 and 9. If r1c1 \
             weren't 9, each digit would fit exactly twice in every row, column and box \
             left, and such a puzzle has no solution or several. So r1c1 is 9."
        );
    }

    #[test]
    fn bug_plus_one_needs_every_house_paired() {
        // Give r4c2 a different pair: 9 no longer pairs up in column 2.
        let mut cells = vec![
            (idx(0, 0), &[1, 2, 9][..]),
            (idx(0, 1), &[1, 9]),
            (idx(0, 2), &[2, 9]),
            (idx(3, 0), &[1, 9]),
            (idx(3, 1), &[1, 8]),
            (idx(6, 0), &[2, 9]),
            (idx(6, 2), &[2, 9]),
        ];
        assert!(staged(&cells).bug_plus_one().is_none());
        // And a second three-way cell isn't BUG+1 either.
        cells[4].1 = &[1, 2, 9];
        assert!(staged(&cells).bug_plus_one().is_none());
        assert!(bug_grid().bug_plus_one().is_some());
    }
}
