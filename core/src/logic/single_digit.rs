//! Single-digit patterns: one digit, a few strong links between its spots.
//! Skyscraper, 2-String Kite, Crane and Empty Rectangle.

use super::{
    Cells, Grid, Link, LinkKind, Pattern, Role, SEEN, Step, Technique, cell_list, cells_in,
    house_name, rc, sees,
};
use crate::sudoku::{CELLS, HOUSES, N, box_of_cell, col_of, idx, row_of};

/// Rows are houses 0..N, columns N..2N and boxes 2N..3N.
const COLS: usize = N;
const BOXES: usize = 2 * N;

impl Grid {
    /// Two parallel conjugate pairs whose bases share a line but whose
    /// tops don't: one top must be the digit.
    pub(super) fn skyscraper(&self) -> Option<Step> {
        self.chained_pairs(Technique::Skyscraper, |h1, h2, [a1, a2], [b1, b2]| {
            let rows = h1 < COLS && h2 < COLS;
            let cols = (COLS..BOXES).contains(&h1) && (COLS..BOXES).contains(&h2);
            let cross = |c: usize| if rows { col_of(c) } else { row_of(c) };
            let base = if rows { COLS + col_of(a2) } else { row_of(a2) };
            ((rows || cols) && cross(a2) == cross(b1) && cross(a1) != cross(b2))
                .then(|| (vec![h1, h2], base))
        })
    }

    /// A row pair and a column pair joined through a box.
    pub(super) fn two_string_kite(&self) -> Option<Step> {
        self.chained_pairs(Technique::TwoStringKite, |h1, h2, [_, a2], [b1, _]| {
            let (row, col) = (h1 < COLS, (COLS..BOXES).contains(&h2));
            let knot = BOXES + box_of_cell(a2);
            (row && col && box_of_cell(a2) == box_of_cell(b1)).then(|| (vec![h1, h2, knot], knot))
        })
    }

    /// A box pair and a line pair outside it, joined by a shared line.
    pub(super) fn crane(&self) -> Option<Step> {
        self.chained_pairs(Technique::Crane, |h1, h2, [_, a2], [b1, _]| {
            let b = h1.checked_sub(BOXES)?;
            let outside = if h2 < COLS {
                h2 / 3 != b / 3
            } else {
                (h2 - COLS) / 3 != b % 3
            };
            if h2 >= BOXES || !outside {
                return None;
            }
            let link = if row_of(a2) == row_of(b1) {
                row_of(a2)
            } else if col_of(a2) == col_of(b1) {
                COLS + col_of(a2)
            } else {
                return None;
            };
            Some((vec![h1, h2], link))
        })
    }

    /// Two conjugate pairs on a digit, `a1 a2` and `b1 b2`, where `a2` and
    /// `b1` see each other: they can't both hold the digit, so `a1` or
    /// `b2` does, and cells seeing both can't. `join` says whether the
    /// two houses fit `technique`, and gives the houses to tint and the
    /// house `a2` and `b1` share.
    fn chained_pairs(
        &self,
        technique: Technique,
        join: impl Fn(usize, usize, [usize; 2], [usize; 2]) -> Option<(Vec<usize>, usize)>,
    ) -> Option<Step> {
        for digit in 1..=9u8 {
            let bit = 1u16 << digit;
            let spots: Cells = (0..CELLS)
                .filter(|&c| self.cands[c] & bit != 0)
                .fold(0, |s, c| s | 1 << c);
            let pairs: Vec<(usize, [usize; 2])> = (0..HOUSES.len())
                .filter_map(|h| self.conjugates(h, digit).map(|(a, b)| (h, [a, b])))
                .collect();
            for &(h1, pa) in &pairs {
                for &(h2, pb) in &pairs {
                    for a in [pa, [pa[1], pa[0]]] {
                        for b in [pb, [pb[1], pb[0]]] {
                            let [a1, a2] = a;
                            let [b1, b2] = b;
                            let four = [a1, a2, b1, b2];
                            let distinct = (0..4).all(|i| (0..i).all(|j| four[i] != four[j]));
                            if h1 == h2 || !distinct || !sees(a2, b1) {
                                continue;
                            }
                            let Some((houses, shared)) = join(h1, h2, a, b) else {
                                continue;
                            };
                            let pattern_cells = four.iter().fold(0 as Cells, |s, &c| s | 1 << c);
                            let seeing = SEEN[a1] & SEEN[b2] & spots & !pattern_cells;
                            if seeing == 0 {
                                continue;
                            }
                            let targets: Vec<usize> = cells_in(seeing).collect();
                            let why = format!(
                                "In {}, {digit} is {} or {}; in {}, it is {} or {}. \
                                 {} and {} share {}, so they can't both be {digit}. \
                                 So {} or {} is {digit}, and {} can't be.",
                                house_name(h1),
                                rc(a1),
                                rc(a2),
                                house_name(h2),
                                rc(b1),
                                rc(b2),
                                rc(a2),
                                rc(b1),
                                house_name(shared),
                                rc(a1),
                                rc(b2),
                                cell_list(&targets)
                            );
                            let mut marks = self.marks([a1, b2], bit, Role::Key);
                            marks.extend(self.marks([a2, b1], bit, Role::Other));
                            let link = |from, to, kind| Link {
                                from: (from, digit),
                                to: (to, digit),
                                kind,
                            };
                            let pattern = Pattern {
                                digit: Some(digit),
                                houses,
                                marks,
                                links: vec![
                                    link(a1, a2, LinkKind::Strong),
                                    link(b1, b2, LinkKind::Strong),
                                    link(a2, b1, LinkKind::Weak),
                                ],
                                ..Pattern::default()
                            };
                            let removals = targets.iter().map(|&c| (c, bit)).collect();
                            if let Some(step) = self.eliminating(technique, removals, pattern, why)
                            {
                                return Some(step);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// In a box every spot for a digit lies in one row `R` and one column
    /// `C` of it. A conjugate pair on a line outside the box that touches
    /// `R` (or `C`) clears the digit from the cell where its other end's
    /// line meets `C` (or `R`).
    pub(super) fn empty_rectangle(&self) -> Option<Step> {
        for digit in 1..=9u8 {
            let bit = 1u16 << digit;
            for b in 0..N {
                let (br, bc) = (b / 3 * 3, b % 3 * 3);
                let spots: Vec<usize> = HOUSES[BOXES + b]
                    .iter()
                    .map(|&c| usize::from(c))
                    .filter(|&c| self.cands[c] & bit != 0)
                    .collect();
                if spots.len() < 2 {
                    continue;
                }
                // Case 1 reads a column pair, case 2 the same with rows and
                // columns swapped: `at(p, s)` is the cell at primary
                // coordinate `p` and secondary `s`.
                for swap in [false, true] {
                    let at = |p: usize, s: usize| if swap { idx(s, p) } else { idx(p, s) };
                    let prim = |c: usize| if swap { col_of(c) } else { row_of(c) };
                    let sec = |c: usize| if swap { row_of(c) } else { col_of(c) };
                    let (pbase, sbase) = if swap { (bc, br) } else { (br, bc) };
                    for r in pbase..pbase + 3 {
                        for c in sbase..sbase + 3 {
                            let on_cross = spots.iter().all(|&s| prim(s) == r || sec(s) == c);
                            let all_r = spots.iter().all(|&s| prim(s) == r);
                            let all_c = spots.iter().all(|&s| sec(s) == c);
                            if !on_cross || all_r || all_c {
                                continue;
                            }
                            for x in (0..N).filter(|x| !(sbase..sbase + 3).contains(x)) {
                                let line = if swap { x } else { COLS + x };
                                let Some((p, q)) = self.conjugates(line, digit) else {
                                    continue;
                                };
                                for (near, far) in [(p, q), (q, p)] {
                                    if near != at(r, x) || (pbase..pbase + 3).contains(&prim(far)) {
                                        continue;
                                    }
                                    let target = at(prim(far), c);
                                    if self.cands[target] & bit == 0 {
                                        continue;
                                    }
                                    let (rn, cn, xn) = if swap {
                                        (format!("column {}", r + 1), format!("row {}", c + 1), x)
                                    } else {
                                        (format!("row {}", r + 1), format!("column {}", c + 1), x)
                                    };
                                    let why = format!(
                                        "In box {}, every {digit} is in {rn} or {cn}. \
                                         In {} {}, {digit} is {} or {}. \
                                         If {} is {digit}, it fills {rn}, so the box's {digit} \
                                         is in {cn}, which {} sees. If {} is {digit}, {} sees \
                                         it. Either way {} can't be {digit}.",
                                        b + 1,
                                        if swap { "row" } else { "column" },
                                        xn + 1,
                                        rc(near),
                                        rc(far),
                                        rc(near),
                                        rc(target),
                                        rc(far),
                                        rc(target),
                                        rc(target),
                                    );
                                    let mut marks = self.marks([near, far], bit, Role::Key);
                                    marks.extend(self.marks(
                                        spots.iter().copied(),
                                        bit,
                                        Role::Other,
                                    ));
                                    let pattern = Pattern {
                                        digit: Some(digit),
                                        houses: vec![BOXES + b, line],
                                        marks,
                                        links: vec![Link {
                                            from: (near, digit),
                                            to: (far, digit),
                                            kind: LinkKind::Strong,
                                        }],
                                        ..Pattern::default()
                                    };
                                    let removals = vec![(target, bit)];
                                    if let Some(step) = self.eliminating(
                                        Technique::EmptyRectangle,
                                        removals,
                                        pattern,
                                        why,
                                    ) {
                                        return Some(step);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::testing::{removals, staged};
    use crate::logic::{LinkKind, Role, Technique};
    use crate::sudoku::idx;

    const D: u8 = 4;

    #[test]
    fn skyscraper_clears_cells_seeing_both_tops() {
        // Columns 1 and 5 hold a 4 only in two cells each; the bases
        // (6,1) and (6,5) share row 6. The tops (0,1) and (1,5) both see
        // (0,3).
        let grid = staged(&[
            (idx(0, 1), &[D, 5]),
            (idx(6, 1), &[D, 5]),
            (idx(1, 5), &[D, 5]),
            (idx(6, 5), &[D, 5]),
            (idx(0, 3), &[D, 9]),
            (idx(0, 7), &[D, 8]),
            (idx(6, 8), &[D, 8]),
        ]);
        let step = grid.skyscraper();
        let pattern = step.clone().unwrap().pattern;
        assert_eq!(pattern.digit, Some(D));
        assert_eq!(pattern.houses.len(), 2);
        let key = |r| pattern.marks.iter().filter(|m| m.role == r).count();
        assert_eq!((key(Role::Key), key(Role::Other)), (2, 2));
        let strong = |k| pattern.links.iter().filter(|l| l.kind == k).count();
        assert_eq!((strong(LinkKind::Strong), strong(LinkKind::Weak)), (2, 1));
        assert_eq!(step.unwrap().technique, Technique::Skyscraper);
        assert_eq!(removals(grid.skyscraper()), [(idx(0, 3), 1 << D)]);
    }

    #[test]
    fn kite_clears_the_cell_where_the_strings_meet() {
        // Row 0 holds a 4 only at (0,1) and (0,6); column 2 only at (2,2)
        // and (7,2). (0,1) and (2,2) share box 1, so (0,6) or (7,2) is 4.
        let grid = staged(&[
            (idx(0, 1), &[D, 5]),
            (idx(0, 6), &[D, 5]),
            (idx(2, 2), &[D, 5]),
            (idx(7, 2), &[D, 5]),
            (idx(7, 6), &[D, 9]),
            (idx(7, 8), &[D, 8]),
            (idx(4, 6), &[D, 8]),
        ]);
        assert_eq!(removals(grid.two_string_kite()), [(idx(7, 6), 1 << D)]);
    }

    #[test]
    fn crane_links_a_box_pair_to_a_line_pair() {
        // Box 1 holds a 4 only at (0,1) and (2,2); row 6 only at (6,2) and
        // (6,7). (2,2) and (6,2) share column 2, so (0,1) or (6,7) is 4.
        let grid = staged(&[
            (idx(0, 1), &[D, 5]),
            (idx(2, 2), &[D, 5]),
            (idx(6, 2), &[D, 5]),
            (idx(6, 7), &[D, 5]),
            (idx(0, 7), &[D, 9]),
            (idx(0, 8), &[D, 8]),
        ]);
        assert_eq!(removals(grid.crane()), [(idx(0, 7), 1 << D)]);
    }

    #[test]
    fn empty_rectangle_with_a_column_pair() {
        // In box 5 the 4s are (3,5) in row 3 and (4,4), (5,4) in column 4.
        // Column 0 holds a 4 only at (3,0) and (7,0). If (3,0) is 4, row 3
        // is used up and the box's 4 is in column 4; if (7,0), it sees (7,4).
        let grid = staged(&[
            (idx(3, 5), &[D, 5]),
            (idx(4, 4), &[D, 5]),
            (idx(5, 4), &[D, 5]),
            (idx(3, 0), &[D, 6]),
            (idx(7, 0), &[D, 6]),
            (idx(7, 4), &[D, 9]),
            (idx(7, 8), &[D, 8]),
        ]);
        assert_eq!(removals(grid.empty_rectangle()), [(idx(7, 4), 1 << D)]);
    }

    #[test]
    fn empty_rectangle_with_a_row_pair() {
        let grid = staged(&[
            (idx(5, 3), &[D, 5]),
            (idx(4, 4), &[D, 5]),
            (idx(4, 5), &[D, 5]),
            (idx(0, 3), &[D, 6]),
            (idx(0, 7), &[D, 6]),
            (idx(4, 7), &[D, 9]),
            (idx(8, 7), &[D, 8]),
        ]);
        assert_eq!(removals(grid.empty_rectangle()), [(idx(4, 7), 1 << D)]);
    }
}
