//! What's on screen, and how input changes it.
//!
//! The base screen is the title menu (no game yet), the techniques
//! library in its place, or the board.
//! Popups live on a stack of [`Overlay`]s: the top one receives every key
//! and click, and `ui::render` draws them bottom to top. Input and drawing
//! read the same stack, so they can never disagree about which popup is
//! really open.
//!
//! Keys and clicks both become an [`Action`], and [`App::apply`] is the one
//! place that decides what an action does.
//!
//! The tutor is not a popup: its pane sits beside a board that stays in
//! play, so the board keys keep working while it is open.

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::deal::Difficulty;
use crate::game::Game;
use crate::library::{self, Library};
use crate::logic::Technique;
use crate::store::{Stats, Store};
use crate::sudoku::{col_of, row_of};
use crate::tutor::{self, Advice, NoteMode, rules};

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
    /// "Are you sure?" before throwing away a game's progress.
    Confirm(Ask),
    /// Solves, best and average times and streaks, per level.
    Stats,
}

/// Something that loses work, asked about first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Deal a new puzzle at this level in place of the game so far.
    NewGame(Difficulty),
    /// Clear the board back to its givens.
    Restart,
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
    Redo,
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
    /// Clear the board back to its givens (after asking).
    Restart,
    /// Leave the game, saved, for the title menu.
    ToTitle,
    /// Bring back the "Solved!" popup over a finished board.
    ShowResult,
    OpenStats,
    CursorUp,
    CursorDown,
    /// Enter on a menu: take the highlighted row.
    Confirm,
    OpenLevels,
    OpenHelp,
    Pause,
    /// Dismiss the top overlay: Esc, or a click outside it.
    Close,
    /// Open the tutor, or take it one stage deeper.
    Tutor,
    /// Let the tutor do what it advises.
    TutorApply,
    CloseTutor,
    /// Step the worked example back (-1) or on (+1).
    ExampleStep(i8),
    /// Show the library's map, from the title menu.
    OpenLibrary,
    /// Move the map's highlight `(rows, columns)`.
    LibraryMove(i8, i8),
    /// Show a technique's lesson.
    LibraryOpen(Technique),
    /// Go to the previous (-1) or next (+1) technique's lesson.
    LibraryTurn(i8),
    /// From a lesson to the map; from the map to the title menu.
    LibraryBack,
}

/// How far the tutor has gone with its advice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TutorStage {
    /// Notes aren't filled in yet: a word to try first, which names
    /// nothing and so costs nothing.
    TryFirst,
    /// Only the strategy's name, so you can hunt for it yourself.
    Nudge,
    /// The strategy in plain steps, and a worked example on a sample
    /// board, at one of its `EXAMPLE_STAGES`.
    Explain { stage: u8 },
    /// The step itself, on your own board.
    ShowMe,
}

/// The worked example's stages: what's involved, then the links, then
/// the result.
pub const EXAMPLE_STAGES: u8 = 3;

/// The tutor's pane: what it made of the board when it was opened, and
/// how far it has gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tutor {
    pub mode: NoteMode,
    pub advice: Advice,
    pub stage: TutorStage,
}

impl Tutor {
    /// Whether `a` can do it for you: place a single or clear the
    /// eliminated notes once the step is shown, or place one digit when
    /// no strategy applies. Eliminations need full notes to clear.
    pub fn can_apply(&self) -> bool {
        self.stage != TutorStage::TryFirst
            && rules::can_apply(self.mode, &self.advice, self.stage == TutorStage::ShowMe)
    }
}

/// A row of the title-screen menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRow {
    Continue,
    Level(Difficulty),
    Library,
    Stats,
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
    /// The tutor's pane, while it is open.
    pub tutor: Option<Tutor>,
    /// The techniques library, while it is on screen instead of the title
    /// menu. Only reachable from the title menu.
    pub library: Option<Library>,
    store: Store,
    last_saved: Instant,
    /// Whether the terminal window has focus, as far as we know.
    focused: bool,
    /// Whether the screen is big enough to show the game.
    fits: bool,
    /// Saving has failed and the player has been told.
    storage_failing: bool,
}

/// Keys in the library: the map's arrows, or a lesson's steps and pages.
fn library_key(library: Library, code: KeyCode) -> Option<Action> {
    use KeyCode::{Char, Down, Enter, Esc, F, Left, Right, Up};
    use Library::{Lesson, Map};

    let action = match (library, code) {
        (_, Esc) => Action::LibraryBack,
        (_, Char('?') | F(1)) => Action::OpenHelp,
        (Map { .. }, Up | Char('k')) => Action::LibraryMove(-1, 0),
        (Map { .. }, Down | Char('j')) => Action::LibraryMove(1, 0),
        (Map { .. }, Left | Char('h')) => Action::LibraryMove(0, -1),
        (Map { .. }, Right | Char('l')) => Action::LibraryMove(0, 1),
        (Map { at }, Enter | Char(' ')) => Action::LibraryOpen(at),
        (Lesson { .. }, Left | Char('h')) => Action::ExampleStep(-1),
        (Lesson { .. }, Right | Char('l')) => Action::ExampleStep(1),
        (Lesson { .. }, Up | Char('k')) => Action::LibraryTurn(-1),
        (Lesson { .. }, Down | Char('j')) => Action::LibraryTurn(1),
        _ => return None,
    };
    Some(action)
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
        let last_level = resume.as_ref().map_or(Difficulty::Easy, |g| g.difficulty);
        let mut app = Self {
            game: None,
            overlays: Vec::new(),
            menu_cursor: 0,
            last_level,
            dealing: None,
            resume,
            stats: store.load_stats().ok().flatten().unwrap_or_default(),
            new_best: false,
            tutor: None,
            library: None,
            store,
            last_saved: Instant::now(),
            focused: true,
            fits: true,
            storage_failing: false,
        };
        if app.resume.is_none() {
            app.menu_cursor = app.menu_row_of(last_level);
        }
        app
    }

    /// The title menu: "Continue" first when there's a saved game, the
    /// library and stats last.
    pub fn menu_rows(&self) -> Vec<MenuRow> {
        let levels = Difficulty::ALL.map(MenuRow::Level);
        self.resume
            .iter()
            .map(|_| MenuRow::Continue)
            .chain(levels)
            .chain([MenuRow::Library, MenuRow::Stats])
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
                Char('z') if self.fits => Some(Action::Board(BoardAction::Undo)),
                Char('r' | 'y') if self.fits => Some(Action::Board(BoardAction::Redo)),
                _ => None,
            };
        }
        if let Char('q' | 'Q') = code {
            return Some(Action::Quit);
        }
        // Nothing else while dealing, or while the screen is too small to
        // show what a key would do.
        if self.dealing.is_some() || !self.fits {
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
                Char('m') => Action::ToTitle,
                Esc => Action::Close,
                Char('?') | F(1) => Action::OpenHelp,
                _ => return None,
            },
            Some(Overlay::Confirm(_)) => match code {
                Enter | Char('y' | 'Y') => Action::Confirm,
                Esc | Char('n' | 'N') => Action::Close,
                _ => return None,
            },
            Some(Overlay::Stats) => match code {
                Esc | Enter | Char(' ' | '7') => Action::Close,
                _ => return None,
            },
        };
        Some(action)
    }

    /// Keys for the base screen, when no overlay is open.
    fn base_key(&self, code: KeyCode) -> Option<Action> {
        use BoardAction::{Digit, Erase, Move, Redo, ToggleNotes, Undo};
        use KeyCode::{Backspace, Char, Delete, Down, Enter, Esc, F, Left, Right, Up};

        let Some(game) = &self.game else {
            if let Some(library) = self.library {
                return library_key(library, code);
            }
            // Title menu.
            return match code {
                Up | Char('k') => Some(Action::CursorUp),
                Down | Char('j') => Some(Action::CursorDown),
                Enter | Char(' ') => Some(Action::Confirm),
                Char('?') | F(1) => Some(Action::OpenHelp),
                Char('c') if self.resume.is_some() => Some(Action::Continue),
                Char('6') => Some(Action::OpenLibrary),
                Char('7') => Some(Action::OpenStats),
                Char(c) => level_for_key(c),
                _ => None,
            };
        };
        let explaining = self.explaining();
        let action = match code {
            Left | Char('h') if explaining => Action::ExampleStep(-1),
            Right | Char('l') if explaining => Action::ExampleStep(1),
            Up | Char('k') => Action::Board(Move(-1, 0)),
            Down | Char('j') => Action::Board(Move(1, 0)),
            Left | Char('h') => Action::Board(Move(0, -1)),
            Right | Char('l') => Action::Board(Move(0, 1)),
            Char('d') => Action::OpenLevels,
            Char('?') | F(1) => Action::OpenHelp,
            Char('N') => Action::NewGame,
            Char('m') => Action::ToTitle,
            // A finished board only takes navigation and "what next" keys.
            Enter | Char('n') if game.completed => Action::NewGame,
            Char('w') if game.completed => Action::ShowResult,
            _ if game.completed => return None,
            Char(c @ '1'..='9') => Action::Board(Digit(c as u8 - b'0')),
            Char('0' | 'x' | 'X' | 'e' | 'E') | Backspace | Delete => Action::Board(Erase),
            Char('n') => Action::Board(ToggleNotes),
            Char('u' | 'U') => Action::Board(Undo),
            Char('r') => Action::Board(Redo),
            Char('R') => Action::Restart,
            Char('t') => Action::Tutor,
            Char('a') if self.tutor.is_some() => Action::TutorApply,
            Esc if self.tutor.is_some() => Action::CloseTutor,
            Char('p' | 'P') => Action::Pause,
            _ => return None,
        };
        // The board on screen is an example: yours waits for you.
        if explaining && matches!(action, Action::Board(_) | Action::Restart) {
            return None;
        }
        Some(action)
    }

    /// Whether the tutor is showing its worked example in place of
    /// your board.
    fn explaining(&self) -> bool {
        self.tutor
            .as_ref()
            .is_some_and(|t| matches!(t.stage, TutorStage::Explain { .. }))
    }

    /// Apply an action. Returns true when the app should quit.
    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => return true,
            // Only quitting works while a puzzle is being dealt.
            _ if self.dealing.is_some() => {}
            Action::Board(b) => self.play(b),
            Action::NewGame => self.new_game(self.last_level),
            Action::StartLevel(level) => self.new_game(level),
            Action::Continue => self.continue_saved(),
            Action::Restart => self.ask_restart(),
            Action::ToTitle => self.back_to_title(),
            Action::ShowResult => self.show_result(),
            Action::OpenStats => {
                if self.game.is_none() && self.library.is_none() {
                    self.overlays.push(Overlay::Stats);
                }
            }
            Action::CursorUp => self.move_cursor(-1),
            Action::CursorDown => self.move_cursor(1),
            Action::Confirm => self.confirm(),
            Action::OpenLevels => self.overlays.push(Overlay::Levels {
                cursor: self.last_level.index(),
            }),
            Action::OpenHelp => self.overlays.push(Overlay::Help),
            Action::Pause => self.pause(),
            Action::Close => self.close_top(),
            Action::Tutor => self.ask_tutor(),
            Action::TutorApply => self.apply_tutor(),
            Action::CloseTutor => self.tutor = None,
            Action::ExampleStep(delta) => self.step_example(delta),
            Action::OpenLibrary
            | Action::LibraryMove(..)
            | Action::LibraryOpen(_)
            | Action::LibraryTurn(_)
            | Action::LibraryBack => self.browse(action),
        }
        false
    }

    fn confirm(&mut self) {
        match self.top() {
            Some(Overlay::Levels { cursor }) => self.new_game(Difficulty::ALL[cursor]),
            Some(Overlay::Confirm(ask)) => {
                self.overlays.pop();
                match ask {
                    Ask::NewGame(level) => {
                        self.give_up();
                        self.request_new_game(level);
                    }
                    Ask::Restart => self.restart(),
                }
            }
            None if self.game.is_none() => match self.menu_rows()[self.menu_cursor] {
                MenuRow::Continue => self.continue_saved(),
                MenuRow::Level(level) => self.new_game(level),
                MenuRow::Library => self.browse(Action::OpenLibrary),
                MenuRow::Stats => self.overlays.push(Overlay::Stats),
            },
            _ => {}
        }
    }

    /// The unfinished game a new puzzle would replace, if it has anything
    /// on it worth asking about: the one on screen, or on the title menu
    /// the saved one.
    pub fn at_stake(&self) -> Option<&Game> {
        let game = match (&self.game, &self.resume) {
            (Some(game), _) | (None, Some(game)) => game,
            (None, None) => return None,
        };
        (!game.completed && game.has_progress()).then_some(game)
    }

    /// Deal a new puzzle at `level`, first asking if that throws away a
    /// game in progress.
    fn new_game(&mut self, level: Difficulty) {
        if self.at_stake().is_some() {
            if let Some(Overlay::Levels { .. }) = self.top() {
                self.overlays.pop();
            }
            self.overlays.push(Overlay::Confirm(Ask::NewGame(level)));
        } else {
            self.request_new_game(level);
        }
    }

    /// The game in progress is being dropped for a new one: that ends its
    /// level's streak.
    fn give_up(&mut self) {
        if let Some(level) = self.at_stake().map(|g| g.difficulty) {
            let ((), saved) = self.update_stats(|stats| stats.abandon(level));
            self.report(saved, "Couldn't save stats");
        }
    }

    /// Change the stats, building on what's on disk now: another running
    /// tudoku may have recorded solves since we loaded. If the file can't
    /// be read, the change stays in memory, since writing would replace
    /// records we never saw.
    fn update_stats<T>(&mut self, change: impl FnOnce(&mut Stats) -> T) -> (T, io::Result<()>) {
        let read = self.store.load_stats().map(|on_disk| {
            if let Some(stats) = on_disk {
                self.stats = stats;
            }
        });
        let out = change(&mut self.stats);
        let saved = read.and_then(|()| self.store.save_stats(&self.stats));
        (out, saved)
    }

    /// `R`: ask before clearing the board, if there's anything on it.
    fn ask_restart(&mut self) {
        let Some(g) = &mut self.game else { return };
        if g.completed || g.paused || !self.overlays.is_empty() {
            return;
        }
        if g.has_progress() {
            self.overlays.push(Overlay::Confirm(Ask::Restart));
        } else {
            g.say("Nothing to clear yet");
        }
    }

    fn restart(&mut self) {
        let Some(g) = &mut self.game else { return };
        if g.restart() {
            self.tutor = None;
            self.save();
        }
    }

    /// `m`: back to the title menu, where the game waits as "Continue"
    /// with its clock stopped. A finished game is simply left.
    fn back_to_title(&mut self) {
        let Some(g) = &mut self.game else { return };
        g.set_paused(true);
        self.save();
        let game = self.game.take().filter(|g| !g.completed);
        self.resume = game;
        self.tutor = None;
        self.overlays.clear();
        self.menu_cursor = if self.resume.is_some() {
            0
        } else {
            self.menu_row_of(self.last_level)
        };
    }

    /// `w`: the "Solved!" popup again, after Esc put it away.
    fn show_result(&mut self) {
        if self.game.as_ref().is_some_and(|g| g.completed) && self.overlays.is_empty() {
            self.overlays.push(Overlay::Won);
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
        let saved = self.write_game();
        self.report(saved, "Couldn't save");
    }

    /// The final save on the way out. Its error is the caller's to show,
    /// since by then the screen is gone.
    pub fn save_on_exit(&self) -> io::Result<()> {
        self.write_game()
    }

    fn write_game(&self) -> io::Result<()> {
        match &self.game {
            Some(game) if !game.completed => self.store.save_game(game),
            _ => Ok(()),
        }
    }

    /// Tell the player when storage starts failing, once, rather than
    /// drowning out every move's feedback with the same error.
    fn report(&mut self, result: io::Result<()>, what: &str) {
        match result {
            Ok(()) => self.storage_failing = false,
            Err(_) if self.storage_failing => {}
            Err(e) => {
                self.storage_failing = true;
                if let Some(game) = &mut self.game {
                    game.say(&format!("{what}: {e}"));
                }
            }
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
        self.tutor = None;
        let Some(game) = &self.game else { return };
        let (level, time, helped) = (game.difficulty, game.elapsed(), game.tutor_used);
        let (new_best, stats_saved) = self.update_stats(|stats| stats.record(level, time, helped));
        self.new_best = new_best;
        // Clear the save even if the stats can't be written: a solved
        // puzzle must never come back as "Continue".
        let cleared = self.store.clear_game();
        self.report(stats_saved.and(cleared), "Couldn't save stats");
        self.overlays.push(Overlay::Won);
    }

    fn close_top(&mut self) {
        let closed = self.overlays.pop();
        let Some(g) = &mut self.game else { return };
        match closed {
            Some(Overlay::Paused) => g.set_paused(false),
            Some(Overlay::Won) => g.say("w shows the result again"),
            _ => {}
        }
    }

    /// Whether the screen can show the game. A screen too small to show it
    /// pauses it, since you can't see what the clock is timing.
    pub fn set_fits(&mut self, fits: bool) {
        self.fits = fits;
        if !fits {
            self.pause();
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

    /// The terminal window gained or lost focus. Walking away pauses the
    /// game, so the clock never runs while you're elsewhere.
    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.pause();
        }
    }

    fn play(&mut self, action: BoardAction) {
        if !self.overlays.is_empty() || self.explaining() {
            return;
        }
        let Some(g) = &mut self.game else { return };
        let was_done = g.completed;
        // Whether the game changed, so there's something to save.
        let edit = match action {
            BoardAction::Move(dr, dc) => {
                g.move_selection(dr, dc);
                false
            }
            BoardAction::Select(r, c) => {
                g.click(r, c);
                false
            }
            BoardAction::Digit(d) => g.enter_digit(d),
            BoardAction::Erase => g.erase(),
            BoardAction::ToggleNotes => g.toggle_notes_mode(),
            BoardAction::Undo => g.undo(),
            BoardAction::Redo => g.redo(),
        };
        let solved = g.completed && !was_done;
        if edit {
            self.drop_stale_tutor();
        }
        if solved {
            self.finish();
        } else if edit {
            self.save();
        }
    }

    /// `t`: open the tutor on a nudge, or take it one stage deeper: the
    /// worked example, then the step on your board. Each fresh opening
    /// counts against the puzzle's best time.
    fn ask_tutor(&mut self) {
        if !self.overlays.is_empty() {
            return;
        }
        let Some(g) = &mut self.game else { return };
        if g.completed || g.paused {
            return;
        }
        if let Some(t) = &mut self.tutor {
            match (&t.advice, t.stage) {
                // Asked again after the word to try first: that's help.
                (_, TutorStage::TryFirst) => {
                    t.stage = TutorStage::Nudge;
                    g.use_tutor();
                    self.save();
                }
                (Advice::Step(_), TutorStage::Nudge) if rules::has_example(&t.advice) => {
                    t.stage = TutorStage::Explain { stage: 0 };
                }
                (Advice::Step(_), _) => t.stage = TutorStage::ShowMe,
                _ => {}
            }
            return;
        }
        let (mode, advice) = tutor::consult(g);
        // The tutor points at the cell it is talking about.
        if let Advice::WrongDigit(i) | Advice::WrongNotes(i) = advice {
            g.set_selected(row_of(i), col_of(i));
        }
        // With notes still to fill in, there's more to try alone first.
        // A wrong digit is always worth hearing about at once.
        let try_first = rules::starts_with_try_first(mode, &advice);
        self.tutor = Some(Tutor {
            mode,
            advice,
            stage: if try_first {
                TutorStage::TryFirst
            } else {
                TutorStage::Nudge
            },
        });
        if !try_first {
            g.use_tutor();
            self.save();
        }
    }

    /// `←` / `→`: step the worked example back or on, within its stages,
    /// in the tutor or in a library lesson.
    fn step_example(&mut self, delta: i8) {
        if !self.overlays.is_empty() {
            return;
        }
        let stage = match (&mut self.tutor, &mut self.library) {
            (
                Some(Tutor {
                    stage: TutorStage::Explain { stage },
                    ..
                }),
                _,
            ) => stage,
            (None, Some(Library::Lesson { stage, .. })) => stage,
            _ => return,
        };
        *stage = stage.saturating_add_signed(delta).min(EXAMPLE_STAGES - 1);
    }

    /// Find your way about the library: open it from the title menu, move
    /// over the map, open a lesson, turn its pages, and back out. Each
    /// lesson starts at the example's first stage.
    fn browse(&mut self, action: Action) {
        if !self.overlays.is_empty() || self.game.is_some() {
            return;
        }
        use Library::{Lesson, Map};
        self.library = match (self.library, action) {
            (None, Action::OpenLibrary) => Some(Map {
                at: Technique::FullHouse,
            }),
            (Some(Map { at }), Action::LibraryMove(dr, dc)) => Some(Map {
                at: library::map_move(at, dr, dc),
            }),
            (Some(Map { .. }), Action::LibraryOpen(at)) => Some(Lesson { at, stage: 0 }),
            // Past the first or last technique, nothing moves.
            (Some(Lesson { at, stage }), Action::LibraryTurn(delta)) => {
                let next = library::turn(at, delta);
                let stage = if next == at { stage } else { 0 };
                Some(Lesson { at: next, stage })
            }
            (Some(Lesson { at, .. }), Action::LibraryBack) => Some(Map { at }),
            (Some(Map { .. }), Action::LibraryBack) => None,
            (library, _) => library,
        };
    }

    /// `a`: do what the tutor advises, then put the tutor away.
    fn apply_tutor(&mut self) {
        if !self.overlays.is_empty() {
            return;
        }
        let (Some(g), Some(t)) = (&mut self.game, &self.tutor) else {
            return;
        };
        if g.completed || g.paused {
            return;
        }
        if !t.can_apply() {
            g.say(match (&t.advice, t.stage) {
                (Advice::WrongDigit(_) | Advice::WrongNotes(_), _) => "Fix that first",
                (Advice::Step(_), TutorStage::ShowMe) => {
                    "a clears notes once every empty cell has them"
                }
                _ => "t until the tutor shows the step, then a",
            });
            return;
        }
        if !rules::apply(g, t.mode, &t.advice) {
            return;
        }
        let done = g.completed;
        self.tutor = None;
        if done {
            self.finish();
        } else {
            self.save();
        }
    }

    /// After an edit, close the tutor if the board no longer reads the way
    /// it did when the tutor was opened.
    fn drop_stale_tutor(&mut self) {
        let (Some(g), Some(t)) = (&self.game, &self.tutor) else {
            return;
        };
        if rules::is_stale(g, t.mode, &t.advice) {
            self.tutor = None;
        }
    }

    fn request_new_game(&mut self, level: Difficulty) {
        self.tutor = None;
        self.last_level = level;
        self.dealing = Some(level);
        self.overlays.clear();
    }

    /// Swap in a new or resumed game (replacing any older save).
    pub fn start_game(&mut self, game: Game) {
        self.game = Some(game);
        self.tutor = None;
        self.library = None;
        self.resume = None;
        self.dealing = None;
        self.new_best = false;
        self.overlays.clear();
        self.save();
        // Dealt while the player was in another window, or with the
        // window too small to show it: wait for them.
        if !self.focused || !self.fits {
            self.pause();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{Effect, Grid, Step};
    use crate::store::Store;
    use crate::sudoku::{Board, CELLS, idx, parse_board, solve_one, test_board};
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
        assert_eq!(app.dealing, Some(Difficulty::Master));

        let mut app = App::new(Store::none());
        key(&mut app, KeyCode::Char('5'));
        assert_eq!(app.dealing, Some(Difficulty::Zen));

        let mut app = App::new(Store::none());
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.dealing, Some(Difficulty::Medium));
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
        app.start_game(Game::new(Difficulty::Hard, p, s));

        let mut app = app_on_disk(&dir);
        key(&mut app, KeyCode::Char('c'));
        assert_eq!(app.game.as_ref().unwrap().difficulty, Difficulty::Hard);

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
    fn losing_focus_pauses_a_running_game_only() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('?'));
        app.set_focus(false);
        assert!(app.game.as_ref().unwrap().paused);
        assert_eq!(app.top(), Some(Overlay::Paused), "pause covers the help");
        app.set_focus(true);
        assert!(
            app.game.as_ref().unwrap().paused,
            "coming back doesn't resume"
        );
        key(&mut app, KeyCode::Char('p'));
        assert_eq!(app.top(), Some(Overlay::Help), "help is still underneath");

        // Nothing to pause on the title screen or a finished board.
        let mut title = App::new(Store::none());
        title.set_focus(false);
        assert!(title.overlays.is_empty());
        let mut done = app_with_game(Difficulty::Easy);
        solve(&mut done);
        done.set_focus(false);
        assert_eq!(done.top(), Some(Overlay::Won));
    }

    #[test]
    fn a_puzzle_dealt_while_away_starts_paused() {
        let mut app = App::new(Store::none());
        key(&mut app, KeyCode::Char('1'));
        app.set_focus(false); // dealing: nothing to pause yet
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        assert!(app.game.as_ref().unwrap().paused);
        assert_eq!(app.top(), Some(Overlay::Paused));
    }

    #[test]
    fn two_running_copies_keep_each_others_solves() {
        let dir = tempfile::tempdir().unwrap();
        let (p, s) = test_board();
        let mut a = app_on_disk(&dir);
        let mut b = app_on_disk(&dir);
        a.start_game(Game::new(Difficulty::Easy, p, s));
        b.start_game(Game::new(Difficulty::Easy, p, s));
        solve(&mut b);
        solve(&mut a); // a loaded its stats before b's solve
        let easy = Store::at(dir.path())
            .load_stats()
            .unwrap()
            .unwrap()
            .level(Difficulty::Easy);
        assert_eq!(easy.solved, 2, "neither solve is lost");
        assert!(!a.new_best || a.stats.level(Difficulty::Easy).best() == easy.best());
    }

    #[test]
    fn a_solve_is_never_offered_again_even_if_stats_cant_be_written() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("stats.json")).unwrap(); // unwritable
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        solve(&mut app);
        let msg = app.game.as_ref().unwrap().message(Duration::from_secs(60));
        assert!(msg.unwrap().starts_with("Couldn't save stats"), "{msg:?}");
        assert!(app_on_disk(&dir).resume.is_none());
    }

    #[test]
    fn a_failing_save_is_reported_once_not_on_every_move() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-dir");
        std::fs::write(&file, "").unwrap();
        let mut app = App::new(Store::at(&file.join("tudoku")));
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        let msg = |app: &App| {
            app.game
                .as_ref()
                .unwrap()
                .message(Duration::from_secs(60))
                .map(str::to_string)
        };
        assert!(msg(&app).unwrap().starts_with("Couldn't save"));
        key(&mut app, KeyCode::Char('9')); // wrong digit at (0,2)
        assert_eq!(msg(&app).unwrap(), "Not quite — that digit is wrong");
        assert!(
            app.save_on_exit().is_err(),
            "the last save's error surfaces"
        );
    }

    #[test]
    fn tutor_use_costs_the_best_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        open_tutor(&mut app); // even a nudge counts
        key(&mut app, KeyCode::Esc);
        solve(&mut app);
        assert!(!app.new_best);
        let easy = app.stats.level(Difficulty::Easy);
        assert_eq!((easy.solved, easy.best()), (1, None));
    }

    fn tutor_of(app: &App) -> &Tutor {
        app.tutor.as_ref().expect("the tutor is open")
    }

    fn stage(app: &App) -> Option<TutorStage> {
        app.tutor.as_ref().map(|t| t.stage)
    }

    /// Press `t` until the tutor shows its step on the board.
    fn show_me(app: &mut App) {
        for _ in 0..5 {
            if stage(app) == Some(TutorStage::ShowMe) {
                return;
            }
            key(app, KeyCode::Char('t'));
        }
        panic!("the tutor never got to show me");
    }

    /// The digit the tutor is about to place, and where.
    fn placement(app: &App) -> (usize, u8) {
        match &tutor_of(app).advice {
            Advice::Step(Step {
                effect: Effect::Place { cell, digit },
                ..
            }) => (*cell, *digit),
            other => panic!("expected a placement, got {other:?}"),
        }
    }

    /// Open the tutor at its nudge, past the word to try first that a
    /// board without full notes gets.
    fn open_tutor(app: &mut App) {
        key(app, KeyCode::Char('t'));
        if stage(app) == Some(TutorStage::TryFirst) {
            key(app, KeyCode::Char('t'));
        }
    }

    #[test]
    fn t_opens_a_nudge_and_counts_once_per_opening() {
        let mut app = app_with_game(Difficulty::Easy);
        let used = |app: &App| app.game.as_ref().unwrap().tutor_used;
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::TryFirst), "no notes yet");
        assert_eq!(used(&app), 0, "trying first costs nothing");
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(
            app.game.as_ref().unwrap().values,
            test_board().0,
            "nothing to do yet"
        );
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::Nudge));
        assert_eq!(used(&app), 1);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::Explain { stage: 0 }));
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::ShowMe));
        assert_eq!(used(&app), 1, "going deeper is the same ask");
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.tutor, None);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::TryFirst), "a fresh start");
        key(&mut app, KeyCode::Esc);
        assert_eq!(used(&app), 1, "closed before it named anything");
    }

    #[test]
    fn full_notes_go_straight_to_the_nudge() {
        let mut app = app_with_game(Difficulty::Easy);
        let g = app.game.as_mut().unwrap();
        for i in (0..CELLS).filter(|&i| g.values[i] == 0) {
            g.notes[i] = crate::sudoku::candidates_mask(&g.values, i);
        }
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::Nudge));
        assert_eq!(app.game.as_ref().unwrap().tutor_used, 1);
    }

    #[test]
    fn a_wrong_digit_is_named_without_trying_first() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('9')); // (0,2) is 4
        key(&mut app, KeyCode::Char('t'));
        let t = tutor_of(&app);
        assert_eq!(
            (t.stage, &t.advice),
            (TutorStage::Nudge, &Advice::WrongDigit(2))
        );
    }

    /// The stage of the worked example on screen.
    fn example_stage(app: &App) -> u8 {
        match stage(app) {
            Some(TutorStage::Explain { stage }) => stage,
            other => panic!("expected the example, got {other:?}"),
        }
    }

    /// Everything about the game a stray key or click could change.
    fn board_state(app: &App) -> ((usize, usize), Board, [u16; CELLS], bool, u32) {
        let g = app.game.as_ref().unwrap();
        (g.selected, g.values, g.notes, g.notes_mode, g.mistakes)
    }

    #[test]
    fn explain_comes_between_nudge_and_show_me() {
        let mut app = app_with_game(Difficulty::Easy);
        open_tutor(&mut app);
        assert_eq!(stage(&app), Some(TutorStage::Nudge));
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::Explain { stage: 0 }));
        assert!(
            !tutor_of(&app).can_apply(),
            "nothing to apply to an example"
        );

        // The board under the example is your game: its keys wait.
        let before = board_state(&app);
        for code in [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('4'),
            KeyCode::Char('e'),
            KeyCode::Char('n'),
            KeyCode::Char('u'),
            KeyCode::Char('a'),
        ] {
            key(&mut app, code);
        }
        key_mods(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(board_state(&app), before);
        assert_eq!(stage(&app), Some(TutorStage::Explain { stage: 0 }));

        key(&mut app, KeyCode::Char('t'));
        assert_eq!(stage(&app), Some(TutorStage::ShowMe));

        // Esc leaves the example for your game at once.
        key(&mut app, KeyCode::Esc);
        open_tutor(&mut app);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(example_stage(&app), 0);
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.tutor, None);
        assert_eq!(board_state(&app), before);
    }

    #[test]
    fn arrows_step_through_the_example() {
        let mut app = app_with_game(Difficulty::Easy);
        open_tutor(&mut app);
        key(&mut app, KeyCode::Char('t'));
        let before = board_state(&app);
        for want in [1, 2, 2] {
            key(&mut app, KeyCode::Right);
            assert_eq!(example_stage(&app), want);
        }
        for want in [1, 0, 0] {
            key(&mut app, KeyCode::Left);
            assert_eq!(example_stage(&app), want);
        }
        key(&mut app, KeyCode::Char('l'));
        assert_eq!(example_stage(&app), 1);
        key(&mut app, KeyCode::Char('h'));
        assert_eq!(example_stage(&app), 0);
        assert_eq!(board_state(&app), before, "the selection stays put");

        // Outside the example, the arrows move the selection as ever.
        key(&mut app, KeyCode::Char('t'));
        app.apply(Action::ExampleStep(1));
        assert_eq!(stage(&app), Some(TutorStage::ShowMe));
        key(&mut app, KeyCode::Right);
        assert_eq!(app.game.as_ref().unwrap().selected, (0, 3));
    }

    #[test]
    fn pausing_during_explain_hides_the_example() {
        let mut app = app_with_game(Difficulty::Easy);
        open_tutor(&mut app);
        key(&mut app, KeyCode::Char('t'));
        key(&mut app, KeyCode::Right);
        assert!(render(&app).0.contains("EXAMPLE"));
        app.set_focus(false);
        assert_eq!(app.top(), Some(Overlay::Paused));
        let screen = render(&app).0;
        assert!(screen.contains("Paused"), "{screen}");
        assert!(!screen.contains("EXAMPLE"), "{screen}");
        assert!(!screen.contains("show me in my game"), "{screen}");
        key(&mut app, KeyCode::Right); // the pause screen takes the keys
        key(&mut app, KeyCode::Char('p'));
        assert!(!app.game.as_ref().unwrap().paused);
        assert_eq!(stage(&app), Some(TutorStage::Explain { stage: 1 }));
        assert!(render(&app).0.contains("EXAMPLE"));
    }

    #[test]
    fn clicks_on_the_example_do_not_touch_your_game() {
        let mut app = app_with_game(Difficulty::Easy);
        open_tutor(&mut app);
        key(&mut app, KeyCode::Char('t'));
        let before = board_state(&app);
        // The board's top-left corner, as drawn.
        let (screen, _) = render(&app);
        let (oy, ox) = screen
            .lines()
            .enumerate()
            .find_map(|(y, line)| line.chars().position(|c| c == '┏').map(|x| (y, x)))
            .expect("a board on screen");
        let (ox, oy) = (ox as u16, oy as u16);
        for r in 0..9 {
            for c in 0..9 {
                let x = ox + 1 + c * (ui::CELL_W + 1) + ui::CELL_W / 2;
                let y = oy + 1 + r * (ui::CELL_H + 1) + ui::CELL_H / 2;
                click(&mut app, x, y);
            }
        }
        assert_eq!(board_state(&app), before);
        assert_eq!(stage(&app), Some(TutorStage::Explain { stage: 0 }));
    }

    #[test]
    fn board_keys_still_work_during_the_nudge() {
        let mut app = app_with_game(Difficulty::Easy);
        open_tutor(&mut app);
        let nudge = app.tutor.clone();
        key(&mut app, KeyCode::Down); // (1,2) is empty
        key(&mut app, KeyCode::Char('n'));
        key(&mut app, KeyCode::Char('7'));
        let g = app.game.as_ref().unwrap();
        assert_eq!(g.selected, (1, 2));
        assert_eq!(g.notes[idx(1, 2)], 1 << 7, "a pencil mark went in");
        assert_eq!(app.tutor, nudge, "and the nudge still stands");
    }

    #[test]
    fn a_applies_the_step_in_show_me() {
        let mut app = app_with_game(Difficulty::Easy);
        show_me(&mut app);
        let (cell, digit) = placement(&app);
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.game.as_ref().unwrap().values[cell], digit);
        assert_eq!(app.tutor, None);
    }

    #[test]
    fn a_is_not_offered_for_eliminations_in_pair_mark_mode() {
        let p = parse_board(
            "087000500003000060150600000200500003000089002000073900000430000900005706040007000",
        )
        .unwrap();
        let mut game = Game::new(Difficulty::Hard, p, solve_one(&p).unwrap());
        loop {
            let (mode, advice) = tutor::consult(&game);
            assert_eq!(mode, NoteMode::PairMarks);
            let Advice::Step(step) = advice else {
                panic!("expected a step, got {advice:?}")
            };
            if matches!(step.effect, Effect::Eliminate(_)) {
                break;
            }
            assert!(game.apply_step(&step, mode));
        }
        let mut app = App::new(Store::none());
        app.start_game(game);
        show_me(&mut app);
        assert!(!tutor_of(&app).can_apply());
        let before = app.game.as_ref().map(|g| (g.values, g.notes));
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.game.as_ref().map(|g| (g.values, g.notes)), before);
        assert!(app.tutor.is_some(), "the pane stays up to be read");
        let screen = render(&app).0;
        assert!(screen.contains("back to solving"), "{screen}");
        assert!(!screen.contains("do it for me"), "{screen}");
    }

    #[test]
    fn entering_the_nudged_digit_yourself_closes_the_tutor() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('t'));
        let (cell, digit) = placement(&app);
        app.apply(Action::Board(BoardAction::Select(
            row_of(cell),
            col_of(cell),
        )));
        assert!(app.tutor.is_some(), "looking around keeps it open");
        app.apply(Action::Board(BoardAction::Digit(digit)));
        assert_eq!(app.tutor, None);
    }

    #[test]
    fn undo_closes_a_stale_tutor() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('9')); // (0,2) wants a 4
        key(&mut app, KeyCode::Right);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(tutor_of(&app).advice, Advice::WrongDigit(idx(0, 2)));
        assert_eq!(app.game.as_ref().unwrap().selected, (0, 2), "it goes there");
        key(&mut app, KeyCode::Char('u'));
        assert_eq!(app.game.as_ref().unwrap().values[idx(0, 2)], 0);
        assert_eq!(app.tutor, None);
    }

    #[test]
    fn a_can_finish_the_puzzle() {
        let mut app = app_with_game(Difficulty::Easy);
        let g = app.game.as_ref().unwrap();
        let mut open: Vec<(usize, u8)> = (0..CELLS)
            .filter(|&i| !g.given[i])
            .map(|i| (i, g.solution[i]))
            .collect();
        open.pop(); // the tutor's to fill
        for (i, d) in open {
            app.apply(Action::Board(BoardAction::Select(row_of(i), col_of(i))));
            app.apply(Action::Board(BoardAction::Digit(d)));
        }
        show_me(&mut app);
        key(&mut app, KeyCode::Char('a'));
        assert!(app.game.as_ref().unwrap().completed);
        assert_eq!(app.top(), Some(Overlay::Won));
        assert_eq!(app.tutor, None);
        assert!(!app.new_best);
    }

    #[test]
    fn tutor_is_ignored_on_finished_or_paused_games() {
        let mut app = app_with_game(Difficulty::Easy);
        app.game.as_mut().unwrap().set_paused(true);
        app.apply(Action::Tutor);
        assert_eq!(app.tutor, None, "paused");
        app.game.as_mut().unwrap().set_paused(false);
        solve(&mut app);
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('t'));
        app.apply(Action::Tutor);
        assert_eq!(app.tutor, None, "finished");
        assert_eq!(app.game.as_ref().unwrap().tutor_used, 0);
    }

    #[test]
    fn starting_a_new_game_closes_the_tutor() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('t'));
        key(&mut app, KeyCode::Char('N'));
        assert_eq!(app.tutor, None);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        key(&mut app, KeyCode::Char('t'));
        assert!(app.tutor.is_some());
        app.start_game(Game::new(Difficulty::Easy, p, s));
        assert_eq!(app.tutor, None);
    }

    /// Inkala's puzzle, worked by every strategy the tutor knows until
    /// none applies, with those candidates as full notes.
    fn stuck_app() -> App {
        let p = parse_board(
            "800000000003600000070090200050007000000045700000100030001000068008500010090000400",
        )
        .unwrap();
        let mut grid = Grid::new(&p);
        while let Some(step) = grid.next_step() {
            grid.apply(&step);
        }
        let mut game = Game::new(Difficulty::Master, p, solve_one(&p).unwrap());
        game.values = *grid.values();
        game.notes = *grid.cands();
        let mut app = App::new(Store::none());
        app.start_game(game);
        app
    }

    #[test]
    fn a_places_one_digit_when_the_tutor_is_stuck() {
        let empty = |app: &App| {
            let g = app.game.as_ref().unwrap();
            (0..CELLS).filter(|&i| g.values[i] == 0).collect::<Vec<_>>()
        };
        let solved = |app: &App, i: usize| {
            let g = app.game.as_ref().unwrap();
            g.values[i] == g.solution[i]
        };
        let select = |app: &mut App, i: usize| {
            app.apply(Action::Board(BoardAction::Select(row_of(i), col_of(i))));
        };

        // On an empty cell, that's the one it fills.
        let mut app = stuck_app();
        let target = *empty(&app).last().unwrap();
        select(&mut app, target);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(tutor_of(&app).advice, Advice::Stuck);
        assert!(tutor_of(&app).can_apply(), "even at the nudge");
        key(&mut app, KeyCode::Char('a'));
        assert!(solved(&app, target));
        assert_eq!(app.tutor, None);

        // On a filled cell, it fills the first empty one.
        let mut app = stuck_app();
        let first = empty(&app)[0];
        let given = (0..CELLS).find(|&i| !empty(&app).contains(&i)).unwrap();
        select(&mut app, given);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(tutor_of(&app).advice, Advice::Stuck);
        key(&mut app, KeyCode::Char('a'));
        assert!(solved(&app, first));
    }

    fn title() -> App {
        App::new(Store::none())
    }

    #[test]
    fn the_title_menu_ends_with_the_library_and_stats() {
        let app = title();
        assert_eq!(
            app.menu_rows()[app.menu_rows().len() - 2..],
            [MenuRow::Library, MenuRow::Stats]
        );
        let screen = render(&app).0;
        assert!(
            screen.contains("6. Library Every strategy, with a worked example"),
            "{screen}"
        );
        assert!(screen.contains("7. Stats"), "{screen}");
        assert!(screen.contains("↑↓/jk + Enter, keys 1-7"), "{screen}");
    }

    #[test]
    fn six_enter_or_a_click_opens_the_map_at_the_first_technique() {
        let map = Some(Library::Map {
            at: Technique::FullHouse,
        });
        let mut app = title();
        key(&mut app, KeyCode::Char('6'));
        assert_eq!(app.library, map);

        let mut app = title();
        while app.menu_rows()[app.menu_cursor] != MenuRow::Library {
            key(&mut app, KeyCode::Down);
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.library, map);

        let mut app = title();
        let (_, hits) = render(&app);
        let row = hits
            .iter()
            .find(|h| h.action == Some(Action::OpenLibrary))
            .unwrap();
        click(&mut app, row.rect.x + 2, row.rect.y);
        assert_eq!(app.library, map);
    }

    #[test]
    fn the_in_game_picker_has_no_library() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('d'));
        let (screen, hits) = render(&app);
        assert!(!screen.contains("Library"), "{screen}");
        assert!(screen.contains("keys 1-5"));
        assert!(hits.iter().all(|h| h.action != Some(Action::OpenLibrary)));
        key(&mut app, KeyCode::Char('6'));
        assert_eq!(app.library, None);
    }

    #[test]
    fn map_keys_move_open_and_go_back() {
        let mut app = title();
        key(&mut app, KeyCode::Char('6'));
        for code in [
            KeyCode::Right,
            KeyCode::Right,
            KeyCode::Down,
            KeyCode::Down,
            KeyCode::Down,
        ] {
            key(&mut app, code);
        }
        let sky = Technique::Skyscraper;
        assert_eq!(app.library, Some(Library::Map { at: sky }));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.library, Some(Library::Lesson { at: sky, stage: 0 }));
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.library, Some(Library::Map { at: sky }));
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.library, None);
    }

    #[test]
    fn lesson_keys_step_and_turn_pages_from_stage_one() {
        let mut app = title();
        app.apply(Action::OpenLibrary);
        app.apply(Action::LibraryOpen(Technique::EmptyRectangle));
        key(&mut app, KeyCode::Right);
        key(&mut app, KeyCode::Right);
        key(&mut app, KeyCode::Right);
        let at = Technique::EmptyRectangle;
        assert_eq!(app.library, Some(Library::Lesson { at, stage: 2 }));
        key(&mut app, KeyCode::Down);
        let at = Technique::XYWing;
        assert_eq!(app.library, Some(Library::Lesson { at, stage: 0 }));
        key(&mut app, KeyCode::Char('j'));
        key(&mut app, KeyCode::Esc);
        let at = Technique::UniqueRectangle1;
        assert_eq!(app.library, Some(Library::Map { at }));
    }

    #[test]
    fn help_opens_over_the_library_and_closes_back_to_it() {
        let mut app = title();
        app.apply(Action::OpenLibrary);
        app.apply(Action::LibraryOpen(Technique::Crane));
        key(&mut app, KeyCode::Char('?'));
        assert_eq!(app.top(), Some(Overlay::Help));
        key(&mut app, KeyCode::Down); // swallowed by Help
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.top(), None);
        let at = Technique::Crane;
        assert_eq!(app.library, Some(Library::Lesson { at, stage: 0 }));
    }

    #[test]
    fn the_library_leaves_a_saved_game_alone() {
        let (p, s) = test_board();
        let mut app = title();
        app.resume = Some(Game::new(Difficulty::Hard, p, s));
        app.menu_cursor = 0;
        key(&mut app, KeyCode::Char('6'));
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Char('c')); // not Continue while reading
        assert!(app.game.is_none());
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.menu_rows().first(), Some(&MenuRow::Continue));
        let saved = app.resume.as_ref().map(|g| g.difficulty);
        assert_eq!(saved, Some(Difficulty::Hard));
        key(&mut app, KeyCode::Char('c'));
        assert!(app.game.is_some());
        assert_eq!(app.library, None);
    }

    #[test]
    fn turning_past_either_end_keeps_your_place() {
        let mut app = title();
        app.apply(Action::OpenLibrary);
        app.apply(Action::LibraryOpen(Technique::FullHouse));
        key(&mut app, KeyCode::Right);
        key(&mut app, KeyCode::Up);
        let at = Technique::FullHouse;
        assert_eq!(app.library, Some(Library::Lesson { at, stage: 1 }));
        key(&mut app, KeyCode::Esc);
        app.apply(Action::LibraryOpen(Technique::DeathBlossom));
        key(&mut app, KeyCode::Right);
        key(&mut app, KeyCode::Down);
        let at = Technique::DeathBlossom;
        assert_eq!(app.library, Some(Library::Lesson { at, stage: 1 }));
    }

    /// The keys `N` and then `answer` (y or n) on a game with a digit in.
    fn new_game_asked(answer: char) -> App {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('9'));
        key(&mut app, KeyCode::Char('N'));
        assert_eq!(
            app.top(),
            Some(Overlay::Confirm(Ask::NewGame(Difficulty::Easy)))
        );
        assert_eq!(app.dealing, None, "nothing dealt yet");
        key(&mut app, KeyCode::Char(answer));
        app
    }

    #[test]
    fn a_new_puzzle_asks_before_dropping_a_game_in_progress() {
        let app = new_game_asked('y');
        assert_eq!(app.dealing, Some(Difficulty::Easy));
        let app = new_game_asked('n');
        assert_eq!((app.dealing, app.top()), (None, None), "kept playing");
        // Caps Lock turns notes (n) into N twice over: still nothing lost.
        let app = new_game_asked('N');
        assert_eq!((app.dealing, app.top()), (None, None));
        // Nothing on the board: nothing to ask about.
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('N'));
        assert_eq!(app.dealing, Some(Difficulty::Easy));
    }

    #[test]
    fn the_level_picker_asks_too_and_a_click_answers() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('9'));
        key(&mut app, KeyCode::Char('d'));
        key(&mut app, KeyCode::Char('3'));
        assert_eq!(
            app.overlays,
            [Overlay::Confirm(Ask::NewGame(Difficulty::Hard))]
        );
        let (screen, hits) = render(&app);
        assert!(screen.contains("Deal a fresh Hard puzzle?"), "{screen}");
        assert!(
            screen.contains("Your Easy game so far will be lost."),
            "{screen}"
        );
        let yes = hits
            .iter()
            .find(|h| h.action == Some(Action::Confirm))
            .unwrap();
        click(&mut app, yes.rect.x + 1, yes.rect.y + 1);
        assert_eq!(app.dealing, Some(Difficulty::Hard));
    }

    #[test]
    fn dropping_a_game_ends_its_streak_but_an_empty_one_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        solve(&mut app);
        assert_eq!(app.stats.level(Difficulty::Easy).streak, 1);
        app.start_game(Game::new(Difficulty::Easy, p, s));
        key(&mut app, KeyCode::Char('N'));
        assert_eq!(
            app.stats.level(Difficulty::Easy).streak,
            1,
            "empty: no loss"
        );
        app.start_game(Game::new(Difficulty::Easy, p, s));
        key(&mut app, KeyCode::Char('9'));
        key(&mut app, KeyCode::Char('N'));
        key(&mut app, KeyCode::Char('y'));
        let on_disk = Store::at(dir.path()).load_stats().unwrap().unwrap();
        let easy = on_disk.level(Difficulty::Easy);
        assert_eq!((easy.streak, easy.best_streak), (0, 1));
    }

    #[test]
    fn a_level_on_the_title_menu_asks_before_replacing_the_saved_game() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Hard, p, s));
        key(&mut app, KeyCode::Char('9'));
        let mut app = app_on_disk(&dir);
        key(&mut app, KeyCode::Char('2'));
        assert_eq!(
            app.top(),
            Some(Overlay::Confirm(Ask::NewGame(Difficulty::Medium)))
        );
        key(&mut app, KeyCode::Esc);
        assert!(app.resume.is_some(), "still there to continue");
        key(&mut app, KeyCode::Char('2'));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.dealing, Some(Difficulty::Medium));
    }

    #[test]
    fn capital_r_clears_the_board_after_asking() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.top(), None, "nothing to clear");
        key(&mut app, KeyCode::Char('9'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.top(), Some(Overlay::Confirm(Ask::Restart)));
        key(&mut app, KeyCode::Char('n'));
        assert_eq!(app.game.as_ref().unwrap().values[2], 9, "kept");
        key(&mut app, KeyCode::Char('R'));
        key(&mut app, KeyCode::Char('y'));
        let g = app.game.as_ref().unwrap();
        assert_eq!(g.values, test_board().0);
        assert_eq!(g.mistakes, 1);
    }

    #[test]
    fn r_and_ctrl_r_redo() {
        let mut app = app_with_game(Difficulty::Easy);
        key(&mut app, KeyCode::Char('4')); // (0,2) is 4
        key(&mut app, KeyCode::Char('u'));
        assert_eq!(app.game.as_ref().unwrap().values[2], 0);
        key(&mut app, KeyCode::Char('r'));
        assert_eq!(app.game.as_ref().unwrap().values[2], 4);
        key_mods(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        key_mods(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(app.game.as_ref().unwrap().values[2], 4);
        key_mods(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        key_mods(&mut app, KeyCode::Char('y'), KeyModifiers::CONTROL);
        assert_eq!(app.game.as_ref().unwrap().values[2], 4);
    }

    #[test]
    fn m_leaves_for_the_title_menu_with_the_game_waiting() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_on_disk(&dir);
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Hard, p, s));
        key(&mut app, KeyCode::Char('4'));
        key(&mut app, KeyCode::Char('t'));
        key(&mut app, KeyCode::Char('m'));
        assert!(app.game.is_none() && app.tutor.is_none());
        let resume = app.resume.as_ref().expect("offered as Continue");
        assert!(resume.paused, "its clock stopped");
        assert_eq!(app.menu_rows()[app.menu_cursor], MenuRow::Continue);
        let on_disk = Store::at(dir.path()).load_game().unwrap();
        assert_eq!(on_disk.values[2], 4, "saved on the way out");
        // The library is open to it now.
        key(&mut app, KeyCode::Char('6'));
        assert!(app.library.is_some());
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('c'));
        let g = app.game.as_ref().unwrap();
        assert!(!g.paused);
        assert_eq!(g.values[2], 4);
    }

    #[test]
    fn a_finished_game_is_simply_left_and_w_brings_its_result_back() {
        let mut app = app_with_game(Difficulty::Easy);
        solve(&mut app);
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.top(), None);
        assert_eq!(
            app.game.as_ref().unwrap().message(Duration::from_secs(5)),
            Some("w shows the result again")
        );
        key(&mut app, KeyCode::Char('w'));
        assert_eq!(app.top(), Some(Overlay::Won));
        key(&mut app, KeyCode::Char('m'));
        assert!(app.game.is_none() && app.resume.is_none());
        assert_eq!(app.top(), None);
    }

    #[test]
    fn a_screen_too_small_pauses_and_takes_only_quit() {
        let mut app = app_with_game(Difficulty::Easy);
        app.set_fits(false);
        assert_eq!(app.top(), Some(Overlay::Paused));
        for code in [
            KeyCode::Char('9'),
            KeyCode::Char('p'),
            KeyCode::Esc,
            KeyCode::Char('N'),
        ] {
            key(&mut app, code);
        }
        let g = app.game.as_ref().unwrap();
        assert!(g.paused);
        assert_eq!((g.values[2], g.mistakes), (0, 0), "nothing typed blind");
        assert_eq!(app.dealing, None);
        assert!(key(&mut app, KeyCode::Char('q')), "quit still works");
        app.set_fits(true);
        key(&mut app, KeyCode::Char('p'));
        assert!(!app.game.as_ref().unwrap().paused);
    }

    #[test]
    fn seven_opens_the_stats_and_esc_closes_them() {
        let mut app = title();
        key(&mut app, KeyCode::Char('7'));
        assert_eq!(app.top(), Some(Overlay::Stats));
        let (screen, _) = render(&app);
        assert!(
            screen.contains("Solved") && screen.contains("Average"),
            "{screen}"
        );
        assert!(
            screen.contains("Easy") && screen.contains("Zen"),
            "{screen}"
        );
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.top(), None);
        let mut app = title();
        while app.menu_rows()[app.menu_cursor] != MenuRow::Stats {
            key(&mut app, KeyCode::Down);
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.top(), Some(Overlay::Stats));
    }

    #[test]
    fn a_before_show_me_says_why_nothing_happened() {
        let mut app = app_with_game(Difficulty::Easy);
        open_tutor(&mut app);
        key(&mut app, KeyCode::Char('a'));
        let g = app.game.as_ref().unwrap();
        assert_eq!(g.values, test_board().0);
        assert_eq!(
            g.message(Duration::from_secs(5)),
            Some("t until the tutor shows the step, then a")
        );
        assert!(app.tutor.is_some(), "still open");
    }
}
