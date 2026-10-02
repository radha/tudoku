//! tudoku — an offline Sudoku TUI.
//!
//! Keyboard and mouse friendly: arrows/hjkl + 1-9, or click cells,
//! the number pad, and the action buttons. All puzzles are generated
//! locally; there is no network access at any point.

mod app;
mod game;
mod sudoku;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::{
    cursor,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use rand::SeedableRng;
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::app::App;
use crate::game::Game;
use crate::sudoku::Difficulty;

/// Raw mode, alternate screen and mouse capture for as long as it lives.
/// Dropping it restores the terminal, so an early `?` return can't leave
/// the shell in raw mode.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let guard = TerminalGuard;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = execute!(
        io::stdout(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
    let _ = disable_raw_mode();
}

/// Restore the terminal before the panic message prints, so it lands on
/// the normal screen instead of vanishing with the alternate one.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

fn run() -> io::Result<()> {
    install_panic_hook();
    let _guard = TerminalGuard::enter()?;
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new();
    let mut hits = Vec::new();

    loop {
        term.draw(|f| hits = ui::render(f, &app))?;

        // The "Dealing" popup is on screen now; generate (fully offline).
        if let Some(level) = app.dealing {
            let (puzzle, solution) = sudoku::generate(level, &mut rand::rng());
            app.start_game(Game::new(level, puzzle, solution));
            continue;
        }

        // Poll so the timer refreshes even without input.
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let action = match event::read()? {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                app.key_action(key.code, key.modifiers)
            }
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                ui::hit_at(&hits, m.column, m.row)
            }
            _ => None,
        };
        if let Some(action) = action
            && app.apply(action)
        {
            return Ok(());
        }
    }
}

/// Deal one puzzle per level with a fixed seed and check each one, proving
/// the generator works without any I/O or network.
fn offline_check() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    for d in Difficulty::ALL {
        let (p, s) = sudoku::generate(d, &mut rng);
        assert!(sudoku::is_valid_solution(&s), "{d:?}");
        assert!(sudoku::is_unique(&p), "{d:?}");
        assert_eq!(sudoku::solve_one(&p), Some(s), "{d:?}");
        println!("{} ({} givens): OK", d.name(), d.givens());
    }
}

fn main() {
    if std::env::args().any(|a| a == "--offline-check") {
        offline_check();
        return;
    }
    if let Err(e) = run() {
        eprintln!("tudoku: {e}");
        std::process::exit(1);
    }
}
