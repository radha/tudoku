//! Sets: almost locked sets, and the strategies built on them.
//!
//! An almost locked set (ALS) is 1 to 4 empty cells in one house that fit
//! one more digit than there are cells. The cells can't repeat a digit,
//! so they miss at most one of the set's digits, and if one is ruled out
//! every other one must go in the set. Two sets with no cell in common
//! are linked by a digit both fit when every cell of one that fits it
//! sees every cell of the other that fits it: then they can't both hold
//! it, so one of them is left holding all its other digits.

use super::{
    Cells, Grid, HOUSE_CELLS, Link, LinkKind, Pattern, Role, SEEN, Step, Technique, bits,
    cell_list, cells_in, digit_choice, digit_list, house_name, rc,
};
use crate::sudoku::{ALL_DIGITS, CELLS, HOUSES, N, col_of, row_of};

/// An almost locked set: `cells`, all in `house`, fitting `digits`.
struct Als {
    cells: Vec<usize>,
    digits: u16,
    house: usize,
    /// `cells` as a set.
    set: Cells,
    /// For each digit, the cells of the set that fit it.
    spots: [Cells; 10],
    /// For each digit of the set, the cells that see every cell of the
    /// set that fits it.
    seers: [Cells; 10],
}

impl Als {
    fn new(grid: &Grid, cells: &[usize], digits: u16, house: usize) -> Als {
        let mut spots = [0; 10];
        let mut seers = [0; 10];
        for d in bits(digits) {
            seers[d] = !0;
            for &c in cells.iter().filter(|&&c| grid.cands[c] & (1 << d) != 0) {
                spots[d] |= 1 << c;
                seers[d] &= SEEN[c];
            }
        }
        Als {
            cells: cells.to_vec(),
            digits,
            house,
            set: cells.iter().fold(0, |s, &c| s | 1 << c),
            spots,
            seers,
        }
    }

    /// The first cell of the set that fits `d`.
    fn first(&self, d: usize) -> usize {
        self.spots[d].trailing_zeros() as usize
    }

    /// The set as players read it: "r1c1, r1c2 (1, 2, 3)".
    fn describe(&self) -> String {
        let cells: Vec<String> = self.cells.iter().map(|&c| rc(c)).collect();
        let digits: Vec<String> = bits(self.digits).map(|d| d.to_string()).collect();
        format!("{} ({})", cells.join(", "), digits.join(", "))
    }
}

/// The digits linking `a` and `b`, which share no cell: digits both fit,
/// with every cell of one that fits it seeing every cell of the other
/// that does.
fn restricted(a: &Als, b: &Als) -> u16 {
    bits(a.digits & b.digits)
        .filter(|&d| b.spots[d] & !a.seers[d] == 0)
        .fold(0, |m, d| m | 1 << d)
}

/// The houses of the sets with more than one cell, each once.
fn houses_of(sets: &[&Als]) -> Vec<usize> {
    let mut houses: Vec<usize> = sets
        .iter()
        .filter(|s| s.cells.len() > 1)
        .map(|s| s.house)
        .collect();
    houses.sort_unstable();
    houses.dedup();
    houses
}

/// The cells of `sets`, in order, each once.
fn cells_of(sets: &[&Als]) -> Vec<usize> {
    let mut cells: Vec<usize> = sets.iter().flat_map(|s| s.cells.iter().copied()).collect();
    cells.sort_unstable();
    cells.dedup();
    cells
}

/// "r1c5 sees every 1 in them, so it can't be 1."
fn conclusion(targets: &[usize], z: usize) -> String {
    let one = targets.len() == 1;
    format!(
        "{} {} every {z} in them, so {} can't be {z}.",
        cell_list(targets),
        if one { "sees" } else { "see" },
        if one { "it" } else { "they" },
    )
}

/// The search for almost locked sets, one house at a time.
struct Search<'a> {
    grid: &'a Grid,
    max: usize,
    house: usize,
    empty: Vec<usize>,
    chosen: Vec<usize>,
    found: Vec<Als>,
}

impl Search<'_> {
    /// Add each later empty cell of the house in turn to `chosen`, which
    /// fits `digits`, keeping every almost locked set made on the way.
    fn grow(&mut self, from: usize, digits: u16) {
        for i in from..self.empty.len() {
            let cell = self.empty[i];
            let digits = digits | self.grid.cands[cell];
            // Adding cells never takes a digit away.
            if digits.count_ones() as usize > self.max + 1 {
                continue;
            }
            self.chosen.push(cell);
            let n = self.chosen.len();
            if digits.count_ones() as usize == n + 1 && self.first_house() {
                let set = Als::new(self.grid, &self.chosen, digits, self.house);
                self.found.push(set);
            }
            if n < self.max {
                self.grow(i + 1, digits);
            }
            self.chosen.pop();
        }
    }

    /// Whether `house` is the first house holding all of `chosen`, taking
    /// rows before columns and both before boxes.
    fn first_house(&self) -> bool {
        let set: Cells = self.chosen.iter().fold(0, |s, &c| s | 1 << c);
        let first = self.chosen[0];
        [row_of(first), N + col_of(first)]
            .into_iter()
            .take_while(|&h| h < self.house)
            .all(|h| set & !HOUSE_CELLS[h] != 0)
    }
}

impl Grid {
    /// Every almost locked set of at most `max` cells, each once, smallest
    /// first.
    fn almost_locked_sets(&self, max: usize) -> Vec<Als> {
        let mut search = Search {
            grid: self,
            max,
            house: 0,
            empty: Vec::with_capacity(N),
            chosen: Vec::with_capacity(max),
            found: Vec::new(),
        };
        for (house, cells) in HOUSES.iter().enumerate() {
            search.house = house;
            search.empty.clear();
            search.empty.extend(
                cells
                    .iter()
                    .map(|&c| usize::from(c))
                    .filter(|&c| self.cands[c] != 0),
            );
            search.grow(0, 0);
        }
        let mut found = search.found;
        found.sort_by_key(|s| s.cells.len());
        found
    }

    /// For each digit, the cells where it is a candidate.
    fn spots(&self) -> [Cells; 10] {
        let mut spots = [0; 10];
        for (c, &m) in self.cands.iter().enumerate() {
            for d in bits(m) {
                spots[d] |= 1 << c;
            }
        }
        spots
    }

    /// Two almost locked sets linked by digit x, both fitting another
    /// digit z: one of them has no x, so it holds z. Cells seeing every
    /// z in both can't be z. The smallest pair comes first.
    pub(super) fn als_xz(&self) -> Option<Step> {
        let sets = self.almost_locked_sets(4);
        let spots = self.spots();
        let start = |n: usize| sets.partition_point(|s| s.cells.len() < n);
        for total in 2..=8 {
            for na in 1..=total / 2 {
                let nb = total - na;
                if nb > 4 {
                    continue;
                }
                for ia in start(na)..start(na + 1) {
                    let a = &sets[ia];
                    let from = if na == nb { ia + 1 } else { start(nb) };
                    for b in &sets[from..start(nb + 1)] {
                        let common = a.digits & b.digits;
                        if a.set & b.set != 0 || common.count_ones() < 2 {
                            continue;
                        }
                        for x in bits(restricted(a, b)) {
                            for z in bits(common & !(1 << x)) {
                                let targets = a.seers[z] & b.seers[z] & spots[z] & !(a.set | b.set);
                                if targets != 0 {
                                    return self.als_xz_step(a, b, x, z, targets);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn als_xz_step(&self, a: &Als, b: &Als, x: usize, z: usize, targets: Cells) -> Option<Step> {
        let targets: Vec<usize> = cells_in(targets).collect();
        let why = format!(
            "Each set fits one more digit than it has cells: A = {}; B = {}. Every {x} in A \
             sees every {x} in B, so at least one set has no {x}, and its cells take all its \
             other digits, {z} included. So A or B holds {z}, and {}",
            a.describe(),
            b.describe(),
            conclusion(&targets, z),
        );
        let cells = cells_of(&[a, b]);
        let mut marks = self.marks(cells.iter().copied(), 1 << z, Role::Key);
        marks.extend(self.marks(cells.iter().copied(), 1 << x, Role::Other));
        let x8 = x as u8;
        let pattern = Pattern {
            houses: houses_of(&[a, b]),
            cells,
            marks,
            links: vec![Link {
                from: (a.first(x), x8),
                to: (b.first(x), x8),
                kind: LinkKind::Weak,
            }],
            ..Pattern::default()
        };
        let removals = targets.iter().map(|&t| (t, 1 << z)).collect();
        self.eliminating(Technique::AlsXz, removals, pattern, why)
    }

    /// Almost locked sets A and B, each linked to a third, C, by its own
    /// digit (x and y). C misses at most one digit, so it holds x or y,
    /// and then A or B has no x or y and holds a digit z both fit. Cells
    /// seeing every z in A and B can't be z.
    pub(super) fn als_xy_wing(&self) -> Option<Step> {
        let sets = self.almost_locked_sets(4);
        let spots = self.spots();
        // For each set, the sets linked to it and the digits linking them.
        let mut linked: Vec<Vec<(usize, u16)>> = vec![Vec::new(); sets.len()];
        for (i, a) in sets.iter().enumerate() {
            for (j, b) in sets.iter().enumerate().skip(i + 1) {
                if a.set & b.set == 0 {
                    let digits = restricted(a, b);
                    if digits != 0 {
                        linked[i].push((j, digits));
                        linked[j].push((i, digits));
                    }
                }
            }
        }
        for (c, near) in sets.iter().zip(&linked) {
            for (k, &(ia, xs)) in near.iter().enumerate() {
                let a = &sets[ia];
                for &(ib, ys) in &near[k + 1..] {
                    let b = &sets[ib];
                    let common = a.digits & b.digits;
                    if a.set & b.set != 0 || common == 0 {
                        continue;
                    }
                    for x in bits(xs) {
                        for y in bits(ys & !(1 << x)) {
                            for z in bits(common & !(1 << x | 1 << y)) {
                                let targets =
                                    a.seers[z] & b.seers[z] & spots[z] & !(a.set | b.set | c.set);
                                if targets == 0 {
                                    continue;
                                }
                                // A is the set with the lower link digit.
                                return if x < y {
                                    self.als_xy_step([a, b, c], [x, y, z], targets)
                                } else {
                                    self.als_xy_step([b, a, c], [y, x, z], targets)
                                };
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn als_xy_step(
        &self,
        [a, b, c]: [&Als; 3],
        [x, y, z]: [usize; 3],
        targets: Cells,
    ) -> Option<Step> {
        let targets: Vec<usize> = cells_in(targets).collect();
        let why = format!(
            "Each set fits one more digit than it has cells: A = {}; B = {}; C = {}. C misses \
             at most one of its digits, so it holds {x} or {y}. Every {x} in C sees every {x} \
             in A, and every {y} in C every {y} in B. If C holds {x}, A has no {x}, so its \
             cells take all its other digits, {z} included; if {y}, the same goes for B. So A \
             or B holds {z}, and {}",
            a.describe(),
            b.describe(),
            c.describe(),
            conclusion(&targets, z),
        );
        let mut marks = self.marks(cells_of(&[a, b]), 1 << z, Role::Key);
        marks.extend(self.marks(cells_of(&[a, c]), 1 << x, Role::Other));
        marks.extend(self.marks(cells_of(&[b, c]), 1 << y, Role::Other));
        let link = |from: &Als, to: &Als, d: usize| Link {
            from: (from.first(d), d as u8),
            to: (to.first(d), d as u8),
            kind: LinkKind::Weak,
        };
        let pattern = Pattern {
            houses: houses_of(&[a, b, c]),
            cells: cells_of(&[a, b, c]),
            marks,
            links: vec![link(a, c, x), link(c, b, y)],
            ..Pattern::default()
        };
        let removals = targets.iter().map(|&t| (t, 1 << z)).collect();
        self.eliminating(Technique::AlsXyWing, removals, pattern, why)
    }

    /// Two or three cells where a box meets a line, fitting two more
    /// digits than cells, with a cell of just two of those digits in the
    /// line outside the box and another, sharing no digit with it, in the
    /// box outside the line. Those cells all differ and hold every digit
    /// once, which pins each digit to the line, the box or both.
    pub(super) fn sue_de_coq(&self) -> Option<Step> {
        for b in 0..N {
            let bh = 2 * N + b;
            let in_box = HOUSE_CELLS[bh];
            let corner = usize::from(HOUSES[bh][0]);
            let rows = (0..3).map(|k| row_of(corner) + k);
            let cols = (0..3).map(|k| N + col_of(corner) + k);
            for lh in rows.chain(cols) {
                let in_line = HOUSE_CELLS[lh];
                let shared: Vec<usize> = cells_in(in_box & in_line)
                    .filter(|&c| self.cands[c] != 0)
                    .collect();
                for pick in 1..1u32 << shared.len() {
                    if pick.count_ones() < 2 {
                        continue;
                    }
                    let core: Vec<usize> = bits(pick as u16).map(|k| shared[k]).collect();
                    let digits = core.iter().fold(0, |m, &c| m | self.cands[c]);
                    if digits.count_ones() as usize != core.len() + 2 {
                        continue;
                    }
                    let helpers = |cells: Cells| -> Vec<usize> {
                        cells_in(cells)
                            .filter(|&c| {
                                self.cands[c].count_ones() == 2 && self.cands[c] & !digits == 0
                            })
                            .collect()
                    };
                    let line_helpers = helpers(in_line & !in_box);
                    for bx in helpers(in_box & !in_line) {
                        for &l in &line_helpers {
                            if self.cands[l] & self.cands[bx] != 0 {
                                continue;
                            }
                            let step = self.sue_de_coq_step([bh, lh], &core, digits, [l, bx]);
                            if step.is_some() {
                                return step;
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn sue_de_coq_step(
        &self,
        [bh, lh]: [usize; 2],
        core: &[usize],
        digits: u16,
        [l, bx]: [usize; 2],
    ) -> Option<Step> {
        let (in_box, in_line) = (HOUSE_CELLS[bh], HOUSE_CELLS[lh]);
        let (dl, db) = (self.cands[l], self.cands[bx]);
        let rest = digits & !(dl | db);
        let mut pattern_cells = core.to_vec();
        pattern_cells.extend([l, bx]);
        let outside = (in_box | in_line) & !pattern_cells.iter().fold(0, |s, &c| s | 1 << c);
        let removals: Vec<(usize, u16)> = cells_in(outside)
            .map(|c| {
                let line_cut = if in_line & 1 << c != 0 { dl | rest } else { 0 };
                let box_cut = if in_box & 1 << c != 0 { db | rest } else { 0 };
                (c, (line_cut | box_cut) & self.cands[c])
            })
            .filter(|&(_, mask)| mask != 0)
            .collect();
        if removals.is_empty() {
            return None;
        }
        // Cells losing the same digits are named together.
        let mut groups: Vec<(u16, Vec<usize>)> = Vec::new();
        for &(c, mask) in &removals {
            match groups.iter_mut().find(|(m, _)| *m == mask) {
                Some((_, cells)) => cells.push(c),
                None => groups.push((mask, vec![c])),
            }
        }
        let cuts: Vec<String> = groups
            .iter()
            .map(|(mask, cells)| format!("{} can't be {}", cell_list(cells), digit_choice(*mask)))
            .collect();
        let cuts = match cuts.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{}, and {last}", rest.join(", ")),
            _ => cuts.concat(),
        };
        let count = ["four", "five"][core.len() - 2];
        let rest_part = if rest == 0 {
            String::new()
        } else {
            format!(", and {} in the shared ones", digit_list(rest))
        };
        let why = format!(
            "{} fit only {}. With {} ({}) and {} ({}), that's {count} cells for {count} digits, \
             and no two can match (each two share a house or no digit), so each digit is in \
             one of them. So {} are in the {} cells among them, {}{} in the {} cells{rest_part}. \
             So {cuts}.",
            cell_list(core),
            digit_list(digits),
            rc(l),
            digit_choice(dl),
            rc(bx),
            digit_choice(db),
            digit_list(dl),
            house_name(lh),
            if rest == 0 { "and " } else { "" },
            digit_list(db),
            house_name(bh),
        );
        let mut marks = self.marks(core.iter().copied(), digits, Role::Key);
        marks.extend(self.marks([l], dl, Role::Other));
        marks.extend(self.marks([bx], db, Role::Other));
        let pattern = Pattern {
            houses: vec![bh, lh],
            cells: pattern_cells,
            marks,
            ..Pattern::default()
        };
        self.eliminating(Technique::SueDeCoq, removals, pattern, why)
    }

    /// A stem cell of two or three digits, and for each digit a petal: an
    /// almost locked set of at most three cells whose cells fitting that
    /// digit all see the stem. Whichever digit the stem is, its petal has
    /// none of it and holds all its other digits, so a digit z in every
    /// petal (and not in the stem) is in one of them. Cells seeing every z
    /// in the petals can't be z.
    pub(super) fn death_blossom(&self) -> Option<Step> {
        let sets = self.almost_locked_sets(3);
        let spots = self.spots();
        for stem in (0..CELLS).filter(|&c| (2..=3).contains(&self.cands[c].count_ones())) {
            let stem_digits = self.cands[stem];
            // For each stem digit, the sets that could be its petal.
            let petals: Vec<Vec<usize>> = bits(stem_digits)
                .map(|d| {
                    (0..sets.len())
                        .filter(|&i| {
                            let s = &sets[i];
                            s.set & 1 << stem == 0
                                && s.spots[d] != 0
                                && s.spots[d] & !SEEN[stem] == 0
                        })
                        .collect()
                })
                .collect();
            if petals.iter().any(Vec::is_empty) {
                continue;
            }
            for z in bits(ALL_DIGITS & !stem_digits) {
                let holding: Vec<Vec<usize>> = petals
                    .iter()
                    .map(|p| {
                        p.iter()
                            .copied()
                            .filter(|&i| sets[i].digits & 1 << z != 0)
                            .collect()
                    })
                    .collect();
                let mut chosen = Vec::with_capacity(holding.len());
                let targets = spots[z] & !(1 << stem);
                if let Some(targets) = bloom(&sets, &holding, z, 1 << stem, targets, &mut chosen) {
                    let petals: Vec<&Als> = chosen.iter().map(|&i| &sets[i]).collect();
                    return self.death_blossom_step(stem, &petals, z, targets);
                }
            }
        }
        None
    }

    fn death_blossom_step(
        &self,
        stem: usize,
        petals: &[&Als],
        z: usize,
        targets: Cells,
    ) -> Option<Step> {
        let targets: Vec<usize> = cells_in(targets).collect();
        let stem_digits = self.cands[stem];
        let described: Vec<String> = bits(stem_digits)
            .zip(petals)
            .map(|(d, p)| format!("{}, whose {d}s all see {}", p.describe(), rc(stem)))
            .collect();
        let why = format!(
            "{} is {}. Each petal fits one more digit than it has cells: {}. Whichever digit \
             {} is, its petal has none of it, so the petal's cells take all its other digits, \
             {z} included. So a petal holds {z}, and {}",
            rc(stem),
            digit_choice(stem_digits),
            described.join("; "),
            rc(stem),
            conclusion(&targets, z),
        );
        let mut cells = vec![stem];
        cells.extend(petals.iter().flat_map(|p| p.cells.iter().copied()));
        let mut marks = self.marks(cells_of(petals), 1 << z, Role::Key);
        marks.extend(self.marks([stem], stem_digits, Role::Other));
        let mut links = Vec::new();
        for (d, p) in bits(stem_digits).zip(petals) {
            marks.extend(self.marks(p.cells.iter().copied(), 1 << d, Role::Other));
            links.push(Link {
                from: (stem, d as u8),
                to: (p.first(d), d as u8),
                kind: LinkKind::Weak,
            });
        }
        let pattern = Pattern {
            houses: houses_of(petals),
            cells,
            marks,
            links,
            ..Pattern::default()
        };
        let removals = targets.iter().map(|&t| (t, 1 << z)).collect();
        self.eliminating(Technique::DeathBlossom, removals, pattern, why)
    }
}

/// Pick a petal from each of `lists` (indexes into `sets`), sharing no
/// cell with `used` or each other, keeping in `targets` only the cells
/// that see every z in the petals picked. Returns the targets left, if
/// any, with the petals in `chosen`.
fn bloom(
    sets: &[Als],
    lists: &[Vec<usize>],
    z: usize,
    used: Cells,
    targets: Cells,
    chosen: &mut Vec<usize>,
) -> Option<Cells> {
    let Some((first, rest)) = lists.split_first() else {
        return Some(targets);
    };
    for &i in first {
        let petal = &sets[i];
        if petal.set & used != 0 {
            continue;
        }
        let used = used | petal.set;
        let targets = targets & petal.seers[z] & !used;
        if targets == 0 {
            continue;
        }
        chosen.push(i);
        if let Some(found) = bloom(sets, rest, z, used, targets, chosen) {
            return Some(found);
        }
        chosen.pop();
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::logic::testing::{removals, staged};
    use crate::logic::{Link, LinkKind, Role, Step, Technique};
    use crate::sudoku::idx;

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

    fn weak(from: (usize, u8), to: (usize, u8)) -> Link {
        Link {
            from,
            to,
            kind: LinkKind::Weak,
        }
    }

    #[test]
    fn als_xz_clears_the_shared_digit() {
        // A: r1c1 {1,2} and r1c2 {1,2,3}, two cells of row 1 fitting 1, 2
        // and 3. B: r5c2 {3,5} and r5c5 {1,5}, two cells of row 5 fitting
        // 1, 3 and 5. Their only 3s, r1c2 and r5c2, share column 2, so one
        // set has no 3 and its cells take all its other digits: one set
        // holds 1. r1c5 sees every 1 of both (row 1 and column 5), so it
        // can't be 1; r1c9 sees A's 1s but not B's, so it keeps its 1.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 1), &[1, 2, 3]),
            (idx(4, 1), &[3, 5]),
            (idx(4, 4), &[1, 5]),
            (idx(0, 4), &[1, 7]),
            (idx(0, 8), &[1, 8]),
        ]);
        let step = grid.als_xz().unwrap();
        assert_eq!(step.technique, Technique::AlsXz);
        let pattern = &step.pattern;
        assert_eq!(pattern.digit, None);
        assert_eq!(pattern.houses, [0, 4], "rows 1 and 5");
        assert_eq!(pattern.cells, [idx(0, 0), idx(0, 1), idx(4, 1), idx(4, 4)]);
        assert_eq!(
            marked(&step, Role::Key),
            [(idx(0, 0), 1), (idx(0, 1), 1), (idx(4, 4), 1)]
        );
        assert_eq!(marked(&step, Role::Other), [(idx(0, 1), 3), (idx(4, 1), 3)]);
        assert_eq!(pattern.links, [weak((idx(0, 1), 3), (idx(4, 1), 3))]);
        assert_eq!(
            step.why,
            "Each set fits one more digit than it has cells: A = r1c1, r1c2 (1, 2, 3); \
             B = r5c2, r5c5 (1, 3, 5). Every 3 in A sees every 3 in B, so at least one \
             set has no 3, and its cells take all its other digits, 1 included. So A or B \
             holds 1, and r1c5 sees every 1 in them, so it can't be 1."
        );
        assert_eq!(removals(Some(step)), [(idx(0, 4), 1 << 1)]);
    }

    #[test]
    fn als_xz_needs_every_link_cell_to_see() {
        // As above, but B is r5c2 {3,5}, r5c7 {3,6} and r5c5 {1,5,6}:
        // three cells of row 5 fitting 1, 3, 5 and 6. Its 3 in r5c2 sees
        // A's 3 in r1c2, but its 3 in r5c7 doesn't, so both sets could
        // hold 3 and nothing links them.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 1), &[1, 2, 3]),
            (idx(4, 1), &[3, 5]),
            (idx(4, 6), &[3, 6]),
            (idx(4, 4), &[1, 5, 6]),
            (idx(0, 4), &[1, 7]),
        ]);
        assert_eq!(grid.als_xz(), None);
    }

    #[test]
    fn als_xy_wing_clears_z() {
        // C: r1c1 and r1c2 {1,2,5}, in row 1. A: r2c3 and r3c3 {1,4,9}, in
        // column 3. B: r1c5 and r1c6 {2,6,9}, in row 1. C's two cells take
        // two of its three digits, so it holds 1 or 2. Every 1 in A sees
        // every 1 in C (box 1), and every 2 in B every 2 in C (row 1).
        // If C holds 1, A has none and holds 9; if 2, B holds 9. r1c3 sees
        // every 9 of A and B, so it can't be 9; r2c5 misses r3c3.
        let grid = staged(&[
            (idx(0, 0), &[1, 2, 5]),
            (idx(0, 1), &[1, 2, 5]),
            (idx(1, 2), &[1, 4, 9]),
            (idx(2, 2), &[1, 4, 9]),
            (idx(0, 4), &[2, 6, 9]),
            (idx(0, 5), &[2, 6, 9]),
            (idx(0, 2), &[3, 7, 9]),
            (idx(1, 4), &[7, 8, 9]),
        ]);
        let step = grid.als_xy_wing().unwrap();
        assert_eq!(step.technique, Technique::AlsXyWing);
        let pattern = &step.pattern;
        assert_eq!(pattern.digit, None);
        assert_eq!(pattern.houses, [0, 11], "row 1 and column 3");
        assert_eq!(
            marked(&step, Role::Key),
            [
                (idx(0, 4), 9),
                (idx(0, 5), 9),
                (idx(1, 2), 9),
                (idx(2, 2), 9)
            ]
        );
        assert_eq!(
            marked(&step, Role::Other),
            [
                (idx(0, 0), 1),
                (idx(0, 0), 2),
                (idx(0, 1), 1),
                (idx(0, 1), 2),
                (idx(0, 4), 2),
                (idx(0, 5), 2),
                (idx(1, 2), 1),
                (idx(2, 2), 1),
            ]
        );
        assert_eq!(
            pattern.links,
            [
                weak((idx(1, 2), 1), (idx(0, 0), 1)),
                weak((idx(0, 0), 2), (idx(0, 4), 2)),
            ]
        );
        assert_eq!(
            step.why,
            "Each set fits one more digit than it has cells: A = r2c3, r3c3 (1, 4, 9); \
             B = r1c5, r1c6 (2, 6, 9); C = r1c1, r1c2 (1, 2, 5). C misses at most one \
             of its digits, so it holds 1 or 2. Every 1 in C sees every 1 in A, and \
             every 2 in C every 2 in B. If C holds 1, A has no 1, so its cells take all \
             its other digits, 9 included; if 2, the same goes for B. So A or B holds 9, \
             and r1c3 sees every 9 in them, so it can't be 9."
        );
        assert_eq!(removals(Some(step)), [(idx(0, 2), 1 << 9)]);
    }

    #[test]
    fn sue_de_coq_splits_the_intersection() {
        // Where box 1 meets row 1, r1c1 {1,2,5}, r1c2 {3,4,5} and r1c3
        // {1,3} fit five digits. r1c5 {1,2} is in row 1 outside the box,
        // r2c2 {3,4} in the box outside the row. Those five cells take five
        // different digits, so 1 and 2 are in row 1's part, 3 and 4 in box
        // 1's and 5 in the shared cells. r1c8 loses 1 and 5 (keeping 3),
        // r3c3 loses 3 and 5 (keeping 1).
        let grid = staged(&[
            (idx(0, 0), &[1, 2, 5]),
            (idx(0, 1), &[3, 4, 5]),
            (idx(0, 2), &[1, 3]),
            (idx(0, 4), &[1, 2]),
            (idx(1, 1), &[3, 4]),
            (idx(0, 7), &[1, 3, 5, 9]),
            (idx(2, 2), &[1, 3, 5, 8]),
        ]);
        let step = grid.sue_de_coq().unwrap();
        assert_eq!(step.technique, Technique::SueDeCoq);
        let pattern = &step.pattern;
        assert_eq!(pattern.digit, None);
        assert_eq!(pattern.houses, [18, 0], "box 1 and row 1");
        assert_eq!(
            pattern.cells,
            [idx(0, 0), idx(0, 1), idx(0, 2), idx(0, 4), idx(1, 1)]
        );
        assert_eq!(
            marked(&step, Role::Key),
            [
                (idx(0, 0), 1),
                (idx(0, 0), 2),
                (idx(0, 0), 5),
                (idx(0, 1), 3),
                (idx(0, 1), 4),
                (idx(0, 1), 5),
                (idx(0, 2), 1),
                (idx(0, 2), 3),
            ]
        );
        assert_eq!(
            marked(&step, Role::Other),
            [
                (idx(0, 4), 1),
                (idx(0, 4), 2),
                (idx(1, 1), 3),
                (idx(1, 1), 4)
            ]
        );
        assert_eq!(
            step.why,
            "r1c1, r1c2 and r1c3 fit only 1, 2, 3, 4 and 5. With r1c5 (1 or 2) and \
             r2c2 (3 or 4), that's five cells for five digits, and no two can match \
             (each two share a house or no digit), so each digit is in one of them. \
             So 1 and 2 are in the row 1 cells among them, 3 and 4 in the box 1 cells, \
             and 5 in the shared ones. So r1c8 can't be 1 or 5, and r3c3 can't be 3 or 5."
        );
        assert_eq!(
            removals(Some(step)),
            [(idx(0, 7), 1 << 1 | 1 << 5), (idx(2, 2), 1 << 3 | 1 << 5)]
        );
    }

    #[test]
    fn death_blossom_with_two_petals() {
        // Stem r1c1 {1,2}. Petal for 1: r1c5 {1,3} and r1c6 {3,9} in row
        // 1, whose only 1 sees the stem. Petal for 2: r5c1 {2,4} and r6c1
        // {4,9} in column 1, whose only 2 sees the stem. Whichever the stem
        // is, that petal loses it and holds 9. r6c6 sees both 9s, so it
        // can't be 9; r6c5 misses r1c6.
        let grid = staged(&[
            (idx(0, 0), &[1, 2]),
            (idx(0, 4), &[1, 3]),
            (idx(0, 5), &[3, 9]),
            (idx(4, 0), &[2, 4]),
            (idx(5, 0), &[4, 9]),
            (idx(5, 5), &[7, 9]),
            (idx(5, 4), &[8, 9]),
        ]);
        let step = grid.death_blossom().unwrap();
        assert_eq!(step.technique, Technique::DeathBlossom);
        let pattern = &step.pattern;
        assert_eq!(pattern.digit, None);
        assert_eq!(pattern.houses, [0, 9], "row 1 and column 1");
        assert_eq!(
            pattern.cells,
            [idx(0, 0), idx(0, 4), idx(0, 5), idx(4, 0), idx(5, 0)]
        );
        assert_eq!(marked(&step, Role::Key), [(idx(0, 5), 9), (idx(5, 0), 9)]);
        assert_eq!(
            marked(&step, Role::Other),
            [
                (idx(0, 0), 1),
                (idx(0, 0), 2),
                (idx(0, 4), 1),
                (idx(4, 0), 2)
            ]
        );
        assert_eq!(
            pattern.links,
            [
                weak((idx(0, 0), 1), (idx(0, 4), 1)),
                weak((idx(0, 0), 2), (idx(4, 0), 2)),
            ]
        );
        assert_eq!(
            step.why,
            "r1c1 is 1 or 2. Each petal fits one more digit than it has cells: \
             r1c5, r1c6 (1, 3, 9), whose 1s all see r1c1; r5c1, r6c1 (2, 4, 9), whose \
             2s all see r1c1. Whichever digit r1c1 is, its petal has none of it, so the \
             petal's cells take all its other digits, 9 included. So a petal holds 9, \
             and r6c6 sees every 9 in them, so it can't be 9."
        );
        assert_eq!(removals(Some(step)), [(idx(5, 5), 1 << 9)]);
    }
}
