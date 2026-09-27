//! tudoku — an offline Sudoku TUI.
//!
//! Keyboard and mouse friendly: arrows/hjkl + 1-9, or click cells,
//! the number bar, and the action buttons. All puzzles are generated
//! locally; there is no network access at any point.

mod game;
mod sudoku;
mod ui;

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use rand::SeedableRng;
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::game::Game;
use crate::sudoku::Difficulty;
use crate::ui::{ClickAction, Hit, RenderState, hit_at};

struct App {
    game: Option<Game>,
    show_help: bool,
    show_levels: bool,
    level_cursor: usize,
    menu_cursor: usize,
    generating: bool,
    pending_level: Option<Difficulty>,
    last_level: Difficulty,
    hits: Vec<Hit>,
}

impl App {
    fn new() -> Self {
        Self {
            game: None,
            show_help: false,
            show_levels: false,
            level_cursor: 1,
            menu_cursor: 1,
            generating: false,
            pending_level: None,
            last_level: Difficulty::Medium,
            hits: Vec::new(),
        }
    }

    fn request_new_game(&mut self, level: Difficulty) {
        self.last_level = level;
        self.pending_level = Some(level);
        self.generating = true;
        self.show_levels = false;
        self.show_help = false;
    }

    fn finish_pending(&mut self) {
        if let Some(level) = self.pending_level.take() {
            // Seeded RNG is not needed; real randomness is fine and fully offline.
            let mut rng = rand::rng();
            let (puzzle, solution) = sudoku::generate(level, &mut rng);
            self.game = Some(Game::new(level, puzzle, solution));
            self.level_cursor = level.index();
            self.menu_cursor = level.index();
            self.generating = false;
        }
    }

    fn close_popups(&mut self) {
        if self.show_help {
            self.show_help = false;
        } else if self.show_levels {
            self.show_levels = false;
        }
    }

    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        // Returns true when the app should quit.
        if mods.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('c') | KeyCode::Char('q') => return true,
                KeyCode::Char('z') => {
                    if !self.show_levels
                        && !self.show_help
                        && let Some(g) = self.game.as_mut()
                    {
                        g.undo();
                    }
                    return false;
                }
                _ => return false,
            }
        }
        match code {
            KeyCode::Char('q') => {
                // Lowercase q quits from anywhere except the title screen,
                // where it also quits. (Shift+Q is covered below as 'Q'.)
                return true;
            }
            KeyCode::Char('Q') => return true,
            _ => {}
        }

        // Title screen (no puzzle yet).
        if self.game.is_none() {
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let n = Difficulty::all().len();
                    self.menu_cursor = (self.menu_cursor + n - 1) % n;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let n = Difficulty::all().len();
                    self.menu_cursor = (self.menu_cursor + 1) % n;
                }
                KeyCode::Char('1'..='5') => {
                    let i = match code {
                        KeyCode::Char(c) => (c as u8 - b'1') as usize,
                        _ => 0,
                    };
                    self.request_new_game(Difficulty::from_index(i));
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.request_new_game(Difficulty::from_index(self.menu_cursor));
                }
                KeyCode::Char('?') | KeyCode::F(1) => {
                    self.show_help = !self.show_help;
                }
                KeyCode::Esc => {
                    self.show_help = false;
                }
                _ => {}
            }
            return false;
        }

        let completed = self.game.as_ref().map(|g| g.completed).unwrap_or(false);

        // Difficulty picker popup.
        if self.show_levels {
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let n = Difficulty::all().len();
                    self.level_cursor = (self.level_cursor + n - 1) % n;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let n = Difficulty::all().len();
                    self.level_cursor = (self.level_cursor + 1) % n;
                }
                KeyCode::Char('1'..='5') => {
                    let i = match code {
                        KeyCode::Char(c) => (c as u8 - b'1') as usize,
                        _ => 0,
                    };
                    self.request_new_game(Difficulty::from_index(i));
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.request_new_game(Difficulty::from_index(self.level_cursor));
                }
                KeyCode::Esc | KeyCode::Char('d') => {
                    self.show_levels = false;
                }
                _ => {}
            }
            return false;
        }

        // Help popup.
        if self.show_help {
            match code {
                KeyCode::Esc
                | KeyCode::Char('?')
                | KeyCode::F(1)
                | KeyCode::Enter
                | KeyCode::Char(' ') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return false;
        }

        // Win popup open.
        if completed {
            match code {
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter => {
                    self.request_new_game(self.last_level);
                }
                KeyCode::Char('d') | KeyCode::Esc => {
                    self.show_levels = true;
                }
                _ => {}
            }
            return false;
        }

        // Paused: most keys ignored.
        if self.game.as_ref().map(|g| g.paused).unwrap_or(false) {
            match code {
                KeyCode::Char('p') | KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(g) = self.game.as_mut() {
                        g.set_paused(false);
                    }
                }
                KeyCode::F(1) => {
                    self.show_help = true;
                }
                _ => {}
            }
            return false;
        }

        let g = self.game.as_mut().expect("game exists");
        match code {
            KeyCode::Up => g.move_selection(-1, 0),
            KeyCode::Down => g.move_selection(1, 0),
            KeyCode::Left => g.move_selection(0, -1),
            KeyCode::Right => g.move_selection(0, 1),
            KeyCode::Char('k') => g.move_selection(-1, 0),
            KeyCode::Char('j') => g.move_selection(1, 0),
            KeyCode::Char('h') => g.move_selection(0, -1),
            KeyCode::Char('l') => g.move_selection(0, 1),
            KeyCode::Char('d') => {
                self.level_cursor = self.last_level.index();
                self.show_levels = true;
            }
            KeyCode::Char('1'..='9') => {
                let d = match code {
                    KeyCode::Char(c) => c as u8 - b'0',
                    _ => 0,
                };
                g.enter_digit(d);
            }
            KeyCode::Char('0')
            | KeyCode::Char('x')
            | KeyCode::Char('X')
            | KeyCode::Backspace
            | KeyCode::Delete => g.erase(),
            KeyCode::Char('e') | KeyCode::Char('E') => g.erase(),
            KeyCode::Char('n') => g.toggle_notes_mode(),
            KeyCode::Char('N') => self.request_new_game(self.last_level),
            KeyCode::Char('u') | KeyCode::Char('U') => g.undo(),
            KeyCode::Char('H') => g.hint(),
            KeyCode::Char('p') | KeyCode::Char('P') => g.set_paused(true),
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = true;
            }
            KeyCode::Esc => {}
            _ => {}
        }
        false
    }

    fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let Some(action) = hit_at(&self.hits, x, y) else {
            return false;
        };
        match action {
            ClickAction::Cell(r, c) => {
                if let Some(g) = self.game.as_mut() {
                    if !g.paused && !g.completed {
                        g.set_selected(r, c);
                    } else if g.paused {
                        g.set_paused(false);
                    }
                }
            }
            ClickAction::Digit(d) => {
                if let Some(g) = self.game.as_mut() {
                    g.enter_digit(d);
                }
            }
            ClickAction::Notes => {
                if let Some(g) = self.game.as_mut() {
                    g.toggle_notes_mode();
                }
            }
            ClickAction::Undo => {
                if let Some(g) = self.game.as_mut() {
                    g.undo();
                }
            }
            ClickAction::Hint => {
                if let Some(g) = self.game.as_mut() {
                    g.hint();
                }
            }
            ClickAction::Erase => {
                if let Some(g) = self.game.as_mut() {
                    g.erase();
                }
            }
            ClickAction::New => {
                if self.game.is_some() {
                    self.request_new_game(self.last_level);
                }
            }
            ClickAction::Level => {
                if self.game.is_some() {
                    self.level_cursor = self.last_level.index();
                    self.show_levels = true;
                }
            }
            ClickAction::Pause => {
                if let Some(g) = self.game.as_mut() {
                    g.set_paused(!g.paused);
                }
            }
            ClickAction::Help => {
                self.show_help = true;
            }
            ClickAction::LevelChoice(i) => {
                self.request_new_game(Difficulty::from_index(i));
            }
            ClickAction::MenuChoice(i) => {
                self.request_new_game(Difficulty::from_index(i));
            }
            ClickAction::Close => self.close_popups(),
            ClickAction::WinNew => {
                self.request_new_game(self.last_level);
            }
            ClickAction::WinLevels => {
                self.show_levels = true;
            }
        }
        false
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(term: &mut Terminal<CrosstermBackend<Stdout>>) {
    let _ = execute!(
        term.backend_mut(),
        event::DisableMouseCapture,
        LeaveAlternateScreen
    );
    let _ = disable_raw_mode();
}

fn run() -> io::Result<()> {
    let mut term = setup_terminal()?;
    let mut app = App::new();

    loop {
        // If a new puzzle was requested, paint the "dealing" frame first so
        // the pause is visible, then generate synchronously (fully offline).
        if app.pending_level.is_some() {
            app.generating = true;
            let st = RenderState {
                game: app.game.as_ref(),
                show_help: false,
                show_levels: false,
                level_cursor: app.level_cursor,
                menu_cursor: app.menu_cursor,
                generating: true,
            };
            term.draw(|f| {
                app.hits = ui::render(f, &st);
            })?;
            app.finish_pending();
        }

        {
            let st = RenderState {
                game: app.game.as_ref(),
                show_help: app.show_help,
                show_levels: app.show_levels,
                level_cursor: app.level_cursor,
                menu_cursor: app.menu_cursor,
                generating: app.generating,
            };
            term.draw(|f| {
                app.hits = ui::render(f, &st);
            })?;
        }

        // Poll so the timer refreshes even without input.
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    continue;
                }
                if app.handle_key(key.code, key.modifiers) {
                    break;
                }
            }
            Event::Mouse(m) => {
                if matches!(
                    m.kind,
                    MouseEventKind::Down(crossterm::event::MouseButton::Left)
                ) && app.handle_click(m.column, m.row)
                {
                    break;
                }
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }

    restore_terminal(&mut term);
    Ok(())
}

fn main() {
    // Deterministic self-check entry point for tests stays in unit tests;
    // the binary just runs the TUI.
    if std::env::args().any(|a| a == "--offline-check") {
        // Generate one puzzle per level with a fixed seed to prove the
        // offline generator works without any I/O or network.
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        for d in Difficulty::all() {
            let (mut p, s) = sudoku::generate(d, &mut rng);
            assert_eq!(sudoku::count_solutions(&mut p, 2), 1, "{:?}", d);
            assert_eq!(sudoku::solve_one(&p).unwrap(), s, "{:?}", d);
            println!("{} ({} givens): OK", d.name(), d.givens());
        }
        return;
    }
    if let Err(e) = run() {
        eprintln!("tudoku: {e}");
        std::process::exit(1);
    }
}
