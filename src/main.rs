//! tudoku — an offline Sudoku TUI.
//!
//! Keyboard and mouse friendly: arrows/hjkl + 1-9, or click cells,
//! the number pad, and the action buttons. All puzzles are generated
//! locally; there is no network access at any point.

mod app;
mod ui;

use tudoku_core::{deal, game, library, logic, store, sudoku, tutor};

use std::io::{self, BufWriter, IsTerminal, StdoutLock};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{
        self, DisableFocusChange, EnableFocusChange, Event, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    style::Print,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::app::App;
use crate::deal::Difficulty;
use crate::game::Game;
use crate::logic::Technique;
use crate::store::Store;

/// Ask for button events only, in SGR form. crossterm's `EnableMouseCapture`
/// also turns on any-motion tracking (`?1002h ?1003h`), which sends an event
/// for every cell the pointer crosses; the app only uses left-button presses.
const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1000l";

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
            Print(MOUSE_ON),
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
        Print(MOUSE_OFF),
        LeaveAlternateScreen,
        cursor::Show
    );
    let _ = disable_raw_mode();
}

/// Restore the terminal before a main-thread panic message prints, so it
/// lands on the normal screen instead of vanishing with the alternate one.
/// Panics on the dealing threads leave the screen alone: the main thread
/// outlives them and deals the puzzle itself.
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

/// If the terminal goes away, crossterm's input loop spins forever on
/// end-of-file and the main thread never gets control back, not even to see
/// the stop flag the hang-up's SIGHUP sets. Watch for that from the side and
/// exit; every edit has already been saved, so at most the last 30 s of
/// clock and selection are lost.
fn exit_if_terminal_vanishes() {
    if !io::stdin().is_terminal() {
        return; // input comes from /dev/tty some other way; nothing to watch
    }
    thread::spawn(|| {
        #[cfg(unix)]
        {
            if wait_for_hangup().is_ok() {
                std::process::exit(0);
            }
            // Waiting itself failed: fall back to sleeping.
        }
        loop {
            thread::sleep(Duration::from_secs(5));
            if !io::stdin().is_terminal() {
                std::process::exit(0);
            }
        }
    });
}

/// Block until the terminal on stdin hangs up, with no wakeups before then.
/// poll() with no events asked for and no timeout returns only for
/// POLLHUP/POLLERR/POLLNVAL: when the terminal goes away.
#[cfg(all(unix, not(target_os = "macos")))]
fn wait_for_hangup() -> io::Result<()> {
    let mut fd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: 0,
        revents: 0,
    };
    loop {
        // SAFETY: `fd` is a valid pollfd and we pass a count of 1.
        let r = unsafe { libc::poll(&mut fd, 1, -1) };
        if r > 0 {
            return Ok(());
        }
        if r < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }
}

/// macOS builds poll() on kqueue and gives a pollfd with no events nothing
/// to wait on, so the poll() above would sleep through the hang-up; and
/// isatty() stays true after it, so the fallback can't see it either. Ask
/// kqueue directly: its read filter flags EV_EOF when the terminal hangs up.
/// Edge-triggered (EV_CLEAR), it wakes once per burst of input, which the
/// main thread is awake for anyway, rather than until that input is read.
#[cfg(target_os = "macos")]
fn wait_for_hangup() -> io::Result<()> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::ptr;

    // SAFETY: no arguments; the descriptor it returns is ours alone.
    let kq = match unsafe { libc::kqueue() } {
        -1 => return Err(io::Error::last_os_error()),
        fd => unsafe { OwnedFd::from_raw_fd(fd) },
    };
    let watch = libc::kevent {
        ident: libc::STDIN_FILENO as usize,
        filter: libc::EVFILT_READ,
        flags: libc::EV_ADD | libc::EV_CLEAR,
        fflags: 0,
        data: 0,
        udata: ptr::null_mut(),
    };
    // SAFETY: one valid change in, no room for events out.
    if unsafe { libc::kevent(kq.as_raw_fd(), &watch, 1, ptr::null_mut(), 0, ptr::null()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    loop {
        let mut event = watch;
        // SAFETY: no changes in, room for one event out, no timeout.
        let n = unsafe { libc::kevent(kq.as_raw_fd(), ptr::null(), 0, &mut event, 1, ptr::null()) };
        if n > 0 && event.flags & libc::EV_ERROR != 0 {
            return Err(io::Error::from_raw_os_error(event.data as i32));
        }
        if n > 0 && event.flags & libc::EV_EOF != 0 {
            return Ok(());
        }
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }
}

fn run() -> io::Result<()> {
    install_panic_hook();
    let stop = stop_on_signals()?;
    let _guard = TerminalGuard::enter()?;
    exit_if_terminal_vanishes();
    let mut term = Terminal::new(CrosstermBackend::new(BufWriter::with_capacity(
        8 * 1024,
        io::stdout().lock(),
    )))?;
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
    term: &mut Terminal<CrosstermBackend<BufWriter<StdoutLock<'static>>>>,
    app: &mut App,
    stop: &AtomicBool,
) -> io::Result<()> {
    let mut hits = Vec::new();
    // Redraw after anything that can change the screen, and when something
    // that depends on time changes what's showing (the clock's whole
    // second, a header message expiring), not on a fixed tick.
    let mut dirty = true;
    let mut shown = Shown::default();

    while !stop.load(Ordering::Relaxed) {
        app.autosave();
        if let Some(level) = app.dealing {
            // Dealt (fully offline) before the next frame: it takes
            // milliseconds, so the board is simply the next thing drawn.
            // On a thread of its own so a panicking worker can't take the
            // game down with it.
            let dealt = match thread::spawn(move || deal::deal(level, &mut rand::rng())).join() {
                Ok(d) => d,
                // A worker panicked: deal here instead, and repaint over
                // the panic message it printed.
                Err(_) => {
                    term.clear()?;
                    deal::deal(level, &mut rand::rng())
                }
            };
            app.start_game(Game::new(level, dealt.puzzle, dealt.solution));
            dirty = true;
        }

        if dirty || Shown::of(app) != shown {
            term.draw(|f| {
                // One size query per frame: the one `draw` already made.
                let area = f.area();
                app.set_fits(area.width >= ui::MIN_W && area.height >= ui::MIN_H);
                hits = ui::render(f, app);
            })?;
            shown = Shown::of(app);
            dirty = false;
        }

        if retry_interrupted(event::poll(next_wake(app)))? != Some(true) {
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
            Event::FocusLost => {
                app.set_focus(false);
                dirty = true;
                None
            }
            Event::FocusGained => {
                app.set_focus(true);
                dirty = true;
                None
            }
            Event::Resize(..) => {
                dirty = true;
                None
            }
            _ => None,
        };
        if let Some(action) = action {
            dirty = true;
            if app.apply(action) {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// How long a header message stays up; mirrors the TTL in `ui::header_lines`.
const MESSAGE_TTL: Duration = Duration::from_secs(4);

/// The longest the loop sleeps: the stop flag (set by a signal) is only
/// looked at when `poll` returns, and the autosave wants a look-in.
const MAX_WAIT: Duration = Duration::from_secs(1);

/// The time-dependent parts of the screen as last drawn.
#[derive(Default, PartialEq)]
struct Shown {
    /// Whole seconds on a running clock.
    secs: Option<u64>,
    /// Whether a header message was up.
    message: bool,
}

impl Shown {
    fn of(app: &App) -> Self {
        let Some(g) = &app.game else {
            return Shown::default();
        };
        if header_hidden(app, g) {
            return Shown::default();
        }
        Shown {
            secs: running(g).then(|| g.elapsed().as_secs()),
            message: g.message(MESSAGE_TTL).is_some(),
        }
    }
}

/// The tutor's worked example replaces the whole game header (clock and
/// message) while it is up, so neither is on screen to go stale. `ui::draw_game`
/// shows the example only while the game is not paused; paused, the normal
/// header is drawn again.
fn header_hidden(app: &App, g: &Game) -> bool {
    app.example.is_some() && !g.paused
}

fn running(g: &Game) -> bool {
    !g.paused && !g.completed
}

/// Poll timeout: until the next thing on screen is due to change, at most
/// [`MAX_WAIT`].
fn next_wake(app: &App) -> Duration {
    let mut wait = MAX_WAIT;
    // Nothing time-dependent is drawn behind the example screen, so the 1 s
    // cap is only for the stop flag and autosave. The clock itself keeps running.
    if let Some(g) = app.game.as_ref().filter(|g| !header_hidden(app, g)) {
        if running(g) {
            let into = Duration::from_nanos(u64::from(g.elapsed().subsec_nanos()));
            wait = wait.min(Duration::from_secs(1) - into + Duration::from_millis(1));
        }
        if g.message(MESSAGE_TTL).is_some()
            && let Some((_, at)) = &g.status
        {
            wait = wait.min(MESSAGE_TTL.saturating_sub(at.elapsed()) + Duration::from_millis(1));
        }
    }
    wait
}

/// Deal five puzzles per level, unseeded and as the game does
/// (`deal`), and check each one, proving the generator works
/// without any I/O or network. Reports what the last puzzle per level
/// actually is, not what the level promises, and the median deal time.
fn offline_check() {
    const DEALS: usize = 5;
    for level in Difficulty::ALL {
        let mut times = Vec::with_capacity(DEALS);
        let mut last = None;
        for _ in 0..DEALS {
            let started = Instant::now();
            let d = deal::deal(level, &mut rand::rng());
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
