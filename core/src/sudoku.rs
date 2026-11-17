//! Board primitives: indexing, the peer/house tables, a brute-force
//! solver for uniqueness checks, and digging puzzles out of full grids.
//!
//! Everything here is pure computation with no I/O and no network access.

use rand::Rng;
use rand::seq::SliceRandom;

pub const N: usize = 9;
pub const CELLS: usize = 81;

/// A 9x9 grid in row-major order; `0` is an empty cell.
pub type Board = [u8; CELLS];

/// Candidate bitmask with bits 1..=9 set.
pub const ALL_DIGITS: u16 = 0x3FE;

#[inline]
pub const fn idx(row: usize, col: usize) -> usize {
    row * N + col
}

#[inline]
pub const fn row_of(i: usize) -> usize {
    i / N
}

#[inline]
pub const fn col_of(i: usize) -> usize {
    i % N
}

#[inline]
pub const fn box_of(row: usize, col: usize) -> usize {
    (row / 3) * 3 + col / 3
}

#[inline]
pub const fn box_of_cell(i: usize) -> usize {
    box_of(row_of(i), col_of(i))
}

/// The 20 peers of every cell: the other cells sharing its row, column or box.
pub static PEERS: [[u8; 20]; CELLS] = build_peers();

/// All 27 houses: rows 0-8, then columns 9-17, then boxes 18-26.
pub static HOUSES: [[u8; N]; 27] = build_houses();

const fn build_peers() -> [[u8; 20]; CELLS] {
    let mut out = [[0u8; 20]; CELLS];
    let mut i = 0;
    while i < CELLS {
        let mut n = 0;
        let mut p = 0;
        while p < CELLS {
            if p != i
                && (row_of(p) == row_of(i)
                    || col_of(p) == col_of(i)
                    || box_of_cell(p) == box_of_cell(i))
            {
                out[i][n] = p as u8;
                n += 1;
            }
            p += 1;
        }
        i += 1;
    }
    out
}

const fn build_houses() -> [[u8; N]; 27] {
    let mut out = [[0u8; N]; 27];
    let mut h = 0;
    while h < N {
        let mut k = 0;
        while k < N {
            out[h][k] = idx(h, k) as u8;
            out[N + h][k] = idx(k, h) as u8;
            out[2 * N + h][k] = idx((h / 3) * 3 + k / 3, (h % 3) * 3 + k % 3) as u8;
            k += 1;
        }
        h += 1;
    }
    out
}

/// Iterate the 20 peers of cell `i`.
#[inline]
pub fn peers(i: usize) -> impl Iterator<Item = usize> {
    PEERS[i].iter().map(|&p| usize::from(p))
}

/// Bitmask of candidates (bits 1..=9) for a cell given the current board.
pub fn candidates_mask(board: &Board, i: usize) -> u16 {
    // An empty peer sets bit 0, which the final mask drops.
    let used = peers(i).fold(0u16, |m, p| m | (1 << board[p]));
    !used & ALL_DIGITS
}

/// Each cell's row, column and box.
static HOUSE_OF: [[u8; 3]; CELLS] = build_house_of();

const fn build_house_of() -> [[u8; 3]; CELLS] {
    let mut out = [[0u8; 3]; CELLS];
    let mut i = 0;
    while i < CELLS {
        out[i] = [row_of(i) as u8, col_of(i) as u8, box_of_cell(i) as u8];
        i += 1;
    }
    out
}

/// A board being searched, with the digits each row, column and box
/// already holds kept up to date, so a cell's candidates are three
/// lookups rather than a pass over its 20 peers.
struct Solver {
    board: Board,
    /// Bitmasks of used digits: `used[0]` per row, `[1]` per column,
    /// `[2]` per box.
    used: [[u16; N]; 3],
}

impl Solver {
    fn new(board: &Board) -> Solver {
        let mut used = [[0; N]; 3];
        for (i, &v) in board.iter().enumerate() {
            for (k, &h) in HOUSE_OF[i].iter().enumerate() {
                used[k][usize::from(h)] |= 1 << v;
            }
        }
        Solver {
            board: *board,
            used,
        }
    }

    /// The same as [`candidates_mask`] on the current board.
    fn candidates(&self, i: usize) -> u16 {
        let [r, c, b] = HOUSE_OF[i].map(usize::from);
        !(self.used[0][r] | self.used[1][c] | self.used[2][b]) & ALL_DIGITS
    }

    /// Put `v` in empty cell `i`, which must be one of its candidates.
    fn place(&mut self, i: usize, v: u8) {
        for (k, &h) in HOUSE_OF[i].iter().enumerate() {
            self.used[k][usize::from(h)] |= 1 << v;
        }
        self.board[i] = v;
    }

    /// Empty cell `i` again after [`Solver::place`].
    fn lift(&mut self, i: usize) {
        let v = self.board[i];
        for (k, &h) in HOUSE_OF[i].iter().enumerate() {
            self.used[k][usize::from(h)] &= !(1 << v);
        }
        self.board[i] = 0;
    }

    /// Find the empty cell with the fewest candidates (MRV heuristic),
    /// stopping early once a cell with exactly one candidate is found.
    /// `None` means the board is full; a returned mask of `0` means that
    /// cell is unsolvable.
    fn mrv_cell(&self) -> Option<(usize, u16)> {
        let mut best: Option<(usize, u16)> = None;
        for i in (0..CELLS).filter(|&i| self.board[i] == 0) {
            let mask = self.candidates(i);
            match mask.count_ones() {
                0 => return Some((i, 0)),
                1 => return Some((i, mask)),
                n if best.is_none_or(|(_, m)| n < m.count_ones()) => best = Some((i, mask)),
                _ => {}
            }
        }
        best
    }

    fn count(&mut self, limit: u32) -> u32 {
        let (cell, mask) = match self.mrv_cell() {
            None => return 1, // no empties: one solution found
            Some((_, 0)) => return 0,
            Some(cm) => cm,
        };
        let mut count = 0;
        for v in (1..=9u8).filter(|v| mask & (1 << v) != 0) {
            self.place(cell, v);
            count += self.count(limit.saturating_sub(count));
            self.lift(cell);
            if count >= limit {
                break;
            }
        }
        count
    }

    /// Fill the board, leaving it filled if that works and as it was if
    /// not.
    fn solve(&mut self) -> bool {
        let (cell, mask) = match self.mrv_cell() {
            None => return true,
            Some((_, 0)) => return false,
            Some(cm) => cm,
        };
        for v in (1..=9u8).filter(|v| mask & (1 << v) != 0) {
            self.place(cell, v);
            if self.solve() {
                return true;
            }
            self.lift(cell);
        }
        false
    }

    fn fill_random<R: Rng>(&mut self, rng: &mut R) -> bool {
        let (cell, mask) = match self.mrv_cell() {
            None => return true,
            Some((_, 0)) => return false,
            Some(cm) => cm,
        };
        let mut cands = [0u8; N];
        let mut n = 0;
        for v in (1..=9u8).filter(|v| mask & (1 << v) != 0) {
            cands[n] = v;
            n += 1;
        }
        let cands = &mut cands[..n];
        cands.shuffle(rng);
        for &v in cands.iter() {
            self.place(cell, v);
            if self.fill_random(rng) {
                return true;
            }
            self.lift(cell);
        }
        false
    }
}

/// Count solutions up to `limit` using MRV backtracking. Fast enough for
/// uniqueness checks during generation.
pub fn count_solutions(board: &Board, limit: u32) -> u32 {
    Solver::new(board).count(limit)
}

/// True when the puzzle has exactly one solution.
pub fn is_unique(puzzle: &Board) -> bool {
    count_solutions(puzzle, 2) == 1
}

/// Solve the puzzle, returning the first solution found (if any).
pub fn solve_one(board: &Board) -> Option<Board> {
    let mut solver = Solver::new(board);
    solver.solve().then_some(solver.board)
}

/// Build a random complete grid with shuffled DFS.
pub fn full_solution<R: Rng>(rng: &mut R) -> Board {
    let mut solver = Solver::new(&[0; CELLS]);
    solver.fill_random(rng);
    solver.board
}

/// Solvability by constraint propagation, for [`dig`]'s many yes/no
/// questions: forced cells are filled in a tight loop touching only a
/// placement's 20 peers, and a cell or house left with no place for a
/// digit ends a branch at once. It only says whether a solution exists,
/// not what it is.
mod propagate {
    use super::{ALL_DIGITS, CELLS, HOUSE_OF, HOUSES, N, PEERS};

    /// What is still possible: the candidates of every open cell (`0` for
    /// a filled one, so an open cell always has a bit set while the grid
    /// is consistent) and the digits each house already holds.
    #[derive(Clone, Copy)]
    pub struct State {
        pub cands: [u16; CELLS],
        done: [u16; 3 * N],
    }

    impl State {
        pub fn new(cands: [u16; CELLS], board: &[u8; CELLS]) -> State {
            let mut done = [0; 3 * N];
            for (i, &v) in board.iter().enumerate() {
                let [r, c, b] = HOUSE_OF[i].map(usize::from);
                for h in [r, N + c, 2 * N + b] {
                    done[h] |= 1 << v;
                }
            }
            State { cands, done }
        }
    }

    /// Cells holding a single candidate, whose digit is therefore forced.
    /// A cell is queued when it starts there or gets there, so at most
    /// twice.
    pub struct Queue {
        cells: [u8; 2 * CELLS],
        len: usize,
    }

    impl Queue {
        pub fn new() -> Queue {
            Queue {
                cells: [0; 2 * CELLS],
                len: 0,
            }
        }

        fn push(&mut self, i: usize) {
            self.cells[self.len] = i as u8;
            self.len += 1;
        }
    }

    /// Fill cell `i` with `v`, striking `v` from its peers. False when a
    /// peer is left with nothing.
    pub fn assign(s: &mut State, queue: &mut Queue, i: usize, v: u8) -> bool {
        let bit = 1u16 << v;
        s.cands[i] = 0;
        let [r, c, b] = HOUSE_OF[i].map(usize::from);
        for h in [r, N + c, 2 * N + b] {
            s.done[h] |= bit;
        }
        for &p in &PEERS[i] {
            let p = usize::from(p);
            if s.cands[p] & bit != 0 {
                let left = s.cands[p] & !bit;
                if left == 0 {
                    return false;
                }
                s.cands[p] = left;
                if left & (left - 1) == 0 {
                    queue.push(p);
                }
            }
        }
        true
    }

    /// Fill every forced cell, and those that force. False on a dead end.
    pub fn run(s: &mut State, queue: &mut Queue) -> bool {
        while queue.len > 0 {
            queue.len -= 1;
            let i = usize::from(queue.cells[queue.len]);
            let m = s.cands[i];
            if m != 0 && m & (m - 1) == 0 && !assign(s, queue, i, m.trailing_zeros() as u8) {
                return false;
            }
        }
        true
    }

    /// Fill every cell that is forced, by having a single candidate or by
    /// being the one place left for a digit in a house, wherever it is.
    /// False on a dead end: a cell, or a house's digit, with nowhere to go.
    pub fn settle(s: &mut State) -> bool {
        let mut queue = Queue::new();
        for (i, &m) in s.cands.iter().enumerate() {
            if m != 0 && m & (m - 1) == 0 {
                queue.push(i);
            }
        }
        loop {
            if !run(s, &mut queue) {
                return false;
            }
            let mut changed = false;
            for (h, house) in HOUSES.iter().enumerate() {
                let (mut once, mut twice) = (0u16, 0u16);
                for &c in house {
                    let m = s.cands[usize::from(c)];
                    twice |= once & m;
                    once |= m;
                }
                let open = ALL_DIGITS & !s.done[h];
                if open & !once != 0 {
                    return false;
                }
                let mut hidden = once & !twice;
                while hidden != 0 {
                    let v = hidden.trailing_zeros() as u8;
                    hidden &= hidden - 1;
                    let bit = 1 << v;
                    if let Some(&c) = house.iter().find(|&&c| s.cands[usize::from(c)] & bit != 0)
                        && !assign(s, &mut queue, usize::from(c), v)
                    {
                        return false;
                    }
                    changed = true;
                }
                if queue.len > 0 {
                    break;
                }
            }
            if !changed && queue.len == 0 {
                return true;
            }
        }
    }

    /// True when the grid, already settled, can be completed.
    pub fn solvable(s: &State) -> bool {
        // Branch on the open cell with the fewest candidates.
        let mut best: Option<(usize, u32)> = None;
        for (i, &m) in s.cands.iter().enumerate() {
            if m != 0 {
                let n = m.count_ones();
                if best.is_none_or(|(_, b)| n < b) {
                    best = Some((i, n));
                }
            }
        }
        let Some((cell, _)) = best else {
            return true;
        };
        let mask = s.cands[cell];
        (1..=9u8).filter(|v| mask & (1 << v) != 0).any(|v| {
            let mut next = *s;
            let mut queue = Queue::new();
            assign(&mut next, &mut queue, cell, v)
                && run(&mut next, &mut queue)
                && settle(&mut next)
                && solvable(&next)
        })
    }
}

/// Blank cells of `solved` in random order while the puzzle keeps a unique
/// solution, stopping once `target` cells are empty.
///
/// One pass is all it takes: blanking more cells never makes a puzzle more
/// constrained, so a cell that can't be removed now can't be removed later.
pub fn dig<R: Rng>(solved: &Board, target: usize, rng: &mut R) -> Board {
    let mut solver = Solver::new(solved);
    let mut order: [usize; CELLS] = std::array::from_fn(|i| i);
    order.shuffle(rng);
    let mut holes = 0;
    for i in order {
        if holes >= target {
            break;
        }
        let backup = solver.board[i];
        solver.lift(i);
        // The puzzle's one solution is `solved`, so a second solution
        // would have to put another digit here.
        if only_digit_fits(&solver, i, backup) {
            holes += 1;
        } else {
            solver.place(i, backup);
        }
    }
    solver.board
}

/// True when no digit but `digit` in empty cell `cell` leaves the puzzle
/// solvable.
fn only_digit_fits(puzzle: &Solver, cell: usize, digit: u8) -> bool {
    let cands = std::array::from_fn(|i| {
        if puzzle.board[i] == 0 {
            puzzle.candidates(i)
        } else {
            0
        }
    });
    let state = propagate::State::new(cands, &puzzle.board);
    // Digits the givens have not ruled out, apart from `digit`.
    let others = state.cands[cell] & !(1 << digit);
    (1..=9u8).filter(|v| others & (1 << v) != 0).all(|v| {
        let mut next = state;
        let mut queue = propagate::Queue::new();
        !(propagate::assign(&mut next, &mut queue, cell, v)
            && propagate::run(&mut next, &mut queue)
            && propagate::settle(&mut next)
            && propagate::solvable(&next))
    })
}

/// Parse an 81-character string of digits (`0` or `.` for empty).
pub fn parse_board(s: &str) -> Option<Board> {
    let mut b = [0u8; CELLS];
    let mut n = 0;
    for ch in s.chars() {
        let v = match ch {
            '.' => 0,
            c => u8::try_from(c.to_digit(10)?).ok()?,
        };
        *b.get_mut(n)? = v;
        n += 1;
    }
    (n == CELLS).then_some(b)
}

/// The inverse of [`parse_board`], using `0` for empty cells.
pub fn format_board(b: &Board) -> String {
    b.iter().map(|&v| char::from(b'0' + v)).collect()
}

/// True when `b` is a complete grid that obeys every Sudoku rule.
pub fn is_valid_solution(b: &Board) -> bool {
    HOUSES.iter().all(|house| {
        let seen = house
            .iter()
            .fold(0u16, |m, &i| m | (1 << b[usize::from(i)]));
        seen == ALL_DIGITS
    })
}

/// Classic fixed puzzle + solution so tests never pay generation cost.
#[cfg(any(test, feature = "test-support"))]
pub fn test_board() -> (Board, Board) {
    let puzzle =
        "530070000600195000098000060800060003400803001700020006060000280000419005000080079";
    let solution =
        "534678912672195348198342567859761423426853791713924856961537284287419635345286179";
    (parse_board(puzzle).unwrap(), parse_board(solution).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{RngExt, SeedableRng};

    #[test]
    fn peer_and_house_tables_are_right() {
        for i in 0..CELLS {
            let mut ps: Vec<usize> = peers(i).collect();
            ps.sort_unstable();
            ps.dedup();
            assert_eq!(ps.len(), 20, "cell {i} should have 20 distinct peers");
            for p in ps {
                assert_ne!(p, i);
                assert!(
                    row_of(p) == row_of(i)
                        || col_of(p) == col_of(i)
                        || box_of_cell(p) == box_of_cell(i)
                );
            }
        }
        // Every cell appears in exactly three houses: its row, column and box.
        let mut seen = [0; CELLS];
        for house in &HOUSES {
            for &i in house {
                seen[usize::from(i)] += 1;
            }
        }
        assert!(seen.iter().all(|&n| n == 3));
        assert_eq!(HOUSES[2 * N + 4][0] as usize, idx(3, 3));
    }

    #[test]
    fn full_grid_is_valid() {
        let mut rng = StdRng::seed_from_u64(1);
        let solved = full_solution(&mut rng);
        assert!(is_valid_solution(&solved));
    }

    #[test]
    fn dug_puzzles_stay_unique() {
        let mut rng = StdRng::seed_from_u64(7);
        let solved = full_solution(&mut rng);
        let puzzle = dig(&solved, 40, &mut rng);
        assert_eq!(puzzle.iter().filter(|&&v| v == 0).count(), 40);
        assert!(is_unique(&puzzle));
        assert_eq!(solve_one(&puzzle), Some(solved));
        let minimal = dig(&solved, CELLS, &mut rng);
        assert!(is_unique(&minimal));
        for i in (0..CELLS).filter(|&i| minimal[i] != 0) {
            let mut fewer = minimal;
            fewer[i] = 0;
            assert!(
                !is_unique(&fewer),
                "digging to CELLS leaves no removable clue"
            );
        }
    }

    /// The plain definition of [`dig`]: same shuffle, blank a cell, keep
    /// the hole when the whole puzzle is still unique.
    fn dig_by_definition<R: Rng>(solved: &Board, target: usize, rng: &mut R) -> Board {
        let mut puzzle = *solved;
        let mut order: Vec<usize> = (0..CELLS).collect();
        order.shuffle(rng);
        let mut holes = 0;
        for i in order {
            if holes >= target {
                break;
            }
            let backup = puzzle[i];
            puzzle[i] = 0;
            if is_unique(&puzzle) {
                holes += 1;
            } else {
                puzzle[i] = backup;
            }
        }
        puzzle
    }

    #[test]
    fn dig_matches_its_definition() {
        for seed in 0..6 {
            for target in [30, 50, CELLS] {
                let mut a = StdRng::seed_from_u64(seed);
                let mut b = StdRng::seed_from_u64(seed);
                let solved = full_solution(&mut a);
                assert_eq!(solved, full_solution(&mut b));
                assert_eq!(
                    dig(&solved, target, &mut a),
                    dig_by_definition(&solved, target, &mut b),
                    "seed {seed}, target {target}"
                );
                // Both leave the generator in the same place.
                assert_eq!(a.random::<u64>(), b.random::<u64>());
            }
        }
    }

    #[test]
    fn board_parsing_and_validation() {
        let (p, s) = test_board();
        assert_eq!(p[0], 5);
        assert_eq!(p[2], 0);
        assert_eq!(parse_board(&format_board(&p)), Some(p));
        assert_eq!(parse_board(&format_board(&s)), Some(s));
        assert_eq!(parse_board("123"), None);
        assert_eq!(parse_board(&"1".repeat(82)), None);
        assert_eq!(parse_board(&"x".repeat(81)), None);
        assert!(is_valid_solution(&s));
        assert!(!is_valid_solution(&p));
    }

    #[test]
    fn solver_spots_unsolvable() {
        // Take a solved grid, empty one cell, and block its only digit in
        // its column: the cell has zero candidates, so the solver gives up
        // immediately instead of searching a huge tree.
        let (_, solved) = test_board();
        let mut bad = solved;
        bad[idx(8, 8)] = 0; // needs 9 ...
        bad[idx(0, 8)] = 9; // ... but 9 is now used in column 8
        assert_eq!(count_solutions(&bad, 2), 0);
        assert_eq!(solve_one(&bad), None);
    }
}
