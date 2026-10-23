//! Chains: candidates joined by strong and weak links.
//!
//! A strong link joins two candidates of which at least one is true: a
//! house's only two spots for a digit, or a cell's only two digits. A weak
//! link joins two that can't both be true: one digit in two cells that see
//! each other, or two digits in one cell. A chain alternates strong, weak,
//! strong... and starts and ends strong. If its first candidate is false,
//! each link in turn forces the next one true or false, and the last one
//! comes out true: so at least one end is true.

use super::{
    Cells, Grid, HOUSE_CELLS, Link, LinkKind, Mark, Pattern, Role, SEEN, Step, Technique, are,
    cell_list, cells_in, digit_list, rc, sees,
};
use crate::sudoku::{CELLS, N, box_of_cell, col_of, row_of};

/// The most links a chain may have. A chain starts and ends strong, so
/// its length is odd: 3, 5, ... 11.
const MAX_LINKS: usize = 12;

/// A candidate, numbered `cell * 9 + digit - 1`.
type Node = usize;

const NODES: usize = CELLS * N;

fn node(cell: usize, digit: u8) -> Node {
    cell * N + usize::from(digit) - 1
}

fn cell_of(n: Node) -> usize {
    n / N
}

fn digit_of(n: Node) -> u8 {
    (n % N) as u8 + 1
}

/// Which links a kind of chain may use. Weak links between cells on one
/// digit are always allowed.
#[derive(Clone, Copy)]
struct Rules {
    /// Strong links between a house's only two spots for a digit.
    conjugates: bool,
    /// Strong links between a cell's only two digits.
    bivalue: bool,
    /// Weak links between two digits of one cell.
    in_cell: bool,
    /// Only the candidates of cells with two digits take part.
    bivalue_only: bool,
}

/// One digit: pairs in houses, and cells that see each other.
const X_CHAIN: Rules = Rules {
    conjugates: true,
    bivalue: false,
    in_cell: false,
    bivalue_only: false,
};

/// Cells with two digits: in on one, out on the other.
const XY_CHAIN: Rules = Rules {
    conjugates: false,
    bivalue: true,
    in_cell: false,
    bivalue_only: true,
};

const AIC: Rules = Rules {
    conjugates: true,
    bivalue: true,
    in_cell: true,
    bivalue_only: false,
};

/// The candidates a chain may run through, and the links between them.
struct Graph<'a> {
    grid: &'a Grid,
    rules: Rules,
    /// The candidates taking part, per cell.
    cands: [u16; CELLS],
    /// For each digit, the cells where it takes part.
    spots: [Cells; 10],
    /// For each digit, the cells where it is a candidate at all: what a
    /// chain can remove.
    all_spots: [Cells; 10],
}

/// A chain found: its candidates from end to end, and what it removes.
struct Found {
    nodes: Vec<Node>,
    removals: Vec<(usize, u16)>,
}

impl<'a> Graph<'a> {
    fn new(grid: &'a Grid, rules: Rules) -> Graph<'a> {
        let cands: [u16; CELLS] = std::array::from_fn(|c| {
            let m = grid.cands[c];
            if rules.bivalue_only && m.count_ones() != 2 {
                0
            } else {
                m
            }
        });
        let spread = |cands: &[u16; CELLS]| {
            let mut spots = [0; 10];
            for (c, &m) in cands.iter().enumerate() {
                for (d, s) in spots.iter_mut().enumerate() {
                    if m & (1 << d) != 0 {
                        *s |= 1 << c;
                    }
                }
            }
            spots
        };
        Graph {
            grid,
            rules,
            spots: spread(&cands),
            all_spots: spread(&grid.cands),
            cands,
        }
    }

    /// The candidates `n` has a strong link to. A house can hold the same
    /// pair as a row and a box, so one may come twice.
    fn strong(&self, n: Node, out: &mut Vec<Node>) {
        out.clear();
        let (c, d) = (cell_of(n), digit_of(n));
        if self.rules.conjugates {
            for h in [row_of(c), N + col_of(c), 2 * N + box_of_cell(c)] {
                let spots = HOUSE_CELLS[h] & self.spots[usize::from(d)];
                if spots.count_ones() == 2 {
                    out.push(node((spots & !(1 << c)).trailing_zeros() as usize, d));
                }
            }
        }
        let m = self.cands[c];
        if self.rules.bivalue && m.count_ones() == 2 {
            out.push(node(c, (m & !(1 << d)).trailing_zeros() as u8));
        }
    }

    /// The shortest chain that removes something, trying every candidate
    /// as its first end. Ties go to the first found.
    fn shortest(&self) -> Option<Found> {
        // A chain grows from its first end one link at a time. A state is
        // a candidate and whether the next link must be strong (0) or
        // weak (1); `seen` marks the states reached, per digit, and
        // `parent` the candidate each was reached from.
        let mut parent = [[0u16; NODES]; 2];
        let mut queue: Vec<(Node, usize)> = Vec::with_capacity(2 * NODES);
        let mut strong = Vec::with_capacity(4);
        let mut best: Option<Found> = None;
        let mut limit = MAX_LINKS;
        for start in (0..CELLS).flat_map(|c| {
            (1..=9u8)
                .filter(move |&d| self.cands[c] & (1 << d) != 0)
                .map(move |d| node(c, d))
        }) {
            if limit < 3 {
                break;
            }
            self.strong(start, &mut strong);
            if strong.is_empty() {
                continue;
            }
            let mut seen = [[0 as Cells; 10]; 2];
            let (sc, sd) = (cell_of(start), usize::from(digit_of(start)));
            seen[0][sd] |= 1 << sc;
            seen[1][sd] |= 1 << sc;
            queue.clear();
            queue.push((start, 0));
            let mut layer = 0..1;
            // Only a chain shorter than the best so far will do.
            let bound = limit;
            // `links` is the length of the paths in `layer`.
            'search: for links in 0..bound {
                for at in layer.clone() {
                    let (n, next) = queue[at];
                    let (c, d) = (cell_of(n), digit_of(n));
                    if next == 0 {
                        // A strong link ends a chain of `links + 1`.
                        self.strong(n, &mut strong);
                        for &m in &strong {
                            let (mc, md) = (cell_of(m), usize::from(digit_of(m)));
                            if seen[1][md] & (1 << mc) != 0 {
                                continue;
                            }
                            seen[1][md] |= 1 << mc;
                            parent[1][m] = n as u16;
                            queue.push((m, 1));
                            if links + 1 >= 3
                                && let Some(found) = self.chain(start, m, links + 1, &parent)
                            {
                                limit = links + 1 - 2;
                                best = Some(found);
                                break 'search;
                            }
                        }
                    } else if links + 2 <= bound {
                        // A weak link, which a strong one must then follow.
                        let mut to = |m: Node, seen: &mut [[Cells; 10]; 2]| {
                            let (mc, md) = (cell_of(m), usize::from(digit_of(m)));
                            if seen[0][md] & (1 << mc) == 0 {
                                seen[0][md] |= 1 << mc;
                                parent[0][m] = n as u16;
                                queue.push((m, 0));
                            }
                        };
                        let du = usize::from(d);
                        for p in cells_in(SEEN[c] & self.spots[du] & !seen[0][du]) {
                            to(node(p, d), &mut seen);
                        }
                        if self.rules.in_cell {
                            for e in 1..=9u8 {
                                if e != d && self.cands[c] & (1 << e) != 0 {
                                    to(node(c, e), &mut seen);
                                }
                            }
                        }
                    }
                }
                layer = layer.end..queue.len();
                if layer.is_empty() {
                    break;
                }
            }
        }
        best
    }

    /// The chain from `start` to `end`, `links` long, traced back through
    /// `parent`, if it never repeats a candidate and removes something
    /// that isn't one of its own.
    fn chain(
        &self,
        start: Node,
        end: Node,
        links: usize,
        parent: &[[u16; NODES]; 2],
    ) -> Option<Found> {
        let removals = self.removals(start, end);
        if removals.is_empty() {
            return None;
        }
        let mut nodes = vec![end];
        let mut at = end;
        for k in 0..links {
            at = usize::from(parent[1 - k % 2][at]);
            nodes.push(at);
        }
        nodes.reverse();
        debug_assert_eq!(nodes[0], start);
        let repeats = (1..nodes.len()).any(|i| nodes[..i].contains(&nodes[i]));
        let hits_itself = removals.iter().any(|&(c, mask)| {
            nodes
                .iter()
                .any(|&n| cell_of(n) == c && mask & (1 << digit_of(n)) != 0)
        });
        (!repeats && !hits_itself).then_some(Found { nodes, removals })
    }

    /// What a chain whose ends are `start` and `end` removes: one of them
    /// is true. On one digit, a cell seeing both can't be it. On two
    /// digits, in two cells that see each other, neither cell can be the
    /// other's digit.
    fn removals(&self, start: Node, end: Node) -> Vec<(usize, u16)> {
        let (a, x) = (cell_of(start), digit_of(start));
        let (b, y) = (cell_of(end), digit_of(end));
        if x == y {
            let targets = SEEN[a] & SEEN[b] & self.all_spots[usize::from(x)];
            return cells_in(targets).map(|c| (c, 1 << x)).collect();
        }
        let cands = &self.grid.cands;
        let mut removals = Vec::new();
        if sees(a, b) {
            if cands[a] & (1 << y) != 0 {
                removals.push((a, 1 << y));
            }
            if cands[b] & (1 << x) != 0 {
                removals.push((b, 1 << x));
            }
        }
        removals
    }
}

impl Grid {
    /// Cells holding only the same two digits, each joined to the ones it
    /// sees: two that see each other can't match, so the digits alternate
    /// along them. A cell seeing one of each colour sees both digits.
    pub(super) fn remote_pair(&self) -> Option<Step> {
        let mut grouped = [false; CELLS];
        for first in 0..CELLS {
            let pair = self.cands[first];
            if grouped[first] || pair.count_ones() != 2 {
                continue;
            }
            // Colour the group outward from its first cell, keeping the
            // links used to reach each cell.
            let mut colour: [Option<usize>; CELLS] = [None; CELLS];
            colour[first] = Some(0);
            let mut group = vec![first];
            let mut used = Vec::new();
            let mut clash = false;
            let mut at = 0;
            while at < group.len() {
                let cell = group[at];
                at += 1;
                let want = 1 - colour[cell].unwrap_or(0);
                for other in (0..CELLS).filter(|&o| self.cands[o] == pair && sees(cell, o)) {
                    match colour[other] {
                        None => {
                            colour[other] = Some(want);
                            group.push(other);
                            used.push((cell, other));
                        }
                        Some(c) => clash |= c != want,
                    }
                }
            }
            for &c in &group {
                grouped[c] = true;
            }
            // Two cells of one colour seeing each other can't happen on a
            // puzzle with a solution, so there is nothing sound to say.
            if clash || group.len() < 4 {
                continue;
            }
            let side = |k| -> Vec<usize> {
                let mut cells: Vec<usize> = group
                    .iter()
                    .copied()
                    .filter(|&c| colour[c] == Some(k))
                    .collect();
                cells.sort_unstable();
                cells
            };
            let sides = [side(0), side(1)];
            let targets: Vec<usize> = (0..CELLS)
                .filter(|&t| {
                    self.cands[t] & pair != 0
                        && colour[t].is_none()
                        && sides.iter().all(|s| s.iter().any(|&c| sees(t, c)))
                })
                .collect();
            if targets.is_empty() {
                continue;
            }
            let mut cells = group.clone();
            cells.sort_unstable();
            let (lo, hi) = (
                pair.trailing_zeros() as u8,
                (15 - pair.leading_zeros()) as u8,
            );
            let why = format!(
                "{} fit only {}, and any two that see each other differ. So {} {} one digit \
                 and {} {} the other. {} {} one of each, so {} both {lo} and {hi} and \
                 can't be either.",
                cell_list(&cells),
                digit_list(pair),
                cell_list(&sides[0]),
                are(&sides[0]),
                cell_list(&sides[1]),
                are(&sides[1]),
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
            let mut marks = self.marks(sides[0].iter().copied(), pair, Role::Key);
            marks.extend(self.marks(sides[1].iter().copied(), pair, Role::Other));
            // Follow one digit out of each cell: the one its colour would
            // be if colour 0 were the lower digit.
            let links = used
                .iter()
                .map(|&(from, to)| {
                    let d = if colour[from] == Some(0) { lo } else { hi };
                    Link {
                        from: (from, d),
                        to: (to, d),
                        kind: LinkKind::Weak,
                    }
                })
                .collect();
            let pattern = Pattern {
                cells,
                marks,
                links,
                ..Pattern::default()
            };
            let removals = targets.iter().map(|&t| (t, pair)).collect();
            if let Some(step) = self.eliminating(Technique::RemotePair, removals, pattern, why) {
                return Some(step);
            }
        }
        None
    }

    /// The shortest chain on one digit.
    pub(super) fn x_chain(&self) -> Option<Step> {
        self.chain_step(Technique::XChain, X_CHAIN)
    }

    /// The shortest chain through cells with two digits.
    pub(super) fn xy_chain(&self) -> Option<Step> {
        self.chain_step(Technique::XYChain, XY_CHAIN)
    }

    /// The shortest chain of any links.
    pub(super) fn aic(&self) -> Option<Step> {
        self.chain_step(Technique::Aic, AIC)
    }

    fn chain_step(&self, technique: Technique, rules: Rules) -> Option<Step> {
        let Found { nodes, removals } = Graph::new(self, rules).shortest()?;
        let one_digit = technique == Technique::XChain;
        let pairs: Vec<(usize, u8)> = nodes.iter().map(|&n| (cell_of(n), digit_of(n))).collect();
        let (first, last) = (pairs[0], pairs[pairs.len() - 1]);
        let links = pairs
            .windows(2)
            .enumerate()
            .map(|(i, w)| Link {
                from: w[0],
                to: w[1],
                kind: if i % 2 == 0 {
                    LinkKind::Strong
                } else {
                    LinkKind::Weak
                },
            })
            .collect();
        let mark = |(cell, digit), role| Mark { cell, digit, role };
        let mut marks = vec![mark(first, Role::Key), mark(last, Role::Key)];
        marks.extend(
            pairs[1..pairs.len() - 1]
                .iter()
                .map(|&p| mark(p, Role::Other)),
        );
        let mut cells = Vec::new();
        if !one_digit {
            for &(c, _) in &pairs {
                if !cells.contains(&c) {
                    cells.push(c);
                }
            }
        }
        let why = format!(
            "{} {}",
            walk(&pairs, one_digit),
            conclusion(first, last, &removals)
        );
        let pattern = Pattern {
            digit: one_digit.then_some(first.1),
            cells,
            marks,
            links,
            ..Pattern::default()
        };
        self.eliminating(technique, removals, pattern, why)
    }
}

/// The chain read from its first end: "If r1c1 isn't 4, r1c6 is, so r4c6
/// isn't, so ...". On one digit the digit is named only once.
fn walk(chain: &[(usize, u8)], one_digit: bool) -> String {
    let (a, x) = chain[0];
    let steps: Vec<String> = chain
        .windows(2)
        .enumerate()
        .map(|(i, w)| {
            let ((from, _), (cell, d)) = (w[0], w[1]);
            let is = if i % 2 == 0 { "is" } else { "isn't" };
            if cell == from {
                if i % 2 == 0 {
                    format!("it's {d}")
                } else {
                    format!("it isn't {d}")
                }
            } else if one_digit {
                format!("{} {is}", rc(cell))
            } else {
                format!("{} {is} {d}", rc(cell))
            }
        })
        .collect();
    format!("If {} isn't {x}, {}.", rc(a), steps.join(", so "))
}

/// Why the chain's removals follow from one of its ends being true.
fn conclusion(first: (usize, u8), last: (usize, u8), removals: &[(usize, u16)]) -> String {
    let ((a, x), (b, y)) = (first, last);
    if x == y {
        let targets: Vec<usize> = removals.iter().map(|&(c, _)| c).collect();
        return format!(
            "So {} or {} is {x}, and {} {} both, so {} {x}.",
            rc(a),
            rc(b),
            cell_list(&targets),
            if targets.len() == 1 { "sees" } else { "see" },
            if targets.len() == 1 {
                "it can't be"
            } else {
                "they can't be"
            },
        );
    }
    // Each end is its own digit, or sees the other end being its digit.
    let cuts: Vec<String> = removals
        .iter()
        .map(|&(c, _)| {
            let (own, other, theirs) = if c == a { (x, b, y) } else { (y, a, x) };
            format!(
                "{} can't be {theirs} (it's {own}, or it sees {}'s {theirs})",
                rc(c),
                rc(other)
            )
        })
        .collect();
    format!(
        "So {} is {x} or {} is {y}. Either way {}.",
        rc(a),
        rc(b),
        cuts.join(", and ")
    )
}

#[cfg(test)]
mod tests {
    use crate::logic::testing::{dealt_states, removals, staged};
    use crate::logic::{LinkKind, Role, Step, Technique};
    use crate::sudoku::idx;

    use LinkKind::{Strong as S, Weak as W};

    /// A chain step's nodes from end to end, checking its links join up.
    fn nodes(step: &Step) -> Vec<(usize, u8)> {
        let links = &step.pattern.links;
        let mut nodes = vec![links[0].from];
        for l in links {
            assert_eq!(l.from, *nodes.last().unwrap(), "{links:?}");
            nodes.push(l.to);
        }
        nodes
    }

    fn kinds(step: &Step) -> Vec<LinkKind> {
        step.pattern.links.iter().map(|l| l.kind).collect()
    }

    /// The marks of `role`, sorted.
    fn marked(step: &Step, role: Role) -> Vec<(usize, u8)> {
        let mut marks: Vec<(usize, u8)> = step
            .pattern
            .marks
            .iter()
            .filter(|m| m.role == role)
            .map(|m| (m.cell, m.digit))
            .collect();
        marks.sort_unstable();
        marks
    }

    /// The two ends of a chain, sorted.
    fn ends(step: &Step) -> Vec<(usize, u8)> {
        let nodes = nodes(step);
        let mut ends = vec![nodes[0], *nodes.last().unwrap()];
        ends.sort_unstable();
        ends
    }

    #[test]
    fn remote_pair_clears_both_digits() {
        // Four cells holding only 2 and 7, each seeing the next: r1c1 and
        // r1c7 share row 1, r1c7 and r3c8 box 3, r3c8 and r8c8 column 8.
        // Two that see each other can't match, so r1c1 and r3c8 are one
        // digit and r1c7 and r8c8 the other. r8c1 sees r1c1 (column 1) and
        // r8c8 (row 8), one of each, so it sees a 2 and a 7.
        let pair: &[u8] = &[2, 7];
        let grid = staged(&[
            (idx(0, 0), pair),
            (idx(0, 6), pair),
            (idx(2, 7), pair),
            (idx(7, 7), pair),
            (idx(7, 0), &[2, 5, 7]),
        ]);
        let step = grid.remote_pair().unwrap();
        assert_eq!(step.technique, Technique::RemotePair);
        assert_eq!(step.pattern.digit, None);
        let both = |cells: [usize; 2]| {
            let mut marks: Vec<(usize, u8)> =
                cells.iter().flat_map(|&c| [(c, 2), (c, 7)]).collect();
            marks.sort_unstable();
            marks
        };
        assert_eq!(marked(&step, Role::Key), both([idx(0, 0), idx(2, 7)]));
        assert_eq!(marked(&step, Role::Other), both([idx(0, 6), idx(7, 7)]));
        assert_eq!(kinds(&step), [W, W, W]);
        assert!(step.why.contains("r8c1"), "{}", step.why);
        assert_eq!(removals(Some(step)), [(idx(7, 0), 1 << 2 | 1 << 7)]);
    }

    #[test]
    fn remote_pair_needs_four_cells() {
        // Three cells of the pair, and r1c4 sees r1c1 and r1c7 of
        // different colours: that's a naked pair, not a Remote Pair.
        let pair: &[u8] = &[2, 7];
        let grid = staged(&[
            (idx(0, 0), pair),
            (idx(0, 6), pair),
            (idx(2, 7), pair),
            (idx(0, 3), &[2, 5, 7]),
        ]);
        assert_eq!(grid.remote_pair(), None);
    }

    #[test]
    fn simple_colouring_beats_every_remote_pair() {
        // Once no naked pair is left, two cells of the group that see each
        // other are their house's only spots for each digit, so colouring
        // either digit traps or wraps whatever the Remote Pair clears.
        let mut seen = 0;
        dealt_states(|grid, step, _| {
            if grid.find(Technique::RemotePair).is_some() {
                seen += 1;
                assert!(step.technique <= Technique::SimpleColouring, "{step:?}");
            }
        });
        assert!(seen > 0);
    }

    #[test]
    fn x_chain_of_five_links() {
        // On 4, rows 1, 4 and 7 hold just two spots each:
        // r1c1=r1c6 -column 6- r4c6=r4c9 -column 9- r7c9=r7c3.
        // If r1c1 isn't 4, r1c6 is, r4c6 isn't, r4c9 is, r7c9 isn't and
        // r7c3 is: one end is 4. r3c3 sees r1c1 (box 1) and r7c3 (column
        // 3). The pads give every other house one or three 4s, so those
        // rows are the only pairs and no shorter chain removes anything.
        const D: u8 = 4;
        let grid = staged(&[
            (idx(0, 0), &[D]),
            (idx(0, 5), &[D]),
            (idx(3, 5), &[D]),
            (idx(3, 8), &[D]),
            (idx(6, 8), &[D]),
            (idx(6, 2), &[D]),
            (idx(2, 2), &[D]),
            (idx(2, 1), &[D]),
            (idx(2, 8), &[D]),
            (idx(4, 2), &[D]),
            (idx(8, 5), &[D]),
        ]);
        let step = grid.x_chain().unwrap();
        assert_eq!(step.technique, Technique::XChain);
        assert_eq!(step.pattern.digit, Some(D));
        assert_eq!(kinds(&step), [S, W, S, W, S]);
        assert_eq!(ends(&step), [(idx(0, 0), D), (idx(6, 2), D)]);
        assert_eq!(marked(&step, Role::Key), ends(&step));
        assert_eq!(marked(&step, Role::Other).len(), 4);
        assert_eq!(removals(Some(step)), [(idx(2, 2), 1 << D)]);
    }

    #[test]
    fn xy_chain_clears_the_end_digit() {
        // r1c1 {1,2} -2- r1c5 {2,3} -3- r5c5 {3,5} -5- r5c9 {5,1}.
        // If r1c1 isn't 1 it's 2, so r1c5 is 3, r5c5 is 5 and r5c9 is 1.
        // r5c1 sees both ends, so it can't be 1.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 4), &[2, 3]),
            (idx(4, 4), &[3, 5]),
            (idx(4, 8), &[1, 5]),
            (idx(4, 0), &[1, 8, 9]),
        ]);
        let step = grid.xy_chain().unwrap();
        assert_eq!(step.technique, Technique::XYChain);
        assert_eq!(step.pattern.digit, None);
        assert_eq!(kinds(&step), [S, W, S, W, S, W, S]);
        // Strong links inside a cell, weak ones between cells.
        for l in &step.pattern.links {
            assert_eq!(l.kind == S, l.from.0 == l.to.0, "{l:?}");
        }
        assert_eq!(ends(&step), [(idx(0, 0), 1), (idx(4, 8), 1)]);
        assert_eq!(removals(Some(step)), [(idx(4, 0), 1 << 1)]);
    }

    #[test]
    fn aic_with_mixed_links() {
        // r1c1=1 =cell= r1c1=2 -row 1- r1c4=2 =column 4= r6c4=2 -cell-
        // r6c4=7 =row 6= r6c1=7. So r1c1 is 1 or r6c1 is 7; they share
        // column 1, so r6c1 can't be 1 either way. r9c1 keeps column 1 from
        // being a pair on 1.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 3), &[2, 6, 9]),
            (idx(5, 3), &[2, 7, 8]),
            (idx(5, 0), &[1, 3, 7]),
            (idx(8, 0), &[1, 4, 5]),
        ]);
        let step = grid.aic().unwrap();
        assert_eq!(step.technique, Technique::Aic);
        assert_eq!(step.pattern.digit, None);
        assert_eq!(kinds(&step), [S, W, S, W, S]);
        let inside = |kind| {
            step.pattern
                .links
                .iter()
                .filter(|l| l.kind == kind)
                .map(|l| l.from.0 == l.to.0)
                .collect::<Vec<bool>>()
        };
        // Strong and weak links both inside a cell and between cells.
        assert!(inside(S).contains(&true) && inside(S).contains(&false));
        assert!(inside(W).contains(&true) && inside(W).contains(&false));
        assert_eq!(ends(&step), [(idx(0, 0), 1), (idx(5, 0), 7)]);
        assert_eq!(removals(Some(step)), [(idx(5, 0), 1 << 1)]);
    }

    #[test]
    fn chains_never_exceed_twelve_links() {
        let mut longest = 0;
        dealt_states(|grid, _, _| {
            for t in [Technique::XChain, Technique::XYChain, Technique::Aic] {
                if let Some(step) = grid.find(t) {
                    longest = longest.max(step.pattern.links.len());
                }
            }
        });
        assert!(longest <= 12, "a chain of {longest} links");
        assert!(longest > 3, "only the shortest chains came up");
    }
}
