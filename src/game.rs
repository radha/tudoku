//! Mutable game state: values, notes, undo, hints, timer, mistakes.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::deal::Difficulty;
use crate::logic::{self, Effect, Step, Technique};
use crate::sudoku::{
    ALL_DIGITS, Board, CELLS, col_of, format_board, idx, is_unique, parse_board, peers, row_of,
    solve_one,
};
use crate::tutor::NoteMode;

/// Snapshot of one cell (plus auto-removed peer notes) for undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UndoEntry {
    cell: usize,
    prev_value: u8,
    prev_notes: u16,
    prev_hinted: bool,
    peer_notes: Vec<(usize, u16)>,
}

impl UndoEntry {
    /// Could this entry have come from playing `puzzle`?
    fn fits(&self, puzzle: &Board) -> bool {
        let notes_ok = |m: u16| m & !ALL_DIGITS == 0;
        self.cell < CELLS
            && puzzle[self.cell] == 0
            && self.prev_value <= 9
            // Hinted cells are locked, so no edit ever starts from one.
            && !self.prev_hinted
            && notes_ok(self.prev_notes)
            && self
                .peer_notes
                .iter()
                .all(|&(p, m)| p < CELLS && puzzle[p] == 0 && notes_ok(m))
    }
}

/// Everything needed to resume a game, in a stable on-disk shape. Boards
/// are 81-digit strings so a save file stays readable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedGame {
    version: u32,
    difficulty: String,
    puzzle: String,
    solution: String,
    values: String,
    notes: Vec<u16>,
    hinted: Vec<usize>,
    selected: (usize, usize),
    notes_mode: bool,
    mistakes: u32,
    hints_used: u32,
    elapsed_ms: u64,
    undo: Vec<UndoEntry>,
}

const SAVE_VERSION: u32 = 2;

pub struct Game {
    pub difficulty: Difficulty,
    pub solution: Board,
    pub values: Board,
    pub given: [bool; CELLS],
    pub notes: [u16; CELLS],
    pub hinted: [bool; CELLS],
    pub selected: (usize, usize),
    /// The digit last picked on purpose, by clicking a placed digit or
    /// typing one. It stays highlighted while you visit empty cells.
    pinned: u8,
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
            pinned: 0,
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

    /// Select a cell by clicking it. Clicking a placed digit pins its
    /// highlight; arrowing onto one only shows it while you're there, so
    /// digits you pass on the way don't steal the highlight.
    pub fn click(&mut self, row: usize, col: usize) {
        self.set_selected(row, col);
        let v = self.values[self.selected_idx()];
        if v != 0 {
            self.pinned = v;
        }
    }

    /// The digit to highlight across the board: the selected cell's own
    /// digit, else the pinned one.
    pub fn highlight(&self) -> u8 {
        match self.values[self.selected_idx()] {
            0 => self.pinned,
            v => v,
        }
    }

    pub fn toggle_notes_mode(&mut self) {
        if self.completed {
            return;
        }
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
        self.pinned = digit;
        let i = self.selected_idx();
        if self.values[i] == digit {
            return; // already showing this digit: just pins it
        }
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
            self.mistakes = self.mistakes.saturating_add(1);
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

    /// Reveal the correct digit for the selected cell if it is still open;
    /// otherwise reveal the cell a person could deduce next, and say which
    /// technique gets there. Hinted cells lock.
    pub fn hint(&mut self) {
        if self.completed || self.paused {
            return;
        }
        let s = self.selected_idx();
        let (i, msg) = if !self.locked(s) && !self.is_correct(s) {
            (s, "Hint placed".to_string())
        } else if let Some((i, _, technique)) = logic::next_placement(&self.correct_values()) {
            let msg = if technique <= Technique::HiddenSingle {
                format!("Hint: {}", technique.name())
            } else {
                format!("Hint: {}, then a single", technique.name())
            };
            (i, msg)
        } else if let Some(i) = (0..CELLS).find(|&i| !self.is_correct(i)) {
            // Logic is stuck (it shouldn't be, on a dealt puzzle).
            (i, "Hint placed".to_string())
        } else {
            self.say("Nothing to reveal");
            return;
        };
        let digit = self.solution[i];
        let peers = self.clear_peer_notes(i, digit);
        self.push_undo(i, peers);
        self.values[i] = digit;
        self.notes[i] = 0;
        self.hinted[i] = true;
        self.hints_used = self.hints_used.saturating_add(1);
        self.select_cell(i);
        self.say(&msg);
        self.mark_finished_if_done();
    }

    /// Apply the tutor's step. A placement goes in the way a correct entry
    /// does; an elimination crosses candidates out of the notes, which only
    /// makes sense when the notes are full candidates. One undo entry
    /// covers the whole step. False means nothing was applied.
    #[allow(dead_code)] // the tutor pane calls this once it is wired in
    pub fn apply_step(&mut self, step: &Step, mode: NoteMode) -> bool {
        if self.completed || self.paused {
            return false;
        }
        match &step.effect {
            Effect::Place { cell, digit } => {
                self.place_correct(*cell, *digit);
                true
            }
            Effect::Eliminate(removals) => {
                let Some(&(first, _)) = removals.first() else {
                    return false;
                };
                if mode != NoteMode::Full {
                    return false;
                }
                let others = removals[1..]
                    .iter()
                    .map(|&(cell, _)| (cell, self.notes[cell]))
                    .collect();
                self.push_undo(first, others);
                for &(cell, mask) in removals {
                    self.notes[cell] &= !mask;
                }
                true
            }
        }
    }

    /// Place the right digit in `cell`, as the tutor's "put it in for me".
    #[allow(dead_code)] // the tutor pane calls this once it is wired in
    pub fn place_answer(&mut self, cell: usize) {
        if self.completed || self.paused {
            return;
        }
        self.place_correct(cell, self.solution[cell]);
    }

    fn place_correct(&mut self, cell: usize, digit: u8) {
        self.select_cell(cell);
        let peers = self.clear_peer_notes(cell, digit);
        self.push_undo(cell, peers);
        self.values[cell] = digit;
        self.notes[cell] = 0;
        self.mark_finished_if_done();
    }

    /// The board with wrong entries blanked: what logic may build on.
    fn correct_values(&self) -> Board {
        std::array::from_fn(|i| {
            if self.is_correct(i) {
                self.values[i]
            } else {
                0
            }
        })
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

    /// Blanks filled correctly so far, out of the puzzle's blanks.
    pub fn progress(&self) -> (usize, usize) {
        let open = || (0..CELLS).filter(|&i| !self.given[i]);
        let done = open().filter(|&i| self.is_correct(i)).count();
        (done, open().count())
    }

    /// Show a transient message in the header.
    pub fn say(&mut self, msg: &str) {
        self.status = Some((msg.to_string(), Instant::now()));
    }

    /// Current transient message, if younger than `ttl`.
    pub fn message(&self, ttl: Duration) -> Option<&str> {
        self.status
            .as_ref()
            .filter(|(_, t)| t.elapsed() < ttl)
            .map(|(m, _)| m.as_str())
    }

    pub fn snapshot(&self) -> SavedGame {
        let puzzle: Board = std::array::from_fn(|i| if self.given[i] { self.values[i] } else { 0 });
        SavedGame {
            version: SAVE_VERSION,
            difficulty: self.difficulty.name().to_string(),
            puzzle: format_board(&puzzle),
            solution: format_board(&self.solution),
            values: format_board(&self.values),
            notes: self.notes.to_vec(),
            hinted: (0..CELLS).filter(|&i| self.hinted[i]).collect(),
            selected: self.selected,
            notes_mode: self.notes_mode,
            mistakes: self.mistakes,
            hints_used: self.hints_used,
            elapsed_ms: u64::try_from(self.elapsed().as_millis()).unwrap_or(u64::MAX),
            undo: self.undo.clone(),
        }
    }

    /// Rebuild a game from a save, refusing anything that isn't a
    /// consistent, unfinished, uniquely solvable puzzle.
    pub fn restore(s: SavedGame) -> Result<Game, String> {
        // Version 1 was written before the tutor, with the old level names.
        let difficulty = match s.version {
            1 => Difficulty::from_legacy_name(&s.difficulty),
            SAVE_VERSION => Difficulty::from_name(&s.difficulty),
            v => return Err(format!("unknown save version {v}")),
        }
        .ok_or_else(|| format!("unknown level {}", s.difficulty))?;
        let board = |name: &str, text: &str| {
            parse_board(text).ok_or_else(|| format!("unreadable {name} board"))
        };
        let puzzle = board("puzzle", &s.puzzle)?;
        let solution = board("solution", &s.solution)?;
        let values = board("values", &s.values)?;
        let consistent = is_unique(&puzzle)
            && solve_one(&puzzle) == Some(solution)
            && (0..CELLS).all(|i| puzzle[i] == 0 || values[i] == puzzle[i])
            && values != solution
            && s.notes.len() == CELLS
            && s.notes.iter().all(|&m| m & !ALL_DIGITS == 0)
            && s.hinted
                .iter()
                .all(|&i| i < CELLS && puzzle[i] == 0 && values[i] == solution[i])
            && s.selected.0 < 9
            && s.selected.1 < 9
            && s.undo.iter().all(|e| e.fits(&puzzle));
        if !consistent {
            return Err("inconsistent save".to_string());
        }
        let mut game = Game::new(difficulty, puzzle, solution);
        game.values = values;
        game.notes.copy_from_slice(&s.notes);
        for i in s.hinted {
            game.hinted[i] = true;
        }
        game.selected = s.selected;
        game.notes_mode = s.notes_mode;
        game.mistakes = s.mistakes;
        game.hints_used = s.hints_used;
        game.banked = Duration::from_millis(s.elapsed_ms);
        game.paused = true;
        game.undo = s.undo;
        Ok(game)
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

    #[test]
    fn apply_step_places_a_single_and_clears_peer_notes() {
        let mut g = test_game();
        let cell = (0..CELLS).find(|&i| g.values[i] == 0).unwrap();
        let digit = g.solution[cell];
        let peer = peers(cell).find(|&p| g.values[p] == 0).unwrap();
        g.notes[peer] = 1 << digit | 1 << (digit % 9 + 1);
        let before = g.notes[peer];
        let step = Step {
            technique: Technique::NakedSingle,
            effect: Effect::Place { cell, digit },
            pattern: Default::default(),
            why: String::new(),
        };
        assert!(g.apply_step(&step, NoteMode::PairMarks));
        assert_eq!(g.values[cell], digit);
        assert_eq!(g.notes[peer], before & !(1 << digit));
        assert_eq!(g.selected_idx(), cell);
        g.undo();
        assert_eq!(g.values[cell], 0);
        assert_eq!(g.notes[peer], before);
    }

    #[test]
    fn apply_step_removes_notes_in_full_mode_and_one_undo_restores_them() {
        let mut g = test_game();
        let cells: Vec<usize> = (0..CELLS).filter(|&i| g.values[i] == 0).take(2).collect();
        g.notes[cells[0]] = 0b110;
        g.notes[cells[1]] = 0b1100;
        let step = Step {
            technique: Technique::Pointing,
            effect: Effect::Eliminate(vec![(cells[0], 0b10), (cells[1], 0b100)]),
            pattern: Default::default(),
            why: String::new(),
        };
        assert!(g.apply_step(&step, NoteMode::Full));
        assert_eq!((g.notes[cells[0]], g.notes[cells[1]]), (0b100, 0b1000));
        g.undo();
        assert_eq!((g.notes[cells[0]], g.notes[cells[1]]), (0b110, 0b1100));
    }

    #[test]
    fn place_answer_enters_the_solution_digit_and_undoes() {
        let mut g = test_game();
        let cell = (0..CELLS).find(|&i| g.values[i] == 0).unwrap();
        g.place_answer(cell);
        assert_eq!(g.values[cell], g.solution[cell]);
        assert_eq!(g.mistakes, 0);
        g.undo();
        assert_eq!(g.values[cell], 0);
    }

    #[test]
    fn apply_step_refuses_eliminations_in_pair_mark_mode() {
        let mut g = test_game();
        let cell = (0..CELLS).find(|&i| g.values[i] == 0).unwrap();
        g.notes[cell] = 0b110;
        let step = Step {
            technique: Technique::Pointing,
            effect: Effect::Eliminate(vec![(cell, 0b10)]),
            pattern: Default::default(),
            why: String::new(),
        };
        assert!(!g.apply_step(&step, NoteMode::PairMarks));
        assert_eq!(g.notes[cell], 0b110);
        g.undo();
        assert_eq!(g.notes[cell], 0b110);
    }

    #[test]
    fn version_1_saves_resume_at_the_level_their_puzzle_belongs_to() {
        let g = test_game();
        for (old, new) in [
            ("Zen", Difficulty::Master),
            ("Expert", Difficulty::Hard),
            ("Medium", Difficulty::Easy),
        ] {
            let mut v = serde_json::to_value(g.snapshot()).unwrap();
            v["version"] = 1.into();
            v["difficulty"] = old.into();
            let back = Game::restore(serde_json::from_value(v).unwrap()).unwrap();
            assert_eq!(back.difficulty, new, "{old}");
        }
    }

    #[test]
    fn version_2_saves_use_current_names_and_reject_unknown_ones() {
        let mut v = serde_json::to_value(test_game().snapshot()).unwrap();
        assert_eq!(v["version"], 2);
        v["difficulty"] = "Master".into();
        let back = Game::restore(serde_json::from_value(v.clone()).unwrap()).unwrap();
        assert_eq!(back.difficulty, Difficulty::Master);
        v["difficulty"] = "Expert".into();
        assert!(Game::restore(serde_json::from_value(v).unwrap()).is_err());
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
    fn clicked_digits_pin_the_highlight_and_arrows_only_peek() {
        // Row 0 is 5 3 _ _ 7 _ _ _ _.
        let mut g = test_game();
        assert_eq!(g.highlight(), 0, "nothing picked yet");
        g.click(0, 1);
        assert_eq!(g.highlight(), 3);
        g.click(0, 2);
        assert_eq!(g.highlight(), 3, "an empty cell keeps it");
        g.move_selection(0, 2);
        assert_eq!(g.highlight(), 7, "the 7 shows while you're on it");
        g.move_selection(0, 1);
        assert_eq!(g.highlight(), 3, "but passing it doesn't steal the pin");
    }

    #[test]
    fn typed_digits_pin_the_highlight() {
        let mut g = test_game();
        g.toggle_notes_mode();
        g.set_selected(0, 2);
        g.enter_digit(2);
        g.move_selection(0, 1);
        assert_eq!(g.highlight(), 2, "a typed note pins its digit");
        g.set_selected(0, 0); // a given 5
        g.enter_digit(5);
        assert_eq!(g.status, None, "typing a cell's own digit just pins it");
        g.set_selected(0, 3);
        assert_eq!(g.highlight(), 5);
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
    fn hint_on_a_given_reveals_the_next_logical_cell() {
        let mut g = test_game();
        // A wrong entry elsewhere must not mislead the deduction.
        g.set_selected(0, 3);
        g.enter_digit(4);
        let (want, digit, _) = logic::next_placement(&g.correct_values()).unwrap();
        g.set_selected(0, 0); // a given: can't be hinted itself
        g.hint();
        assert_eq!(g.selected_idx(), want);
        assert_eq!(g.values[want], digit);
        assert!(g.hinted[want]);
        let msg = g.message(Duration::from_secs(60)).unwrap();
        assert!(msg.starts_with("Hint: "), "{msg}");
        assert!(msg.contains("single"), "{msg}");
    }

    #[test]
    fn hint_fixes_a_selected_wrong_entry() {
        let mut g = test_game();
        g.set_selected(0, 2);
        g.enter_digit(9);
        g.hint();
        assert_eq!(g.values[idx(0, 2)], 4);
        assert!(!g.cell_error(idx(0, 2)));
    }

    #[test]
    fn snapshots_round_trip_through_json() {
        let mut g = test_game();
        g.set_selected(0, 2);
        g.enter_digit(9); // wrong
        g.toggle_notes_mode();
        g.set_selected(1, 1);
        g.enter_digit(4);
        g.enter_digit(8);
        g.toggle_notes_mode();
        g.set_selected(0, 3);
        g.hint();
        let saved = g.snapshot();
        let json = serde_json::to_string(&saved).unwrap();
        let mut back = Game::restore(serde_json::from_str(&json).unwrap()).unwrap();
        assert_eq!(back.difficulty, g.difficulty);
        assert_eq!(back.values, g.values);
        assert_eq!(back.given, g.given);
        assert_eq!(back.notes, g.notes);
        assert_eq!(back.hinted, g.hinted);
        assert_eq!(back.selected, g.selected);
        assert_eq!(back.mistakes, 1);
        assert_eq!(back.hints_used, 1);
        assert!(back.paused, "restored games wait with the clock stopped");
        assert!(!back.completed);
        assert_eq!(back.elapsed(), Duration::from_millis(saved.elapsed_ms));
        back.set_paused(false);
        // Undo history survives the trip.
        for _ in 0..4 {
            back.undo();
            g.undo();
            assert_eq!(back.values, g.values);
            assert_eq!(back.notes, g.notes);
            assert_eq!(back.hinted, g.hinted);
        }
        assert_eq!(back.values[idx(0, 2)], 0, "back to the start");
    }

    #[test]
    fn restore_rejects_tampered_or_finished_saves() {
        let good = test_game().snapshot();
        assert!(Game::restore(good.clone()).is_ok());
        let tampered: [fn(&mut SavedGame); 10] = [
            |s| s.version = 99,
            |s| s.values.replace_range(0..1, "9"), // overwrites a given
            |s| s.solution.replace_range(0..2, "35"),
            |s| s.puzzle = "0".repeat(81),     // not unique
            |s| s.values = s.solution.clone(), // already solved
            |s| s.notes[2] = 1,                // bit 0 isn't a digit
            |s| s.hinted = vec![0],            // can't hint a given
            |s| s.selected = (9, 0),
            // Undo history that could never come from play: un-hinting into
            // an empty locked cell, or editing a given.
            |s| {
                s.undo.push(UndoEntry {
                    cell: 2,
                    prev_value: 0,
                    prev_notes: 0,
                    prev_hinted: true,
                    peer_notes: Vec::new(),
                })
            },
            |s| {
                s.undo.push(UndoEntry {
                    cell: 0,
                    prev_value: 0,
                    prev_notes: 0,
                    prev_hinted: false,
                    peer_notes: Vec::new(),
                })
            },
        ];
        for (n, tamper) in tampered.iter().enumerate() {
            let mut s = good.clone();
            tamper(&mut s);
            assert!(Game::restore(s).is_err(), "tampering #{n} slipped through");
        }
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
        assert_eq!((done, total), (51, 51), "counts the 51 blanks, not givens");
        assert_eq!(g.remaining(), [0; 10]);
        // A solved board is frozen: not even notes mode changes.
        g.toggle_notes_mode();
        assert!(!g.notes_mode);
    }

    #[test]
    fn counters_saturate_instead_of_wrapping() {
        let mut g = test_game();
        g.mistakes = u32::MAX;
        g.hints_used = u32::MAX;
        g.set_selected(0, 2);
        g.enter_digit(9);
        g.hint();
        assert_eq!((g.mistakes, g.hints_used), (u32::MAX, u32::MAX));
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
