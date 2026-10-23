//! Wings: a pivot cell and two pincers that between them pin down one
//! digit.

use super::{
    Grid, Link, LinkKind, Pattern, Role, Step, Technique, bits, cell_list, digit_choice,
    digit_list, rc, sees,
};
use crate::sudoku::{CELLS, HOUSES, peers};

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

    /// Two cells {x,y} that can't see each other, joined through a
    /// house where x fits in only two cells, one seeing each end: if an
    /// end is x, the far end isn't, so one end is always y.
    pub(super) fn w_wing(&self) -> Option<Step> {
        let bivalue: Vec<usize> = (0..CELLS)
            .filter(|&i| self.cands[i].count_ones() == 2)
            .collect();
        for (n, &a) in bivalue.iter().enumerate() {
            for &b in &bivalue[n + 1..] {
                if self.cands[a] != self.cands[b] || sees(a, b) {
                    continue;
                }
                let pair = self.cands[a];
                let targets: Vec<usize> = peers(a).filter(|&c| sees(c, b)).collect();
                for x in bits(pair) {
                    let y = (pair & !(1 << x)).trailing_zeros() as u8;
                    let live: Vec<usize> = targets
                        .iter()
                        .copied()
                        .filter(|&c| self.cands[c] & (1 << y) != 0)
                        .collect();
                    if live.is_empty() {
                        continue;
                    }
                    for house in 0..HOUSES.len() {
                        let Some((p, q)) = self.conjugates(house, x as u8) else {
                            continue;
                        };
                        // The bridge cell seeing `a` comes first.
                        let (p, q) = if sees(p, a) && sees(q, b) {
                            (p, q)
                        } else if sees(q, a) && sees(p, b) {
                            (q, p)
                        } else {
                            continue;
                        };
                        let x = x as u8;
                        let why = format!(
                            "{} and {} are each {x} or {y}. If {} is {x}, {} isn't, so {} is, \
                             and {} isn't. Either way one of {} and {} is {y}, \
                             so {} can't be {y}.",
                            rc(a),
                            rc(b),
                            rc(a),
                            rc(p),
                            rc(q),
                            rc(b),
                            rc(a),
                            rc(b),
                            cell_list(&live)
                        );
                        let cells = vec![a, b, p, q];
                        let mut marks = self.marks([a, b], 1 << y, Role::Key);
                        marks.extend(self.marks(cells.iter().copied(), 1 << x, Role::Other));
                        let link = |from, to, kind| Link {
                            from: (from, x),
                            to: (to, x),
                            kind,
                        };
                        let pattern = Pattern {
                            cells,
                            marks,
                            links: vec![
                                link(a, p, LinkKind::Weak),
                                link(p, q, LinkKind::Strong),
                                link(q, b, LinkKind::Weak),
                            ],
                            ..Pattern::default()
                        };
                        let removals = live.iter().map(|&c| (c, 1 << y)).collect();
                        if let Some(step) =
                            self.eliminating(Technique::WWing, removals, pattern, why)
                        {
                            return Some(step);
                        }
                    }
                }
            }
        }
        None
    }

    /// A pivot and three cells it sees, fitting only four digits between
    /// them. Every digit but one (z) has its cells all seeing each other,
    /// so it fills at most one cell each; three digits can't fill four
    /// cells, so one cell is z.
    pub(super) fn wxyz_wing(&self) -> Option<Step> {
        let small = |c: usize| (2..=4).contains(&self.cands[c].count_ones());
        for pivot in (0..CELLS).filter(|&c| small(c)) {
            let pc = self.cands[pivot];
            let wings: Vec<usize> = peers(pivot)
                .filter(|&p| small(p) && (pc | self.cands[p]).count_ones() <= 4)
                .collect();
            for (i, &a) in wings.iter().enumerate() {
                let ua = pc | self.cands[a];
                for (j, &b) in wings.iter().enumerate().skip(i + 1) {
                    let uab = ua | self.cands[b];
                    if uab.count_ones() > 4 {
                        continue;
                    }
                    for &c in &wings[j + 1..] {
                        let union = uab | self.cands[c];
                        if union.count_ones() != 4 {
                            continue;
                        }
                        let cells = [pivot, a, b, c];
                        if let Some(step) = self.wxyz(cells, union) {
                            return Some(step);
                        }
                    }
                }
            }
        }
        None
    }

    /// The step for four cells fitting exactly the digits in `union`, if
    /// one of them is the loose digit and something can be removed.
    fn wxyz(&self, cells: [usize; 4], union: u16) -> Option<Step> {
        let holders = |d: usize| -> Vec<usize> {
            cells
                .iter()
                .copied()
                .filter(|&c| self.cands[c] & (1 << d) != 0)
                .collect()
        };
        let locked = |held: &[usize]| {
            held.iter()
                .enumerate()
                .all(|(n, &p)| held[n + 1..].iter().all(|&q| sees(p, q)))
        };
        for z in bits(union) {
            let zcells = holders(z);
            if zcells.len() < 2 || !bits(union & !(1 << z)).all(|d| locked(&holders(d))) {
                continue;
            }
            let zbit = 1 << z;
            let targets: Vec<usize> = peers(zcells[0])
                .filter(|&t| {
                    self.cands[t] & zbit != 0
                        && !cells.contains(&t)
                        && zcells.iter().all(|&q| sees(t, q))
                })
                .collect();
            if targets.is_empty() {
                continue;
            }
            let why = format!(
                "{} fit only {} between them. Each digit but {z} can fill just one of them, \
                 so those three can't fill all four. One is {z}, so {} can't be {z}.",
                cell_list(&cells),
                digit_list(union),
                cell_list(&targets)
            );
            let mut marks = self.marks(cells, zbit, Role::Key);
            marks.extend(self.marks(cells, !zbit, Role::Other));
            let pattern = Pattern {
                cells: cells.to_vec(),
                marks,
                ..Pattern::default()
            };
            let removals = targets.into_iter().map(|t| (t, zbit)).collect();
            if let Some(step) = self.eliminating(Technique::WXYZWing, removals, pattern, why) {
                return Some(step);
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

    #[test]
    fn w_wing_clears_the_other_digit() {
        // Ends (0,0) and (4,4) are {1,2}. Column 5 fits 1 only at (0,5) and
        // (4,5), which see the ends through row 1 and row 5.
        // (0,4) and (4,0) see both ends, so they can't be 2.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(4, 4), &[1, 2]),
            (idx(0, 5), &[1, 5]),
            (idx(4, 5), &[1, 6]),
            (idx(0, 4), &[2, 7]),
            (idx(4, 0), &[2, 8]),
        ]);
        let step = grid.w_wing();
        let pattern = &step.as_ref().unwrap().pattern;
        assert_eq!(pattern.cells, [idx(0, 0), idx(4, 4), idx(0, 5), idx(4, 5)]);
        let link = |from, to, kind| Link {
            from: (from, 1),
            to: (to, 1),
            kind,
        };
        assert_eq!(
            pattern.links,
            [
                link(idx(0, 0), idx(0, 5), LinkKind::Weak),
                link(idx(0, 5), idx(4, 5), LinkKind::Strong),
                link(idx(4, 5), idx(4, 4), LinkKind::Weak)
            ]
        );
        let keys: Vec<_> = pattern
            .marks
            .iter()
            .filter(|m| m.role == Role::Key)
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|m| m.digit == 2));
        assert_eq!(removals(step), [(idx(0, 4), 1 << 2), (idx(4, 0), 1 << 2)]);
    }

    #[test]
    fn wxyz_wing_clears_the_loose_digit() {
        // Pivot (0,0)={1,2,4}; wings (0,5)={1,4}, (1,1)={2,4}, (0,7)={1,5}.
        // 1 is in the pivot, (0,5) and (0,7), all in row 1; 2 in the pivot
        // and (1,1), in one box; 5 only in (0,7). So 4 is the loose digit,
        // in the pivot, (0,5) and (1,1). (0,1) sees all three, so it can't
        // be 4; (8,8) doesn't, so it can.
        let grid = staged(&[
            (idx(0, 0), &[1, 2, 4]),
            (idx(0, 5), &[1, 4]),
            (idx(1, 1), &[2, 4]),
            (idx(0, 7), &[1, 5]),
            (idx(0, 1), &[4, 9]),
            (idx(8, 8), &[4, 9]),
        ]);
        let step = grid.wxyz_wing();
        let pattern = &step.as_ref().unwrap().pattern;
        let mut cells = pattern.cells.clone();
        assert_eq!(cells[0], idx(0, 0), "the pivot comes first");
        cells.sort_unstable();
        assert_eq!(cells, [idx(0, 0), idx(0, 5), idx(0, 7), idx(1, 1)]);
        let keys: Vec<_> = pattern
            .marks
            .iter()
            .filter(|m| m.role == Role::Key)
            .collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.iter().all(|m| m.digit == 4));
        assert_eq!(removals(step), [(idx(0, 1), 1 << 4)]);
    }
}
