//! tudoku — an offline Sudoku TUI.
//!
//! Keyboard and mouse friendly: arrows/hjkl + 1-9, or click cells,
//! the number pad, and the action buttons. All puzzles are generated
//! locally; there is no network access at any point.

mod app;
mod deal;
mod game;
mod logic;
mod store;
mod sudoku;
mod ui;

use std::io::{self, Stdout};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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
use crate::deal::{Deal, Difficulty};
use crate::game::Game;
use crate::store::Store;

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
    let mut app = App::new(Store::open_default());
    // Whether the loop ends with q or a vanished terminal, keep the game.
    // (Closing the window kills us outright with SIGHUP; autosave covers
    // that, so a closed window costs at most a few seconds of clock.)
    let result = event_loop(&mut term, &mut app);
    app.save();
    result
}

/// `Ok(None)` for a call a signal interrupted: just go round again.
fn retry_interrupted<T>(r: io::Result<T>) -> io::Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(e) => Err(e),
    }
}

fn event_loop(term: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<()> {
    let mut hits = Vec::new();
    // A puzzle being dealt on background threads (fully offline).
    let mut dealer: Option<mpsc::Receiver<Deal>> = None;

    loop {
        app.autosave();
        if let Some(level) = app.dealing {
            match &dealer {
                None => {
                    let (tx, rx) = mpsc::channel();
                    // Sending only fails if the app quit meanwhile.
                    thread::spawn(move || {
                        let _ = tx.send(deal::deal_fast(level));
                    });
                    dealer = Some(rx);
                }
                Some(rx) => {
                    if let Ok(d) = rx.try_recv() {
                        app.start_game(Game::new(level, d.puzzle, d.solution));
                        dealer = None;
                    }
                }
            }
        }

        term.draw(|f| hits = ui::render(f, app))?;

        // Poll so the timer (or the dealing spinner) refreshes on its own.
        let tick = if app.dealing.is_some() { 50 } else { 250 };
        if retry_interrupted(event::poll(Duration::from_millis(tick)))? != Some(true) {
            continue;
        }
        let Some(event) = retry_interrupted(event::read())? else {
            continue;
        };
        let action = match event {
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
/// the generator works without any I/O or network. Reports what each
/// puzzle actually is, not what the level promises.
fn offline_check() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    for level in Difficulty::ALL {
        let started = Instant::now();
        let d = deal::deal(level, &mut rng);
        let took = started.elapsed();
        assert!(sudoku::is_valid_solution(&d.solution), "{level:?}");
        assert!(sudoku::is_unique(&d.puzzle), "{level:?}");
        assert_eq!(sudoku::solve_one(&d.puzzle), Some(d.solution), "{level:?}");
        assert!(level.techniques().contains(&d.grade.hardest), "{level:?}");
        println!(
            "{:<6}  {} givens, hardest step: {:<17} ({} steps)  dealt in {:.0?}: OK",
            level.name(),
            d.givens(),
            d.grade.hardest.name(),
            d.grade.steps,
            took,
        );
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
