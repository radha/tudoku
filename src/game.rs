//! Mutable game state: values, notes, undo, hints, timer, mistakes.

use std::time::{Duration, Instant};

use crate::sudoku::{self, CELLS, Difficulty, idx};

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
    pub solution: [u8; CELLS],
    pub values: [u8; CELLS],
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
    pub fn new(difficulty: Difficulty, puzzle: [u8; CELLS], solution: [u8; CELLS]) -> Self {
        let mut values = [0u8; CELLS];
        let mut given = [false; CELLS];
        for i in 0..CELLS {
            values[i] = puzzle[i];
            given[i] = puzzle[i] != 0;
        }
        // Start selection on the first editable cell.
        let mut selected = (0, 0);
        for r in 0..9 {
            for c in 0..9 {
                if !given[idx(r, c)] {
                    selected = (r, c);
                    break;
                }
            }
            if !given[idx(selected.0, selected.1)] {
                break;
            }
        }
        Self {
            difficulty,
            solution,
            values,
            given,
            notes: [0; CELLS],
            hinted: [false; CELLS],
            selected,
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
        if paused == self.paused {
            return;
        }
        if paused {
            self.banked += self.started.elapsed();
            self.paused = true;
        } else {
            self.started = Instant::now();
            self.paused = false;
        }
    }

    fn mark_finished_if_done(&mut self) {
        if !self.completed
            && (0..CELLS).all(|i| self.values[i] == self.solution[i] && self.values[i] != 0)
        {
            self.completed = true;
            self.banked += self.started.elapsed();
        }
    }

    pub fn locked(&self, i: usize) -> bool {
        self.given[i] || self.hinted[i]
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
            if !self.given[i] {
                // hinted cells stay as-is
            }
            return;
        }
        if self.notes_mode {
            if self.values[i] != 0 {
                self.say("Clear the value first (Eraser)");
                return;
            }
            let prev = self.notes[i];
            let next = prev ^ (1 << digit);
            self.undo.push(UndoEntry {
                cell: i,
                prev_value: self.values[i],
                prev_notes: prev,
                prev_hinted: self.hinted[i],
                peer_notes: Vec::new(),
            });
            self.notes[i] = next;
            return;
        }
        if self.values[i] == digit && self.solution[i] == digit {
            return; // already correct, no-op
        }
        let mut peers = Vec::new();
        // Auto-clean peer notes when a correct digit is placed.
        let correct = self.solution[i] == digit;
        if correct {
            for p in 0..CELLS {
                if p != i && self.notes[p] & (1 << digit) != 0 && sudoku::shares_house(i, p) {
                    peers.push((p, self.notes[p]));
                    self.notes[p] &= !(1 << digit);
                }
            }
        }
        self.undo.push(UndoEntry {
            cell: i,
            prev_value: self.values[i],
            prev_notes: self.notes[i],
            prev_hinted: self.hinted[i],
            peer_notes: peers,
        });
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
        self.undo.push(UndoEntry {
            cell: i,
            prev_value: self.values[i],
            prev_notes: self.notes[i],
            prev_hinted: self.hinted[i],
            peer_notes: Vec::new(),
        });
        self.values[i] = 0;
        self.notes[i] = 0;
    }

    pub fn undo(&mut self) {
        if self.paused {
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
        if self.completed {
            self.completed = false;
            self.started = Instant::now();
        }
        self.selected = (e.cell / 9, e.cell % 9);
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
        let mut peers = Vec::new();
        for p in 0..CELLS {
            if p != i && self.notes[p] & (1 << digit) != 0 && sudoku::shares_house(i, p) {
                peers.push((p, self.notes[p]));
                self.notes[p] &= !(1 << digit);
            }
        }
        self.undo.push(UndoEntry {
            cell: i,
            prev_value: self.values[i],
            prev_notes: self.notes[i],
            prev_hinted: self.hinted[i],
            peer_notes: peers,
        });
        self.values[i] = digit;
        self.notes[i] = 0;
        self.hinted[i] = true;
        self.hints_used += 1;
        self.selected = (i / 9, i % 9);
        self.say("Hint placed (locked)");
        self.mark_finished_if_done();
    }

    /// How many of each digit are still missing (for the number bar).
    pub fn remaining(&self) -> [u32; 10] {
        let mut out = [0u32; 10];
        for (d, slot) in out.iter_mut().enumerate().skip(1) {
            let digit = d as u8;
            let placed = (0..CELLS)
                .filter(|&i| self.values[i] == digit && self.values[i] == self.solution[i])
                .count() as u32;
            *slot = 9 - placed;
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
        if v != self.solution[i] {
            return true;
        }
        // Correct digit can still conflict if the user duplicated it
        // elsewhere (both cells show the error).
        let (r, c) = (i / 9, i % 9);
        for k in 0..9 {
            let a = idx(r, k);
            let b = idx(k, c);
            if a != i && self.values[a] == v {
                return true;
            }
            if b != i && self.values[b] == v {
                return true;
            }
        }
        let br = (r / 3) * 3;
        let bc = (c / 3) * 3;
        for dr in 0..3 {
            for dc in 0..3 {
                let a = idx(br + dr, bc + dc);
                if a != i && self.values[a] == v {
                    return true;
                }
            }
        }
        false
    }

    pub fn progress(&self) -> (usize, usize) {
        let done = (0..CELLS)
            .filter(|&i| self.values[i] != 0 && self.values[i] == self.solution[i])
            .count();
        (done, CELLS)
    }

    #[cfg(test)]
    pub fn test_board() -> ([u8; CELLS], [u8; CELLS]) {
        // Classic fixed puzzle + solution so tests never pay generation cost.
        let puzzle =
            "530070000600195000098000060800060003400803001700020006060000280000419005000080079";
        let solution =
            "534678912672195348198342567859761423426853791713924856961537284287419635345286179";
        let mut p = [0u8; CELLS];
        let mut s = [0u8; CELLS];
        for (i, (a, b)) in puzzle.chars().zip(solution.chars()).enumerate() {
            p[i] = a.to_digit(10).unwrap() as u8;
            s[i] = b.to_digit(10).unwrap() as u8;
        }
        (p, s)
    }

    fn say(&mut self, msg: &str) {
        self.status = Some((msg.to_string(), Instant::now()));
    }

    /// Current transient message, if younger than `ttl`.
    pub fn message(&self, ttl: Duration) -> Option<&str> {
        self.status.as_ref().and_then(|(m, t)| {
            if t.elapsed() < ttl {
                Some(m.as_str())
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sudoku::Difficulty;

    fn test_game() -> Game {
        let (p, s) = Game::test_board();
        Game::new(Difficulty::Easy, p, s)
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
        for i in 0..CELLS {
            if !g.given[i] {
                g.set_selected(i / 9, i % 9);
                g.notes_mode = false;
                g.enter_digit(g.solution[i]);
            }
        }
        assert!(g.completed);
        assert_eq!(g.mistakes, 1);
    }

    #[test]
    fn completing_every_cell_wins() {
        let mut g = test_game();
        for i in 0..CELLS {
            if !g.given[i] {
                g.set_selected(i / 9, i % 9);
                g.notes_mode = false;
                g.enter_digit(g.solution[i]);
            }
        }
        assert!(g.completed);
        let (done, total) = g.progress();
        assert_eq!((done, total), (81, 81));
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
    }
}
