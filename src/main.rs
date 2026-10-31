//! tudoku — an offline Sudoku TUI.
//!
//! Keyboard and mouse friendly: arrows/hjkl + 1-9, or click cells,
//! the number pad, and the action buttons. All puzzles are generated
//! locally; there is no network access at any point.

mod app;
mod deal;
mod game;
mod library;
mod logic;
mod store;
mod sudoku;
mod tutor;
mod ui;

use std::io::{self, IsTerminal, Stdout};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{
        self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
        Event, KeyEventKind, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::app::App;
use crate::deal::{Deal, Difficulty};
use crate::game::Game;
use crate::logic::Technique;
use crate::store::Store;

/// Raw mode, alternate screen and mouse capture for as long as it lives.
/// Dropping it restores the terminal, so an early `?` return can't leave
/// the shell in raw mode.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let guard = TerminalGuard;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableFocusChange
        )?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Leave TUI mode. Only the first call does anything: a second
/// alternate-screen exit would jump the cursor back over whatever was
/// printed after the first (like a panic message).
fn restore_terminal() {
    static RESTORED: AtomicBool = AtomicBool::new(false);
    if RESTORED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = execute!(
        io::stdout(),
        DisableFocusChange,
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
    let _ = disable_raw_mode();
}

/// Restore the terminal before a main-thread panic message prints, so it
/// lands on the normal screen instead of vanishing with the alternate one.
/// Panics on the dealing threads leave the screen alone: the main thread
/// survives them and deals the puzzle itself.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if thread::current().name() == Some("main") {
            restore_terminal();
        }
        default_hook(info);
    }));
}

/// SIGTERM, SIGINT (e.g. `kill`) or SIGHUP (the window closed) asks the
/// event loop to stop, so the game is saved, with its clock, and the
/// terminal restored on the way out. A second signal before that's done
/// (the loop is stuck, say) exits at once.
#[cfg(unix)]
fn stop_on_signals() -> io::Result<Arc<AtomicBool>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [SIGTERM, SIGINT, SIGHUP] {
        // Registered first, so it sees the flag as the previous signal
        // left it.
        signal_hook::flag::register_conditional_shutdown(signal, 1, Arc::clone(&stop))?;
        signal_hook::flag::register(signal, Arc::clone(&stop))?;
    }
    Ok(stop)
}

#[cfg(not(unix))]
fn stop_on_signals() -> io::Result<Arc<AtomicBool>> {
    Ok(Arc::new(AtomicBool::new(false)))
}

/// If the terminal goes away and no SIGHUP reaches us, crossterm's input
/// loop spins forever on end-of-file and the main thread never gets
/// control back. Watch for that from the side and exit; the autosave
/// keeps the game to within a few seconds.
fn exit_if_terminal_vanishes() {
    if !io::stdin().is_terminal() {
        return; // input comes from /dev/tty some other way; nothing to watch
    }
    thread::spawn(|| {
        loop {
            thread::sleep(Duration::from_millis(500));
            if !io::stdin().is_terminal() {
                std::process::exit(0);
            }
        }
    });
}

fn run() -> io::Result<()> {
    install_panic_hook();
    let stop = stop_on_signals()?;
    let _guard = TerminalGuard::enter()?;
    exit_if_terminal_vanishes();
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new(Store::open_default());
    // However the loop ends (q, a signal, a dead terminal), keep the game.
    // A failed final save is reported once the screen is restored.
    let result = event_loop(&mut term, &mut app, &stop);
    let saved = app
        .save_on_exit()
        .map_err(|e| io::Error::new(e.kind(), format!("couldn't save your game: {e}")));
    match (result, saved) {
        (Err(e), Err(s)) => Err(io::Error::new(e.kind(), format!("{e}; and {s}"))),
        (result, saved) => result.and(saved),
    }
}

/// `Ok(None)` for a call a signal interrupted: just go round again.
fn retry_interrupted<T>(r: io::Result<T>) -> io::Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(e) => Err(e),
    }
}

fn event_loop(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    stop: &AtomicBool,
) -> io::Result<()> {
    let mut hits = Vec::new();
    // A puzzle being dealt on background threads (fully offline).
    let mut dealer: Option<mpsc::Receiver<Deal>> = None;
    // Redraw after anything that can change the screen, and every tick for
    // the clock, but not for every report of the mouse moving.
    let mut dirty = true;
    let mut drawn = Instant::now();

    while !stop.load(Ordering::Relaxed) {
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
                    let dealt = match rx.try_recv() {
                        Ok(d) => Some(d),
                        Err(TryRecvError::Empty) => None,
                        // The dealer died without a puzzle (a worker
                        // panicked): deal here rather than spin forever,
                        // and repaint over the panic message it printed.
                        Err(TryRecvError::Disconnected) => {
                            term.clear()?;
                            Some(deal::deal(level, &mut rand::rng()))
                        }
                    };
                    if let Some(d) = dealt {
                        app.start_game(Game::new(level, d.puzzle, d.solution));
                        dealer = None;
                    }
                }
            }
        }

        // Poll so the timer (or the dealing spinner) refreshes on its own.
        let tick = Duration::from_millis(if app.dealing.is_some() { 50 } else { 250 });
        if dirty || drawn.elapsed() >= tick {
            let size = term.size()?;
            app.set_fits(size.width >= ui::MIN_W && size.height >= ui::MIN_H);
            term.draw(|f| hits = ui::render(f, app))?;
            drawn = Instant::now();
            dirty = false;
        }

        if retry_interrupted(event::poll(tick))? != Some(true) {
            continue;
        }
        let Some(event) = retry_interrupted(event::read())? else {
            continue;
        };
        if let Event::Mouse(m) = event
            && m.kind != MouseEventKind::Down(MouseButton::Left)
        {
            continue;
        }
        dirty = true;
        let action = match event {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                app.key_action(key.code, key.modifiers)
            }
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                ui::hit_at(&hits, m.column, m.row)
            }
            Event::FocusLost => {
                app.set_focus(false);
                None
            }
            Event::FocusGained => {
                app.set_focus(true);
                None
            }
            _ => None,
        };
        if let Some(action) = action
            && app.apply(action)
        {
            return Ok(());
        }
    }
    Ok(())
}

/// Deal five puzzles per level, unseeded and on every core as the game
/// does (`deal_fast`), and check each one, proving the generator works
/// without any I/O or network. Reports what the last puzzle per level
/// actually is, not what the level promises, and the median deal time.
fn offline_check() {
    const DEALS: usize = 5;
    for level in Difficulty::ALL {
        let mut times = Vec::with_capacity(DEALS);
        let mut last = None;
        for _ in 0..DEALS {
            let started = Instant::now();
            let d = deal::deal_fast(level);
            times.push(started.elapsed());
            assert!(sudoku::is_valid_solution(&d.solution), "{level:?}");
            assert!(sudoku::is_unique(&d.puzzle), "{level:?}");
            assert_eq!(sudoku::solve_one(&d.puzzle), Some(d.solution), "{level:?}");
            assert!(level.accepts(d.grade.hardest), "{level:?}");
            last = Some(d);
        }
        times.sort();
        let median = times[DEALS / 2];
        let d = last.expect("at least one deal");
        println!(
            "{:<6}  {} givens, hardest step: {:<18} ({} steps)  median of {DEALS} deals: {:.0?}: OK",
            level.name(),
            d.givens(),
            d.grade.hardest.name(),
            d.grade.steps,
            median,
        );
    }
}

/// Developer tool: search dealt puzzles for a worked example of each
/// strategy named in `names`, or of every one that has none yet, and print
/// the lines for `src/tutor/examples.txt`. `--puzzles N` sets how many
/// dealt puzzles to search (3000 by default).
fn find_examples(args: &[String]) {
    let mut names = Vec::new();
    let mut puzzles = 3000;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--puzzles" {
            puzzles = args.next().and_then(|n| n.parse().ok()).unwrap_or_else(|| {
                eprintln!("tudoku: --puzzles needs a number");
                std::process::exit(2);
            });
        } else {
            names.push(arg.clone());
        }
    }
    let wanted: Vec<Technique> = if names.is_empty() {
        Technique::ALL
            .iter()
            .copied()
            .filter(|&t| tutor::examples::example(t).is_none())
            .collect()
    } else {
        names
            .iter()
            .map(|name| {
                Technique::from_name(name).unwrap_or_else(|| {
                    eprintln!("tudoku: no strategy called {name:?}");
                    std::process::exit(2);
                })
            })
            .collect()
    };
    for line in tutor::examples::find_examples(&wanted, 7, puzzles) {
        println!("{line}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--offline-check") {
        offline_check();
        return;
    }
    if let Some(at) = args.iter().position(|a| a == "--find-examples") {
        find_examples(&args[at + 1..]);
        return;
    }
    if let Err(e) = run() {
        eprintln!("tudoku: {e}");
        std::process::exit(1);
    }
}
