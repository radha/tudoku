//! Fish: X-Wing, Swordfish and Jellyfish, and their finned and sashimi kin.

use super::{Grid, Pattern, Role, Step, Technique, a_digit, bits, cell_list, listing, subsets};
use crate::sudoku::{HOUSES, N, box_of_cell, idx};

impl Grid {
    /// X-Wing (k=2), Swordfish (3), Jellyfish (4): a digit whose spots in
    /// `k` rows fall in the same `k` columns must take those columns' only
    /// spots, so it leaves the rest of each column. Same with rows and
    /// columns swapped.
    pub(super) fn fish(&self, k: u32, technique: Technique) -> Option<Step> {
        for digit in 1..=9u8 {
            let bit = 1 << digit;
            for base in [0, N] {
                // For row houses a position is a column, and vice versa.
                let lines: [u16; N] =
                    std::array::from_fn(|l| self.positions(&HOUSES[base + l], bit));
                let eligible = (0..N)
                    .filter(|&l| (2..=k).contains(&lines[l].count_ones()))
                    .fold(0u16, |m, l| m | 1 << l);
                // The cell where base line `l` crosses cover line `c`.
                let at = move |l: usize, c: usize| if base == 0 { idx(l, c) } else { idx(c, l) };
                for set in subsets(eligible, k) {
                    let cover = bits(set).fold(0, |m, l| m | lines[l]);
                    if cover.count_ones() != k {
                        continue;
                    }
                    let removals: Vec<(usize, u16)> = bits(cover)
                        .flat_map(|c| {
                            (0..N)
                                .filter(move |&l| set & 1 << l == 0)
                                .map(move |l| (at(l, c), bit))
                        })
                        .filter(|&(c, _)| self.cands[c] & bit != 0)
                        .collect();
                    if removals.is_empty() {
                        continue;
                    }
                    let spots = bits(set).flat_map(|l| bits(lines[l]).map(move |c| at(l, c)));
                    // Rows are houses 0..N and columns N..2N.
                    let cover_houses = N - base;
                    let pattern = Pattern {
                        digit: Some(digit),
                        houses: bits(set)
                            .map(|l| base + l)
                            .chain(bits(cover).map(|c| cover_houses + c))
                            .collect(),
                        marks: self.marks(spots, bit, Role::Key),
                        ..Pattern::default()
                    };
                    let (bases, covers) = if base == 0 {
                        ("rows", "columns")
                    } else {
                        ("columns", "rows")
                    };
                    let numbers = |m: u16| listing(bits(m).map(|l| (l + 1).to_string()), "and");
                    let why = format!(
                        "In {bases} {}, every {digit} is in {covers} {}, \
                         so the rest of those {covers} can't be {digit}.",
                        numbers(set),
                        numbers(cover)
                    );
                    if let Some(step) = self.eliminating(technique, removals, pattern, why) {
                        return Some(step);
                    }
                }
            }
        }
        None
    }

    /// Finned fish: `k` base lines and `k` cover lines like a plain fish,
    /// except a few of the base lines' spots (the fins) stick out past the
    /// cover lines, all inside one box. If no fin holds the digit it is a
    /// plain fish; if one does, it holds the digit in that box. Either way
    /// the cover lines' spots in that box, off the base lines, are out.
    /// Sashimi is the case where a base line has fewer than two spots in
    /// the cover lines, so it leans on its fin.
    pub(super) fn finned_fish(&self, k: u32, technique: Technique) -> Option<Step> {
        let covers: Vec<u16> = subsets((1 << N) - 1, k).collect();
        for digit in 1..=9u8 {
            let bit = 1 << digit;
            for base in [0, N] {
                let lines: [u16; N] =
                    std::array::from_fn(|l| self.positions(&HOUSES[base + l], bit));
                let eligible = (0..N)
                    .filter(|&l| lines[l] != 0)
                    .fold(0u16, |m, l| m | 1 << l);
                let at = move |l: usize, c: usize| if base == 0 { idx(l, c) } else { idx(c, l) };
                for set in subsets(eligible, k) {
                    let union = bits(set).fold(0, |m, l| m | lines[l]);
                    for &cover in &covers {
                        // Every cover line holds a base spot.
                        if cover & !union != 0 {
                            continue;
                        }
                        // The base spots outside the cover lines.
                        let fin_cells = || {
                            bits(set)
                                .flat_map(move |l| bits(lines[l] & !cover).map(move |c| at(l, c)))
                        };
                        let Some(first) = fin_cells().next() else {
                            continue;
                        };
                        let fin_box = box_of_cell(first);
                        if fin_cells().any(|f| box_of_cell(f) != fin_box) {
                            continue;
                        }
                        let removals: Vec<(usize, u16)> = bits(cover)
                            .flat_map(|c| {
                                (0..N)
                                    .filter(move |&l| set & 1 << l == 0)
                                    .map(move |l| at(l, c))
                            })
                            .filter(|&cell| {
                                box_of_cell(cell) == fin_box && self.cands[cell] & bit != 0
                            })
                            .map(|cell| (cell, bit))
                            .collect();
                        if removals.is_empty() {
                            continue;
                        }
                        let fins: Vec<usize> = fin_cells().collect();
                        let spots =
                            bits(set).flat_map(|l| bits(lines[l] & cover).map(move |c| at(l, c)));
                        let cover_houses = N - base;
                        let mut marks = self.marks(spots, bit, Role::Key);
                        marks.extend(self.marks(fins.iter().copied(), bit, Role::Other));
                        let pattern = Pattern {
                            digit: Some(digit),
                            houses: bits(set)
                                .map(|l| base + l)
                                .chain(bits(cover).map(|c| cover_houses + c))
                                .chain([2 * N + fin_box])
                                .collect(),
                            marks,
                            ..Pattern::default()
                        };
                        let (bases, covers_name) = if base == 0 {
                            ("rows", "columns")
                        } else {
                            ("columns", "rows")
                        };
                        let numbers = |m: u16| listing(bits(m).map(|l| (l + 1).to_string()), "and");
                        let sashimi = bits(set).any(|l| (lines[l] & cover).count_ones() < 2);
                        let (plural, not, one, s_verb) = if fins.len() == 1 {
                            ("", "isn't", "it is", "s")
                        } else {
                            ("s", "aren't", "one of them is", "")
                        };
                        let kind = if sashimi { " (a sashimi fish)" } else { "" };
                        let boxn = fin_box + 1;
                        let why = format!(
                            "Look at {bases} {} and {covers_name} {}{kind}. The fin{plural} {} \
                             stick{s_verb} out in box {boxn}. If the fin{plural} {not} {digit}, every \
                             {digit} in those {bases} is in those {covers_name}, so those \
                             {covers_name} have no room for {} outside those {bases}; if \
                             {one}, box {boxn} has its {digit} there. Either way, the other {digit}s \
                             of those {covers_name} in box {boxn} are out.",
                            numbers(set),
                            numbers(cover),
                            cell_list(&fins),
                            a_digit(digit),
                        );
                        if let Some(step) = self.eliminating(technique, removals, pattern, why) {
                            return Some(step);
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
    use crate::logic::{Role, Technique};
    use crate::sudoku::{N, idx};

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
        let step = grid.fish(2, Technique::XWing);
        let pattern = &step.as_ref().unwrap().pattern;
        assert_eq!(pattern.digit, Some(3));
        assert_eq!(pattern.houses, [1, 6, N + 1, N + 7]);
        assert_eq!(pattern.marks.len(), 4);
        assert_eq!(
            step.as_ref().unwrap().why,
            "In rows 2 and 7, every 3 is in columns 2 and 8, \
             so the rest of those columns can't be 3."
        );
        assert_eq!(removals(step), [(idx(4, 7), 1 << 3)]);
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
    fn finned_x_wing_clears_inside_the_fin_box() {
        // 3 sits in columns 1 and 7 of rows 1 and 6, plus a fin at r7c9
        // in box 9: the fin's box loses the 3 below column 8.
        let grid = staged(&[
            (idx(1, 1), &[3, 4]),
            (idx(1, 7), &[3, 5]),
            (idx(6, 1), &[3, 6]),
            (idx(6, 7), &[3, 8]),
            (idx(6, 8), &[3, 9]),
            (idx(7, 7), &[3, 2]),
        ]);
        assert_eq!(grid.fish(2, Technique::XWing), None);
        let step = grid.finned_fish(2, Technique::FinnedXWing);
        let found = step.as_ref().unwrap();
        assert_eq!(found.pattern.digit, Some(3));
        assert_eq!(found.pattern.houses, [1, 6, N + 1, N + 7, 2 * N + 8]);
        let fins: Vec<usize> = found
            .pattern
            .marks
            .iter()
            .filter(|m| m.role == Role::Other)
            .map(|m| m.cell)
            .collect();
        assert_eq!(fins, [idx(6, 8)]);
        assert!(!found.why.contains("sashimi"));
        assert_eq!(removals(step), [(idx(7, 7), 1 << 3)]);
    }

    #[test]
    fn sashimi_swordfish_says_sashimi() {
        // 5 in rows 1, 4 and 7 over columns 1, 4 and 7. Row 4 has only
        // one spot in them, plus a fin at r4c5 in box 5.
        let grid = staged(&[
            (idx(0, 0), &[5, 1]),
            (idx(0, 3), &[5, 2]),
            (idx(0, 6), &[5, 9]),
            (idx(3, 3), &[5, 3]),
            (idx(3, 4), &[5, 4]),
            (idx(6, 0), &[5, 6]),
            (idx(6, 6), &[5, 7]),
            (idx(4, 3), &[5, 8]),
        ]);
        assert_eq!(grid.finned_fish(2, Technique::FinnedXWing), None);
        let step = grid.finned_fish(3, Technique::FinnedSwordfish);
        assert!(step.as_ref().unwrap().why.contains("sashimi"));
        assert_eq!(removals(step), [(idx(4, 3), 1 << 5)]);
    }
}
