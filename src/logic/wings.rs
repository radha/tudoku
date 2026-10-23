//! Wings: a pivot cell and two pincers that between them pin down one
//! digit.

use super::{
    Grid, Link, LinkKind, Pattern, Role, Step, Technique, cell_list, digit_choice, rc, sees,
};
use crate::sudoku::{CELLS, peers};

impl Grid {
    /// Pivot {x,y} sees pincers {x,z} and {y,z}: whichever value the pivot
    /// takes, one pincer is z, so cells seeing both pincers can't be z.
    pub(super) fn xy_wing(&self) -> Option<Step> {
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
                    let targets: Vec<usize> = peers(a)
                        .filter(|&c| sees(c, b) && self.cands[c] & z != 0)
                        .collect();
                    if targets.is_empty() {
                        continue;
                    }
                    // Take the pincers in the order of the digit each shares.
                    let (a, b) = if ca & pc < cb & pc { (a, b) } else { (b, a) };
                    let [x, y, z] =
                        [self.cands[a] & pc, self.cands[b] & pc, z].map(u16::trailing_zeros);
                    let why = format!(
                        "{} is {x} or {y}. If {x}, {} is {z}; if {y}, {} is {z}. \
                         Either way one is {z}, so {} can't be {z}.",
                        rc(pivot),
                        rc(a),
                        rc(b),
                        cell_list(&targets)
                    );
                    if let Some(step) = self.wing(Technique::XYWing, pivot, [a, b], z, targets, why)
                    {
                        return Some(step);
                    }
                }
            }
        }
        None
    }

    /// Pivot {x,y,z} sees pincers {x,z} and {y,z}: one of the three is z,
    /// so cells seeing all three can't be z.
    pub(super) fn xyz_wing(&self) -> Option<Step> {
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
                    let targets: Vec<usize> = peers(pivot)
                        .filter(|&c| sees(c, a) && sees(c, b) && self.cands[c] & z != 0)
                        .collect();
                    if targets.is_empty() {
                        continue;
                    }
                    // Take the pincers in the order of the digit each shares.
                    let (a, b) = if ca & !z < cb & !z { (a, b) } else { (b, a) };
                    let [x, y, z] =
                        [self.cands[a] & !z, self.cands[b] & !z, z].map(u16::trailing_zeros);
                    let why = format!(
                        "{} is {}. If {x}, {} is {z}; if {y}, {} is {z}; if {z}, {} itself is. \
                         Either way one of the three is {z}, so {} can't be {z}.",
                        rc(pivot),
                        digit_choice(pc),
                        rc(a),
                        rc(b),
                        rc(pivot),
                        cell_list(&targets)
                    );
                    if let Some(step) =
                        self.wing(Technique::XYZWing, pivot, [a, b], z, targets, why)
                    {
                        return Some(step);
                    }
                }
            }
        }
        None
    }

    /// The step for a wing on digit `z`: z is a key mark wherever the
    /// three cells hold it, their other candidates support it, and the
    /// pivot links to each pincer through the digit they share besides z.
    fn wing(
        &self,
        technique: Technique,
        pivot: usize,
        pincers: [usize; 2],
        z: u32,
        targets: Vec<usize>,
        why: String,
    ) -> Option<Step> {
        let zbit = 1 << z;
        let cells = vec![pivot, pincers[0], pincers[1]];
        let mut marks = self.marks(cells.iter().copied(), zbit, Role::Key);
        marks.extend(self.marks(cells.iter().copied(), !zbit, Role::Other));
        let links = pincers
            .map(|p| {
                let d = (self.cands[p] & !zbit).trailing_zeros() as u8;
                Link {
                    from: (pivot, d),
                    to: (p, d),
                    kind: LinkKind::Weak,
                }
            })
            .to_vec();
        let pattern = Pattern {
            cells,
            marks,
            links,
            ..Pattern::default()
        };
        let removals = targets.into_iter().map(|c| (c, zbit)).collect();
        self.eliminating(technique, removals, pattern, why)
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::testing::{removals, staged};
    use crate::logic::{Link, LinkKind, Role};
    use crate::sudoku::idx;

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
        let step = grid.xy_wing();
        let pattern = &step.as_ref().unwrap().pattern;
        assert_eq!(pattern.cells, [idx(0, 0), idx(0, 5), idx(4, 0)]);
        let weak = |from, to| Link {
            from,
            to,
            kind: LinkKind::Weak,
        };
        assert_eq!(
            pattern.links,
            [
                weak((idx(0, 0), 1), (idx(0, 5), 1)),
                weak((idx(0, 0), 2), (idx(4, 0), 2))
            ]
        );
        let keys: Vec<_> = pattern
            .marks
            .iter()
            .filter(|m| m.role == Role::Key)
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|m| m.digit == 3));
        assert_eq!(
            step.as_ref().unwrap().why,
            "r1c1 is 1 or 2. If 1, r1c6 is 3; if 2, r5c1 is 3. \
             Either way one is 3, so r5c6 can't be 3."
        );
        assert_eq!(removals(step), [(idx(4, 5), 1 << 3)]);
    }

    #[test]
    fn xy_wing_reads_the_pivot_digits_in_order() {
        // As above with the pincers swapped: the one sharing 1 comes first.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 5), &[2, 3]),
            (idx(4, 0), &[1, 3]),
            (idx(4, 5), &[3, 7]),
        ]);
        let step = grid.xy_wing().unwrap();
        assert_eq!(step.pattern.cells, [idx(0, 0), idx(4, 0), idx(0, 5)]);
        assert_eq!(
            step.why,
            "r1c1 is 1 or 2. If 1, r5c1 is 3; if 2, r1c6 is 3. \
             Either way one is 3, so r5c6 can't be 3."
        );
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
        let step = grid.xyz_wing();
        let pattern = &step.as_ref().unwrap().pattern;
        assert_eq!(pattern.cells, [idx(0, 1), idx(0, 7), idx(1, 0)]);
        assert_eq!(pattern.links.len(), 2);
        assert_eq!(
            pattern.marks.iter().filter(|m| m.role == Role::Key).count(),
            3
        );
        assert_eq!(
            step.as_ref().unwrap().why,
            "r1c2 is 1, 2 or 3. If 1, r1c8 is 3; if 2, r2c1 is 3; if 3, r1c2 itself is. \
             Either way one of the three is 3, so r1c1 and r1c3 can't be 3."
        );
        assert_eq!(removals(step), [(idx(0, 0), 1 << 3), (idx(0, 2), 1 << 3)]);
    }
}
