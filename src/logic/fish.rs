//! Fish: X-Wing, Swordfish and Jellyfish.

use super::{Grid, Pattern, Role, Step, Technique, bits, listing, subsets};
use crate::sudoku::{HOUSES, N, idx};

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
                    let removals = bits(cover)
                        .flat_map(|c| {
                            (0..N)
                                .filter(move |&l| set & 1 << l == 0)
                                .map(move |l| (at(l, c), bit))
                        })
                        .collect();
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
}

#[cfg(test)]
mod tests {
    use crate::logic::Technique;
    use crate::logic::testing::{removals, staged};
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
}
