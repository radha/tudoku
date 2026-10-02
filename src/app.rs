//! What's on screen, and how input changes it.
//!
//! The base screen is either the title menu (no game yet) or the board.
//! Popups live on a stack of [`Overlay`]s: the top one receives every key
//! and click, and `ui::render` draws them bottom to top. Input and drawing
//! read the same stack, so they can never disagree about which popup is
//! really open.
//!
//! Keys and clicks both become an [`Action`], and [`App::apply`] is the one
//! place that decides what an action does.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::deal::Difficulty;
use crate::game::Game;
use crate::store::{Stats, Store};

/// How often a running game is saved even without edits.
const AUTOSAVE_EVERY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Help,
    /// Difficulty picker with its highlighted row.
    Levels {
        cursor: usize,
    },
    /// Board hidden, timer stopped.
    Paused,
    /// Puzzle solved. Dismissing it leaves the finished board on screen.
    Won,
}

/// Something done to the board itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardAction {
    Move(i32, i32),
    Select(usize, usize),
    Digit(u8),
    Erase,
    ToggleNotes,
    Undo,
    Hint,
}

/// Everything the player can ask for, by key or by click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Board(BoardAction),
    /// New puzzle at the current difficulty.
    NewGame,
    StartLevel(Difficulty),
    /// Pick the saved game back up.
    Continue,
    CursorUp,
    CursorDown,
    /// Enter on a menu: take the highlighted row.
    Confirm,
    OpenLevels,
    OpenHelp,
    Pause,
    /// Dismiss the top overlay: Esc, or a click outside it.
    Close,
}

/// A row of the title-screen menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRow {
    Continue,
    Level(Difficulty),
}

pub struct App {
    pub game: Option<Game>,
    pub overlays: Vec<Overlay>,
    /// Highlighted row of the title-screen menu (see [`App::menu_rows`]).
    pub menu_cursor: usize,
    pub last_level: Difficulty,
    /// A puzzle at this level is being dealt.
    pub dealing: Option<Difficulty>,
    /// A saved game, offered as "Continue" on the title screen.
    pub resume: Option<Game>,
    pub stats: Stats,
    /// The last solve set a new best time.
    pub new_best: bool,
    store: Store,
    last_saved: Instant,
}

/// The level bound to a number key on the level menus (`1` = Easy).
fn level_for_key(c: char) -> Option<Action> {
    let i = c.to_digit(10)?.checked_sub(1)?;
    let level = Difficulty::ALL.get(usize::try_from(i).ok()?)?;
    Some(Action::StartLevel(*level))
}

impl App {
    /// Start on the title screen, offering any saved game from `store`.
    pub fn new(store: Store) -> Self {
        let resume = store.load_game();
        let last_level = resume.as_ref().map_or(Difficulty::Medium, |g| g.difficulty);
        let mut app = Self {
            game: None,
            overlays: Vec::new(),
            menu_cursor: 0,
            last_level,
            dealing: None,
            resume,
            stats: store.load_stats(),
            new_best: false,
            store,
            last_saved: Instant::now(),
        };
        if app.resume.is_none() {
            app.menu_cursor = app.menu_row_of(last_level);
        }
        app
    }

    /// The title menu: "Continue" first when there's a saved game.
    pub fn menu_rows(&self) -> Vec<MenuRow> {
        let levels = Difficulty::ALL.map(MenuRow::Level);
        self.resume
            .iter()
            .map(|_| MenuRow::Continue)
            .chain(levels)
            .collect()
    }

    fn menu_row_of(&self, level: Difficulty) -> usize {
        usize::from(self.resume.is_some()) + level.index()
    }

    pub fn top(&self) -> Option<Overlay> {
        self.overlays.last().copied()
    }

    /// Translate a key press into an action for whatever has focus.
    pub fn key_action(&self, code: KeyCode, mods: KeyModifiers) -> Option<Action> {
        use KeyCode::{Char, Enter, Esc, F};

        if mods.contains(KeyModifiers::CONTROL) {
            return match code {
                Char('c' | 'q') => Some(Action::Quit),
                Char('z') if self.top().is_none() => self.base_key(Char('u')),
                _ => None,
            };
        }
        if let Char('q' | 'Q') = code {
            return Some(Action::Quit);
        }
        if self.dealing.is_some() {
            return None;
        }
        let action = match self.top() {
            None => return self.base_key(code),
            Some(Overlay::Help) => match code {
                Esc | Enter | Char(' ' | '?') | F(1) => Action::Close,
                _ => return None,
            },
            Some(Overlay::Levels { .. }) => match code {
                KeyCode::Up | Char('k') => Action::CursorUp,
                KeyCode::Down | Char('j') => Action::CursorDown,
                Enter | Char(' ') => Action::Confirm,
                Esc | Char('d') => Action::Close,
                Char(c) => return level_for_key(c),
                _ => return None,
            },
            Some(Overlay::Paused) => match code {
                Esc | Enter | Char(' ' | 'p' | 'P') => Action::Close,
                Char('?') | F(1) => Action::OpenHelp,
                _ => return None,
            },
            Some(Overlay::Won) => match code {
                Enter | Char('n' | 'N') => Action::NewGame,
                Char('d') => Action::OpenLevels,
                Esc => Action::Close,
                Char('?') | F(1) => Action::OpenHelp,
                _ => return None,
            },
        };
        Some(action)
    }

    /// Keys for the base screen, when no overlay is open.
    fn base_key(&self, code: KeyCode) -> Option<Action> {
        use BoardAction::{Digit, Erase, Hint, Move, ToggleNotes, Undo};
        use KeyCode::{Backspace, Char, Delete, Down, Enter, F, Left, Right, Up};

        let Some(game) = &self.game else {
            // Title menu.
            return match code {
                Up | Char('k') => Some(Action::CursorUp),
                Down | Char('j') => Some(Action::CursorDown),
                Enter | Char(' ') => Some(Action::Confirm),
                Char('?') | F(1) => Some(Action::OpenHelp),
                Char('c') if self.resume.is_some() => Some(Action::Continue),
                Char(c) => level_for_key(c),
                _ => None,
            };
        };
        let action = match code {
            Up | Char('k') => Action::Board(Move(-1, 0)),
            Down | Char('j') => Action::Board(Move(1, 0)),
            Left | Char('h') => Action::Board(Move(0, -1)),
            Right | Char('l') => Action::Board(Move(0, 1)),
            Char('d') => Action::OpenLevels,
            Char('?') | F(1) => Action::OpenHelp,
            Char('N') => Action::NewGame,
            // A finished board only takes navigation and "what next" keys.
            Enter | Char('n') if game.completed => Action::NewGame,
            _ if game.completed => return None,
            Char(c @ '1'..='9') => Action::Board(Digit(c as u8 - b'0')),
            Char('0' | 'x' | 'X' | 'e' | 'E') | Backspace | Delete => Action::Board(Erase),
            Char('n') => Action::Board(ToggleNotes),
            Char('u' | 'U') => Action::Board(Undo),
            Char('H') => Action::Board(Hint),
            Char('p' | 'P') => Action::Pause,
            _ => return None,
        };
        Some(action)
    }

    /// Apply an action. Returns true when the app should quit.
    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => return true,
            // Only quitting works while a puzzle is being dealt.
            _ if self.dealing.is_some() => {}
            Action::Board(b) => self.play(b),
            Action::NewGame => self.request_new_game(self.last_level),
            Action::StartLevel(level) => self.request_new_game(level),
            Action::Continue => self.continue_saved(),
            Action::CursorUp => self.move_cursor(-1),
            Action::CursorDown => self.move_cursor(1),
            Action::Confirm => self.confirm(),
            Action::OpenLevels => self.overlays.push(Overlay::Levels {
                cursor: self.last_level.index(),
            }),
            Action::OpenHelp => self.overlays.push(Overlay::Help),
            Action::Pause => self.pause(),
            Action::Close => self.close_top(),
        }
        false
    }

    fn confirm(&mut self) {
        match self.top() {
            Some(Overlay::Levels { cursor }) => self.request_new_game(Difficulty::ALL[cursor]),
            None if self.game.is_none() => match self.menu_rows()[self.menu_cursor] {
                MenuRow::Continue => self.continue_saved(),
                MenuRow::Level(level) => self.request_new_game(level),
            },
            _ => {}
        }
    }

    fn move_cursor(&mut self, delta: isize) {
        let rows = self.menu_rows().len();
        let (cursor, n) = match self.overlays.last_mut() {
            Some(Overlay::Levels { cursor }) => (cursor, Difficulty::ALL.len()),
            None if self.game.is_none() => (&mut self.menu_cursor, rows),
            _ => return,
        };
        *cursor = (*cursor as isize + delta).rem_euclid(n as isize) as usize;
    }

    fn continue_saved(&mut self) {
        if let Some(mut game) = self.resume.take() {
            // Restored games wait with the clock stopped; it runs again now.
            game.set_paused(false);
            self.last_level = game.difficulty;
            self.start_game(game);
        }
    }

    /// Save the game in progress, so quitting (or a crash, or a closed
    /// window) loses nothing. Finished games aren't kept.
    pub fn save(&mut self) {
        self.last_saved = Instant::now();
        let Some(game) = &mut self.game else { return };
        if game.completed {
            return;
        }
        if let Err(e) = self.store.save_game(game) {
            game.say(&format!("Couldn't save: {e}"));
        }
    }

    /// Every edit saves at once; this also keeps the clock and selection
    /// fresh on disk while the player is just thinking.
    pub fn autosave(&mut self) {
        let running = self
            .game
            .as_ref()
            .is_some_and(|g| !g.paused && !g.completed);
        if running && self.last_saved.elapsed() >= AUTOSAVE_EVERY {
            self.save();
        }
    }

    /// Record a solve and forget the save.
    fn finish(&mut self) {
        let Some(game) = &mut self.game else { return };
        self.new_best = self
            .stats
            .record(game.difficulty, game.elapsed(), game.hints_used);
        let saved = self
            .store
            .save_stats(&self.stats)
            .and_then(|()| self.store.clear_game());
        if let Err(e) = saved {
            game.say(&format!("Couldn't save stats: {e}"));
        }
        self.overlays.push(Overlay::Won);
    }

    fn close_top(&mut self) {
        if self.overlays.pop() == Some(Overlay::Paused)
            && let Some(g) = &mut self.game
        {
            g.set_paused(false);
        }
    }

    /// Hide the board and stop the clock, on top of whatever is open.
    pub fn pause(&mut self) {
        let Some(g) = &mut self.game else { return };
        if g.completed || g.paused || self.dealing.is_some() {
            return;
        }
        g.set_paused(true);
        self.overlays.push(Overlay::Paused);
        self.save();
    }

    fn play(&mut self, action: BoardAction) {
        if !self.overlays.is_empty() {
            return;
        }
        let Some(g) = &mut self.game else { return };
        let was_done = g.completed;
        match action {
            BoardAction::Move(dr, dc) => g.move_selection(dr, dc),
            BoardAction::Select(r, c) => g.set_selected(r, c),
            BoardAction::Digit(d) => g.enter_digit(d),
            BoardAction::Erase => g.erase(),
            BoardAction::ToggleNotes => g.toggle_notes_mode(),
            BoardAction::Undo => g.undo(),
            BoardAction::Hint => g.hint(),
        }
        if g.completed && !was_done {
            self.finish();
        } else if !matches!(action, BoardAction::Move(..) | BoardAction::Select(..)) {
            self.save();
        }
    }

    fn request_new_game(&mut self, level: Difficulty) {
        self.last_level = level;
        self.dealing = Some(level);
        self.overlays.clear();
    }

    /// Swap in a new or resumed game (replacing any older save).
    pub fn start_game(&mut self, game: Game) {
        self.game = Some(game);
        self.resume = None;
        self.dealing = None;
        self.new_best = false;
        self.overlays.clear();
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::sudoku::{CELLS, col_of, row_of, test_board};
    use crate::ui::{self, hit_at};
    use ratatui::{Terminal, backend::TestBackend};

    fn app_with_game(level: Difficulty) -> App {
        let mut app = App::new(Store::none());
        let (p, s) = test_board();
        app.start_game(Game::new(level, p, s));
        app.last_level = level;
        app
    }

    fn key(app: &mut App, code: KeyCode) -> bool {
        key_mods(app, code, KeyModifiers::NONE)
    }

    fn key_mods(app: &mut App, code: KeyCode, mods: KeyModifiers) -> bool {
        app.key_action(code, mods)
            .is_some_and(|action| app.apply(action))
    }

    /// Render like the event loop does and click at `(x, y)`.
    fn click(app: &mut App, x: u16, y: u16) {
        let screen = render(app);
        let action = hit_at(&screen.1, x, y);
        if let Some(action) = action {
            app.apply(action);
        }
    }

    fn render(app: &App) -> (String, Vec<ui::Hit>) {
        let mut term = Terminal::new(TestBackend::new(110, 44)).unwrap();
        let mut hits = Vec::new();
        term.draw(|f| hits = ui::render(f, app)).unwrap();
        let buf = term.backend().buffer();
        let text = (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, hits)
    }

    fn solve(app: &mut App) {
        let g = app.game.as_ref().unwrap();
        let cells: Vec<(usize, u8)> = (0..CELLS)
            .filter(|&i| !g.given[i])
            .map(|i| (i, g.solution[i]))
            .collect();
        for (i, d) in cells {
            app.apply(Action::Board(BoardAction::Select(row_of(i), col_of(i))));
            app.apply(Action::Board(BoardAction::Digit(d)));
        }
        assert!(app.game.as_ref().unwrap().completed);
    }

    #[test]
    fn help_opened_while_paused_is_visible_and_p_resumes_after_it() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('p'));
        key(&mut app, KeyCode::F(1));
        assert_eq!(app.top(), Some(Overlay::Help));
        assert!(render(&app).0.contains("How to play"), "help must be drawn");
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.top(), Some(Overlay::Paused), "back to the pause screen");
        key(&mut app, KeyCode::Char('p'));
        assert!(!app.game.as_ref().unwrap().paused);
        assert!(app.overlays.is_empty());
    }

    #[test]
    fn undo_cannot_unsolve_a_finished_puzzle() {
        let mut app = app_with_game(Difficulty::Easy);
        solve(&mut app);
        assert_eq!(app.top(), Some(Overlay::Won));
        key(&mut app, KeyCode::Char('u'));
        key_mods(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert!(app.game.as_ref().unwrap().completed);
        // Still true once the win popup is dismissed, by key or by button.
        key(&mut app, KeyCode::Esc);
        assert!(app.overlays.is_empty());
        key_mods(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        app.apply(Action::Board(BoardAction::Undo));
        assert!(app.game.as_ref().unwrap().completed);
    }

    #[test]
    fn finished_board_stays_viewable_and_offers_next_steps() {
        let mut app = app_with_game(Difficulty::Hard);
        solve(&mut app);
        key(&mut app, KeyCode::Esc);
        assert!(app.overlays.is_empty());
        assert!(render(&app).0.contains("Solved"), "header shows the result");
        let row = app.game.as_ref().unwrap().selected.0;
        key(&mut app, KeyCode::Down);
        let moved = app.game.as_ref().unwrap().selected.0;
        assert_eq!(moved, (row + 1) % 9, "can look around");
        key(&mut app, KeyCode::Char('d'));
        assert!(matches!(app.top(), Some(Overlay::Levels { cursor: 2 })));
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('n'));
        assert_eq!(app.dealing, Some(Difficulty::Hard));
    }

    #[test]
    fn ctrl_z_undoes_during_play() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('9')); // (0,2) is empty; 9 is wrong
        key_mods(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(app.game.as_ref().unwrap().values[2], 0);
    }

    #[test]
    fn help_closes_on_a_click_anywhere() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('?'));
        click(&mut app, 1, 42); // far corner, well outside the popup
        assert!(app.overlays.is_empty());
        key(&mut app, KeyCode::Char('?'));
        click(&mut app, 50, 22); // middle of the popup
        assert!(app.overlays.is_empty());
    }

    #[test]
    fn pause_resumes_on_a_click_anywhere() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('p'));
        click(&mut app, 10, 8); // on the hidden board
        assert!(!app.game.as_ref().unwrap().paused);
        assert!(app.overlays.is_empty());
    }

    #[test]
    fn clicks_under_a_popup_never_reach_the_board() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('p'));
        let before = app.game.as_ref().unwrap().selected;
        click(&mut app, 10, 8);
        assert_eq!(app.game.as_ref().unwrap().selected, before);
    }

    #[test]
    fn level_picker_closes_on_outside_click_but_not_inside() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('d'));
        let (_, hits) = render(&app);
        // The popup's own frame: clicking it must not dismiss the picker.
        let frame = hits
            .iter()
            .rev()
            .find(|h| h.action.is_none() && h.rect.width < 100)
            .expect("picker frame")
            .rect;
        click(&mut app, frame.x + 1, frame.y + 1);
        assert!(matches!(app.top(), Some(Overlay::Levels { .. })));
        click(&mut app, 0, 43);
        assert!(app.overlays.is_empty());
    }

    #[test]
    fn help_on_the_title_screen_captures_keys() {
        let mut app = App::new(Store::none());
        key(&mut app, KeyCode::Char('?'));
        let before = app.menu_cursor;
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Char('3'));
        assert_eq!(
            app.menu_cursor, before,
            "menu under the popup must not move"
        );
        assert_eq!(app.dealing, None, "and must not start a game");
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Down);
        assert_eq!(app.menu_cursor, before + 1);
    }

    #[test]
    fn reopening_picker_from_win_screen_shows_current_difficulty() {
        let mut app = app_with_game(Difficulty::Easy);
        solve(&mut app);
        key(&mut app, KeyCode::Char('d'));
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Esc); // cancel with a moved cursor
        assert_eq!(app.top(), Some(Overlay::Won));
        key(&mut app, KeyCode::Char('d'));
        assert_eq!(
            app.top(),
            Some(Overlay::Levels {
                cursor: Difficulty::Easy.index()
            })
        );
    }

    #[test]
    fn number_keys_and_enter_start_levels() {
        let mut app = App::new(Store::none());
        key(&mut app, KeyCode::Char('4'));
        assert_eq!(app.dealing, Some(Difficulty::Expert));

        let mut app = App::new(Store::none());
        key(&mut app, KeyCode::Up);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.dealing, Some(Difficulty::Easy));
    }

    #[test]
    fn input_is_ignored_while_dealing_except_quit() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('N'));
        assert!(app.dealing.is_some());
        let before = app.game.as_ref().unwrap().values;
        key(&mut app, KeyCode::Char('9'));
        key(&mut app, KeyCode::Char('?'));
        assert_eq!(app.game.as_ref().unwrap().values, before);
        assert!(app.overlays.is_empty());
        assert!(key(&mut app, KeyCode::Char('q')));
    }

    /// An app backed by a real (temporary) save directory.
    fn app_on_disk(dir: &tempfile::TempDir) -> App {
        App::new(Store::at(dir.path()))
    }

    #[test]
    fn quitting_mid_game_offers_continue_next_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        assert_eq!(app.menu_rows()[0], MenuRow::Level(Difficulty::Easy));
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Hard, p, s));
        key(&mut app, KeyCode::Char('9')); // a (wrong) entry at (0,2)
        key(&mut app, KeyCode::Right);
        app.save(); // what quitting does
        let before = app.game.as_ref().unwrap().values;

        let mut app = app_on_disk(&dir);
        assert!(app.game.is_none(), "starts on the title screen");
        assert_eq!(app.menu_rows()[0], MenuRow::Continue);
        assert_eq!(app.menu_cursor, 0, "Continue is preselected");
        assert_eq!(app.last_level, Difficulty::Hard);
        key(&mut app, KeyCode::Enter);
        let g = app.game.as_ref().expect("resumed");
        assert_eq!(g.values, before);
        assert_eq!(g.selected, (0, 3));
        assert_eq!(g.mistakes, 1);
        assert!(app.resume.is_none());
    }

    #[test]
    fn c_continues_and_number_keys_start_fresh_instead() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Expert, p, s));

        let mut app = app_on_disk(&dir);
        key(&mut app, KeyCode::Char('c'));
        assert_eq!(app.game.as_ref().unwrap().difficulty, Difficulty::Expert);

        let mut app = app_on_disk(&dir);
        key(&mut app, KeyCode::Down);
        assert_eq!(
            app.menu_rows()[app.menu_cursor],
            MenuRow::Level(Difficulty::Easy)
        );
        key(&mut app, KeyCode::Char('2'));
        assert_eq!(app.dealing, Some(Difficulty::Medium));
    }

    #[test]
    fn solving_records_a_best_time_and_drops_the_save() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        solve(&mut app);
        assert!(app.new_best);
        let easy = app.stats.level(Difficulty::Easy);
        assert_eq!(easy.solved, 1);
        assert!(easy.best().is_some());

        let app = app_on_disk(&dir);
        assert!(app.resume.is_none(), "a solved puzzle isn't offered again");
        assert_eq!(app.stats.level(Difficulty::Easy).solved, 1, "stats persist");
    }

    #[test]
    fn autosave_catches_up_on_moves_while_running() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        key(&mut app, KeyCode::Down); // moves aren't saved on their own
        app.autosave();
        let stored = || Store::at(dir.path()).load_game().unwrap().selected;
        assert_eq!(stored(), (0, 2), "too soon to autosave");

        let long_ago = Instant::now().checked_sub(AUTOSAVE_EVERY * 2).unwrap();
        app.last_saved = long_ago;
        app.autosave();
        assert_eq!(stored(), (1, 2));

        key(&mut app, KeyCode::Char('p'));
        key(&mut app, KeyCode::Char('?')); // help over the pause screen
        app.last_saved = long_ago;
        app.autosave();
        assert_eq!(app.last_saved, long_ago, "a paused game doesn't change");
    }

    #[test]
    fn a_hinted_solve_counts_but_sets_no_best_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        key(&mut app, KeyCode::Char('H'));
        solve(&mut app);
        assert!(!app.new_best);
        let easy = app.stats.level(Difficulty::Easy);
        assert_eq!((easy.solved, easy.best()), (1, None));
    }
}
