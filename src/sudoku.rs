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

/// Find the empty cell with the fewest candidates (MRV heuristic), stopping
/// early once a cell with exactly one candidate is found. `None` means the
/// board is full; a returned mask of `0` means that cell is unsolvable.
fn find_mrv_cell(board: &Board) -> Option<(usize, u16)> {
    let mut best: Option<(usize, u16)> = None;
    for i in (0..CELLS).filter(|&i| board[i] == 0) {
        let mask = candidates_mask(board, i);
        match mask.count_ones() {
            0 => return Some((i, 0)),
            1 => return Some((i, mask)),
            n if best.is_none_or(|(_, m)| n < m.count_ones()) => best = Some((i, mask)),
            _ => {}
        }
    }
    best
}

/// Count solutions up to `limit` using MRV backtracking. Fast enough for
/// uniqueness checks during generation.
pub fn count_solutions(board: &mut Board, limit: u32) -> u32 {
    let (cell, mask) = match find_mrv_cell(board) {
        None => return 1, // no empties: one solution found
        Some((_, 0)) => return 0,
        Some(cm) => cm,
    };
    let mut count = 0;
    for v in (1..=9u8).filter(|v| mask & (1 << v) != 0) {
        board[cell] = v;
        count += count_solutions(board, limit.saturating_sub(count));
        board[cell] = 0;
        if count >= limit {
            break;
        }
    }
    count
}

/// True when the puzzle has exactly one solution.
pub fn is_unique(puzzle: &Board) -> bool {
    let mut probe = *puzzle;
    count_solutions(&mut probe, 2) == 1
}

/// Solve the puzzle, returning the first solution found (if any).
pub fn solve_one(board: &Board) -> Option<Board> {
    let mut b = *board;
    solve_into(&mut b).then_some(b)
}

fn solve_into(board: &mut Board) -> bool {
    let (cell, mask) = match find_mrv_cell(board) {
        None => return true,
        Some((_, 0)) => return false,
        Some(cm) => cm,
    };
    for v in (1..=9u8).filter(|v| mask & (1 << v) != 0) {
        board[cell] = v;
        if solve_into(board) {
            return true;
        }
    }
    board[cell] = 0;
    false
}

/// Build a random complete grid with shuffled DFS.
pub fn full_solution<R: Rng>(rng: &mut R) -> Board {
    let mut board = [0u8; CELLS];
    fill_random(&mut board, rng);
    board
}

fn fill_random<R: Rng>(board: &mut Board, rng: &mut R) -> bool {
    let (cell, mask) = match find_mrv_cell(board) {
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
        board[cell] = v;
        if fill_random(board, rng) {
            return true;
        }
    }
    board[cell] = 0;
    false
}

/// Blank cells of `solved` in random order while the puzzle keeps a unique
/// solution, stopping once `target` cells are empty.
///
/// One pass is all it takes: blanking more cells never makes a puzzle more
/// constrained, so a cell that can't be removed now can't be removed later.
pub fn dig<R: Rng>(solved: &Board, target: usize, rng: &mut R) -> Board {
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
#[cfg(test)]
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
    use rand::SeedableRng;
    use rand::rngs::StdRng;

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
        let mut probe = bad;
        assert_eq!(count_solutions(&mut probe, 2), 0);
        assert_eq!(solve_one(&bad), None);
    }
}
