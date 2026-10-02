//! Mutable game state: values, notes, undo, hints, timer, mistakes.

use std::time::{Duration, Instant};

use crate::sudoku::{Board, CELLS, Difficulty, col_of, idx, peers, row_of};

/// Snapshot of one cell (plus auto-removed peer notes) for undo.
#[derive(Debug, Clone)]
struct UndoEntry {
    cell: usize,
    prev_value: u8,
    prev_notes: u16,
    prev_hinted: bool,
    peer_notes: Vec<(usize, u16)>,
}

pub struct Game {
    pub difficulty: Difficulty,
    pub solution: Board,
    pub values: Board,
    pub given: [bool; CELLS],
    pub notes: [u16; CELLS],
    pub hinted: [bool; CELLS],
    pub selected: (usize, usize),
    pub notes_mode: bool,
    pub mistakes: u32,
    pub hints_used: u32,
    pub completed: bool,
    undo: Vec<UndoEntry>,
    started: Instant,
    banked: Duration,
    pub paused: bool,
    pub status: Option<(String, Instant)>,
}

impl Game {
    pub fn new(difficulty: Difficulty, puzzle: Board, solution: Board) -> Self {
        let given = puzzle.map(|v| v != 0);
        // Start selection on the first editable cell.
        let first = (0..CELLS).find(|&i| !given[i]).unwrap_or(0);
        Self {
            difficulty,
            solution,
            values: puzzle,
            given,
            notes: [0; CELLS],
            hinted: [false; CELLS],
            selected: (row_of(first), col_of(first)),
            notes_mode: false,
            mistakes: 0,
            hints_used: 0,
            completed: false,
            undo: Vec::new(),
            started: Instant::now(),
            banked: Duration::ZERO,
            paused: false,
            status: None,
        }
    }

    pub fn selected_idx(&self) -> usize {
        idx(self.selected.0, self.selected.1)
    }

    fn select_cell(&mut self, i: usize) {
        self.selected = (row_of(i), col_of(i));
    }

    pub fn move_selection(&mut self, dr: i32, dc: i32) {
        let (r, c) = self.selected;
        let nr = (r as i32 + dr).rem_euclid(9) as usize;
        let nc = (c as i32 + dc).rem_euclid(9) as usize;
        self.selected = (nr, nc);
    }

    pub fn set_selected(&mut self, row: usize, col: usize) {
        self.selected = (row.min(8), col.min(8));
    }

    pub fn toggle_notes_mode(&mut self) {
        self.notes_mode = !self.notes_mode;
    }

    pub fn elapsed(&self) -> Duration {
        if self.paused || self.completed {
            self.banked
        } else {
            self.banked + self.started.elapsed()
        }
    }

    /// Pause or resume the timer. While paused the board is hidden.
    pub fn set_paused(&mut self, paused: bool) {
        if paused == self.paused || self.completed {
            return;
        }
        if paused {
            self.banked += self.started.elapsed();
        } else {
            self.started = Instant::now();
        }
        self.paused = paused;
    }

    fn mark_finished_if_done(&mut self) {
        if !self.completed && self.values == self.solution {
            self.completed = true;
            self.banked += self.started.elapsed();
        }
    }

    pub fn locked(&self, i: usize) -> bool {
        self.given[i] || self.hinted[i]
    }

    /// Clear `digit` from the pencil marks of every peer of `i` (same row,
    /// column, or box), returning the cleared cells' prior notes for undo.
    fn clear_peer_notes(&mut self, i: usize, digit: u8) -> Vec<(usize, u16)> {
        let bit = 1 << digit;
        let mut cleared = Vec::new();
        for p in peers(i) {
            if self.notes[p] & bit != 0 {
                cleared.push((p, self.notes[p]));
                self.notes[p] &= !bit;
            }
        }
        cleared
    }

    fn push_undo(&mut self, i: usize, peer_notes: Vec<(usize, u16)>) {
        self.undo.push(UndoEntry {
            cell: i,
            prev_value: self.values[i],
            prev_notes: self.notes[i],
            prev_hinted: self.hinted[i],
            peer_notes,
        });
    }

    /// Enter a digit: place the value, or toggle a pencil mark in notes mode.
    pub fn enter_digit(&mut self, digit: u8) {
        if self.completed || self.paused {
            return;
        }
        debug_assert!((1..=9).contains(&digit));
        let i = self.selected_idx();
        if self.locked(i) {
            self.say("Locked cell");
            return;
        }
        if self.notes_mode {
            if self.values[i] != 0 {
                self.say("Clear the value first (Eraser)");
                return;
            }
            self.push_undo(i, Vec::new());
            self.notes[i] ^= 1 << digit;
            return;
        }
        if self.values[i] == digit {
            return; // already showing this digit, no-op
        }
        // Auto-clean peer notes when a correct digit is placed.
        let correct = self.solution[i] == digit;
        let peers = if correct {
            self.clear_peer_notes(i, digit)
        } else {
            Vec::new()
        };
        self.push_undo(i, peers);
        self.values[i] = digit;
        self.notes[i] = 0;
        if !correct {
            // Mistakes are cumulative: fixing or undoing the cell later
            // must not decrement this counter.
            self.mistakes += 1;
            self.say("Not quite — that digit is wrong");
        }
        self.mark_finished_if_done();
    }

    /// Clear the selected cell (value + notes) unless it is locked.
    pub fn erase(&mut self) {
        if self.completed || self.paused {
            return;
        }
        let i = self.selected_idx();
        if self.locked(i) {
            self.say("Locked cell");
            return;
        }
        if self.values[i] == 0 && self.notes[i] == 0 {
            return;
        }
        self.push_undo(i, Vec::new());
        self.values[i] = 0;
        self.notes[i] = 0;
    }

    /// Step back one edit. A solved puzzle stays solved.
    pub fn undo(&mut self) {
        if self.paused || self.completed {
            return;
        }
        let Some(e) = self.undo.pop() else {
            self.say("Nothing to undo");
            return;
        };
        for (p, prev) in e.peer_notes {
            self.notes[p] = prev;
        }
        self.values[e.cell] = e.prev_value;
        self.notes[e.cell] = e.prev_notes;
        self.hinted[e.cell] = e.prev_hinted;
        // Do not restore mistakes: the counter is cumulative.
        self.select_cell(e.cell);
    }

    /// Reveal the correct digit for the selected cell if it is editable,
    /// otherwise the first editable wrong/empty cell. Hinted cells lock.
    pub fn hint(&mut self) {
        if self.completed || self.paused {
            return;
        }
        let s = self.selected_idx();
        let target = if !self.locked(s) && self.values[s] != self.solution[s] {
            Some(s)
        } else {
            (0..CELLS).find(|&i| !self.locked(i) && self.values[i] != self.solution[i])
        };
        let Some(i) = target else {
            self.say("Nothing to reveal");
            return;
        };
        let digit = self.solution[i];
        let peers = self.clear_peer_notes(i, digit);
        self.push_undo(i, peers);
        self.values[i] = digit;
        self.notes[i] = 0;
        self.hinted[i] = true;
        self.hints_used += 1;
        self.select_cell(i);
        self.say("Hint placed (locked)");
        self.mark_finished_if_done();
    }

    fn is_correct(&self, i: usize) -> bool {
        self.values[i] != 0 && self.values[i] == self.solution[i]
    }

    /// How many of each digit are still missing (for the number bar).
    pub fn remaining(&self) -> [u32; 10] {
        let mut out = [9u32; 10];
        out[0] = 0;
        for i in (0..CELLS).filter(|&i| self.is_correct(i)) {
            out[usize::from(self.values[i])] -= 1;
        }
        out
    }

    /// True when the current value breaks Sudoku rules or disagrees with
    /// the solution. Used for red error highlighting.
    pub fn cell_error(&self, i: usize) -> bool {
        let v = self.values[i];
        if v == 0 || self.given[i] {
            return false;
        }
        // A correct digit can still conflict if the user duplicated it
        // elsewhere (both cells show the error).
        v != self.solution[i] || peers(i).any(|p| self.values[p] == v)
    }

    pub fn progress(&self) -> (usize, usize) {
        let done = (0..CELLS).filter(|&i| self.is_correct(i)).count();
        (done, CELLS)
    }

    fn say(&mut self, msg: &str) {
        self.status = Some((msg.to_string(), Instant::now()));
    }

    /// Current transient message, if younger than `ttl`.
    pub fn message(&self, ttl: Duration) -> Option<&str> {
        self.status
            .as_ref()
            .filter(|(_, t)| t.elapsed() < ttl)
            .map(|(m, _)| m.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sudoku::test_board;

    fn test_game() -> Game {
        let (p, s) = test_board();
        Game::new(Difficulty::Easy, p, s)
    }

    fn solve_all(g: &mut Game) {
        for i in 0..CELLS {
            if !g.given[i] {
                g.set_selected(row_of(i), col_of(i));
                g.notes_mode = false;
                g.enter_digit(g.solution[i]);
            }
        }
    }

    #[test]
    fn starts_on_first_editable_cell() {
        let g = test_game();
        assert_eq!(g.selected, (0, 2));
    }

    #[test]
    fn correct_entry_is_quiet_and_wrong_counts_a_mistake() {
        let mut g = test_game();
        // First empty cell is (0,2) with solution 4.
        g.set_selected(0, 2);
        g.enter_digit(4);
        assert_eq!(g.mistakes, 0);
        assert_eq!(g.values[idx(0, 2)], 4);
        g.enter_digit(9);
        assert_eq!(g.mistakes, 1);
        assert!(g.cell_error(idx(0, 2)));
    }

    #[test]
    fn duplicate_of_a_correct_digit_flags_both_cells() {
        let mut g = test_game();
        g.set_selected(0, 2);
        g.enter_digit(4); // correct
        g.set_selected(0, 3);
        g.enter_digit(4); // wrong, and duplicates (0,2) in row 0
        assert!(g.cell_error(idx(0, 3)));
        assert!(g.cell_error(idx(0, 2)));
        assert!(!g.cell_error(idx(0, 0)), "givens never flag");
    }

    #[test]
    fn notes_toggle_and_undo_restore() {
        let mut g = test_game();
        g.set_selected(0, 2);
        g.toggle_notes_mode();
        g.enter_digit(3);
        g.enter_digit(7);
        assert_eq!(g.notes[idx(0, 2)], (1 << 3) | (1 << 7));
        g.undo();
        assert_eq!(g.notes[idx(0, 2)], 1 << 3);
        g.undo();
        assert_eq!(g.notes[idx(0, 2)], 0);
    }

    #[test]
    fn hint_fills_solution_and_locks() {
        let mut g = test_game();
        g.set_selected(0, 2);
        g.hint();
        assert_eq!(g.values[idx(0, 2)], 4);
        assert_eq!(g.hints_used, 1);
        // Locked: erase and overwrite are refused.
        g.erase();
        assert_eq!(g.values[idx(0, 2)], 4);
        g.enter_digit(9);
        assert_eq!(g.values[idx(0, 2)], 4);
    }

    #[test]
    fn repeated_wrong_digit_does_not_inflate_mistakes() {
        let mut g = test_game();
        // First empty cell is (0,2) with solution 4; 9 is wrong.
        g.set_selected(0, 2);
        g.enter_digit(9);
        g.enter_digit(9);
        g.enter_digit(9);
        assert_eq!(g.mistakes, 1, "re-entering the same wrong digit is a no-op");
    }

    #[test]
    fn mistakes_survive_undo() {
        let mut g = test_game();
        // First empty cell is (0,2) with solution 4; 9 is wrong.
        g.set_selected(0, 2);
        g.enter_digit(9);
        assert_eq!(g.mistakes, 1);
        g.undo();
        assert_eq!(g.values[idx(0, 2)], 0);
        assert_eq!(
            g.mistakes, 1,
            "undo must not erase a mistake from the count"
        );
        // Finish the puzzle correctly: the win screen must still report 1.
        solve_all(&mut g);
        assert!(g.completed);
        assert_eq!(g.mistakes, 1);
    }

    #[test]
    fn completing_every_cell_wins() {
        let mut g = test_game();
        solve_all(&mut g);
        assert!(g.completed);
        let (done, total) = g.progress();
        assert_eq!((done, total), (81, 81));
        assert_eq!(g.remaining(), [0; 10]);
    }

    #[test]
    fn remaining_counts_only_correct_placements() {
        let mut g = test_game();
        let before = g.remaining();
        g.set_selected(0, 2);
        g.enter_digit(9); // wrong: 9 still missing the same number of times
        assert_eq!(g.remaining()[9], before[9]);
        g.enter_digit(4); // correct
        assert_eq!(g.remaining()[4], before[4] - 1);
    }

    #[test]
    fn selection_wraps_around_board_edges() {
        let mut g = test_game();
        g.set_selected(0, 4);
        g.move_selection(-1, 0);
        assert_eq!(g.selected, (8, 4));
        g.move_selection(1, 0);
        assert_eq!(g.selected, (0, 4));
        g.set_selected(5, 0);
        g.move_selection(0, -1);
        assert_eq!(g.selected, (5, 8));
        g.move_selection(0, 1);
        assert_eq!(g.selected, (5, 0));
    }

    #[test]
    fn erase_and_peer_note_cleanup() {
        let mut g = test_game();
        // Pencil 4 in a peer of (0,2), then place the correct 4: peer note clears.
        g.toggle_notes_mode();
        g.set_selected(0, 0); // given cell: notes refused, stays empty
        g.enter_digit(4);
        assert_eq!(g.notes[idx(0, 0)], 0);
        g.set_selected(1, 1);
        g.enter_digit(4);
        assert_ne!(g.notes[idx(1, 1)], 0);
        g.toggle_notes_mode();
        g.set_selected(0, 2);
        g.enter_digit(4);
        assert_eq!(g.notes[idx(1, 1)] & (1 << 4), 0);
        // Undo brings the peer's pencil mark back.
        g.undo();
        assert_ne!(g.notes[idx(1, 1)] & (1 << 4), 0);
    }
}
