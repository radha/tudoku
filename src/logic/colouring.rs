//! Colouring: chains of conjugate pairs on one digit.
//!
//! Along a chain of conjugate pairs the digit alternates: one cell has
//! it, the next doesn't, and so on. So the chain's cells split into two
//! colours, and one colour is all the digit while the other has none.

use super::{Grid, Link, LinkKind, Pattern, Role, Step, Technique, cell_list, rc, sees};
use crate::sudoku::{CELLS, HOUSES};

/// A connected chain of conjugate pairs on one digit.
struct Chain {
    /// The cells of each colour.
    colours: [Vec<usize>; 2],
    /// The pairs, as the cells they join.
    pairs: Vec<(usize, usize)>,
}

/// "is" for one cell, "are" for several.
fn are(cells: &[usize]) -> &'static str {
    if cells.len() == 1 { "is" } else { "are" }
}

impl Grid {
    /// Colour the chains of conjugate pairs on each digit. A colour that
    /// sees itself can't be the digit (wrap); a cell seeing both colours
    /// can't be it either (trap).
    pub(super) fn colouring(&self) -> Option<Step> {
        for digit in 1..=9u8 {
            for chain in self.chains(digit) {
                if let Some(step) = self
                    .wrap(digit, &chain)
                    .or_else(|| self.trap(digit, &chain))
                {
                    return Some(step);
                }
            }
        }
        None
    }

    /// The chains of three or more cells on `digit`.
    fn chains(&self, digit: u8) -> Vec<Chain> {
        let mut pairs: Vec<(usize, usize)> = (0..HOUSES.len())
            .filter_map(|h| self.conjugates(h, digit))
            .collect();
        // A pair that shares a row and a box is one pair.
        pairs.sort_unstable();
        pairs.dedup();
        let mut colour: [Option<usize>; CELLS] = [None; CELLS];
        let mut chains = Vec::new();
        for &(start, _) in &pairs {
            if colour[start].is_some() {
                continue;
            }
            colour[start] = Some(0);
            let mut cells = vec![start];
            let mut at = 0;
            let mut clash = false;
            while at < cells.len() {
                let cell = cells[at];
                at += 1;
                for &(a, b) in &pairs {
                    let other = if a == cell {
                        b
                    } else if b == cell {
                        a
                    } else {
                        continue;
                    };
                    let want = 1 - colour[cell].unwrap_or(0);
                    match colour[other] {
                        None => {
                            colour[other] = Some(want);
                            cells.push(other);
                        }
                        Some(c) => clash |= c != want,
                    }
                }
            }
            // An odd loop of pairs can't happen on a puzzle with a
            // solution, so there is nothing sound to say about one.
            if clash || cells.len() < 3 {
                continue;
            }
            let side = |k| {
                cells
                    .iter()
                    .copied()
                    .filter(|&c| colour[c] == Some(k))
                    .collect()
            };
            chains.push(Chain {
                colours: [side(0), side(1)],
                pairs: pairs
                    .iter()
                    .copied()
                    .filter(|&(a, _)| cells.contains(&a))
                    .collect(),
            });
        }
        chains
    }

    /// The pattern for a chain: its pairs as strong links, with the cells
    /// of each `shown` colour marked with its role.
    fn chain_pattern(&self, digit: u8, chain: &Chain, shown: &[(usize, Role)]) -> Pattern {
        let bit = 1u16 << digit;
        let mut marks = Vec::new();
        for &(k, role) in shown {
            marks.extend(self.marks(chain.colours[k].iter().copied(), bit, role));
        }
        let links = chain
            .pairs
            .iter()
            .map(|&(a, b)| Link {
                from: (a, digit),
                to: (b, digit),
                kind: LinkKind::Strong,
            })
            .collect();
        Pattern {
            digit: Some(digit),
            marks,
            links,
            ..Pattern::default()
        }
    }

    /// Two cells of one colour see each other, so that colour can't be
    /// the digit: if it were, they would both be it.
    fn wrap(&self, digit: u8, chain: &Chain) -> Option<Step> {
        let bit = 1u16 << digit;
        for k in 0..2 {
            let cells = &chain.colours[k];
            let clash = cells.iter().enumerate().find_map(|(i, &a)| {
                cells[i + 1..]
                    .iter()
                    .find(|&&b| sees(a, b))
                    .map(|&b| (a, b))
            });
            let Some((a, b)) = clash else { continue };
            let why = format!(
                "Along these pairs, {digit} alternates: {} {} one colour and {} {} the other, \
                 and one colour is all {digit}. {} and {} are one colour and see each other, \
                 so that colour can't be all {digit} (Simple Colouring wrap). \
                 The other colour is, and {} can't be {digit}.",
                cell_list(&chain.colours[0]),
                are(&chain.colours[0]),
                cell_list(&chain.colours[1]),
                are(&chain.colours[1]),
                rc(a),
                rc(b),
                cell_list(cells),
            );
            let pattern = self.chain_pattern(digit, chain, &[(1 - k, Role::Key)]);
            let removals = cells.iter().map(|&c| (c, bit)).collect();
            return self.eliminating(Technique::SimpleColouring, removals, pattern, why);
        }
        None
    }

    /// A cell outside the chain sees one cell of each colour. One colour
    /// is the digit, so the cell sees it either way.
    fn trap(&self, digit: u8, chain: &Chain) -> Option<Step> {
        let bit = 1u16 << digit;
        let in_chain = |c: usize| chain.colours.iter().any(|side| side.contains(&c));
        let targets: Vec<usize> = (0..CELLS)
            .filter(|&c| {
                self.cands[c] & bit != 0
                    && !in_chain(c)
                    && chain
                        .colours
                        .iter()
                        .all(|side| side.iter().any(|&s| sees(c, s)))
            })
            .collect();
        let why = format!(
            "Along these pairs, {digit} alternates: {} {} one colour and {} {} the other, \
             and one colour is all {digit}. {} {} both colours, so {} a {digit} \
             either way (Simple Colouring trap) and can't be {digit}.",
            cell_list(&chain.colours[0]),
            are(&chain.colours[0]),
            cell_list(&chain.colours[1]),
            are(&chain.colours[1]),
            cell_list(&targets),
            if targets.len() == 1 {
                "sees"
            } else {
                "each see"
            },
            if targets.len() == 1 {
                "it sees"
            } else {
                "they see"
            },
        );
        let pattern = self.chain_pattern(digit, chain, &[(0, Role::Key), (1, Role::Other)]);
        let removals = targets.into_iter().map(|c| (c, bit)).collect();
        self.eliminating(Technique::SimpleColouring, removals, pattern, why)
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::testing::{removals, staged};
    use crate::logic::{LinkKind, Role};
    use crate::sudoku::idx;

    const D: u8 = 4;

    #[test]
    fn colour_trap_clears_a_cell_seeing_both_colours() {
        // A chain of conjugate pairs: (0,0)-(0,5) in row 1, (0,5)-(4,5) in
        // column 6 and (4,5)-(4,1) in row 5. Colours alternate:
        // (0,0) and (4,5) are A, (0,5) and (4,1) are B. The cell (1,1)
        // sees (0,0) by box and (4,1) by column. Padding keeps every other
        // house from holding exactly two spots.
        let grid = staged(&[
            (idx(0, 0), &[D]),
            (idx(0, 5), &[D]),
            (idx(4, 5), &[D]),
            (idx(4, 1), &[D]),
            (idx(1, 1), &[D]),
            (idx(2, 2), &[D]),
            (idx(7, 1), &[D]),
        ]);
        let step = grid.colouring().unwrap();
        assert!(step.why.contains("trap"), "{}", step.why);
        assert!(
            step.why.contains("r1c1 and r5c6 are one colour")
                && step.why.contains("r1c6 and r5c2 are the other")
                && step.why.contains("r2c2 sees both colours, so it sees a 4"),
            "{}",
            step.why
        );
        assert_eq!(step.pattern.digit, Some(D));
        assert_eq!(step.pattern.links.len(), 3);
        assert!(
            step.pattern
                .links
                .iter()
                .all(|l| l.kind == LinkKind::Strong)
        );
        let side = |role| {
            let mut cells: Vec<usize> = step
                .pattern
                .marks
                .iter()
                .filter(|m| m.role == role)
                .map(|m| m.cell)
                .collect();
            cells.sort_unstable();
            cells
        };
        assert_eq!(side(Role::Key), vec![idx(0, 0), idx(4, 5)]);
        assert_eq!(side(Role::Other), vec![idx(0, 5), idx(4, 1)]);
        assert_eq!(removals(Some(step)), vec![(idx(1, 1), 1 << D)]);
    }

    #[test]
    fn colour_wrap_clears_the_clashing_colour() {
        // The chain (0,0)-(0,5)-(4,5)-(4,1)-(2,1): (0,0), (4,5) and (2,1)
        // are A, and (0,0) and (2,1) share box 1 (which also holds the
        // padding at (1,2), so it isn't a pair). A can't be the digit.
        let grid = staged(&[
            (idx(0, 0), &[D]),
            (idx(0, 5), &[D]),
            (idx(4, 5), &[D]),
            (idx(4, 1), &[D]),
            (idx(2, 1), &[D]),
            (idx(1, 2), &[D]),
        ]);
        let step = grid.colouring().unwrap();
        assert!(step.why.contains("wrap"), "{}", step.why);
        let marked: Vec<usize> = step.pattern.marks.iter().map(|m| m.cell).collect();
        assert!(step.pattern.marks.iter().all(|m| m.role == Role::Key));
        assert_eq!(marked.len(), 2);
        assert!(marked.contains(&idx(0, 5)) && marked.contains(&idx(4, 1)));
        assert_eq!(
            removals(Some(step)),
            vec![
                (idx(0, 0), 1 << D),
                (idx(2, 1), 1 << D),
                (idx(4, 5), 1 << D)
            ]
        );
    }
}
