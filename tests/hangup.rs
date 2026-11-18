//! Closing the terminal must end the game, even when the hang-up's single
//! SIGHUP can't: crossterm's input loop spins on end-of-file and the event
//! loop never sees the stop flag, so only the hang-up watcher can exit.
#![cfg(unix)]

use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The game as session leader on a fresh terminal, as `exec tudoku` or a
/// terminal profile that runs it directly would have it: no shell to pass on
/// a second SIGHUP when the window closes.
#[test]
fn exits_when_its_terminal_closes() {
    let (mut master, slave) = open_pty(45, 120);
    let home = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_tudoku"));
    command
        .env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path())
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave));
    // SAFETY: setsid and ioctl are async-signal-safe.
    unsafe {
        command.pre_exec(|| {
            // A session of its own, with the terminal as its controlling one.
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    drop(command); // the parent's copies of the terminal's game side

    // Wait for the first frame to finish, so the game is up and watching.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    let mut last_output = Instant::now();
    let mut buf = [0; 4096];
    loop {
        match master.read(&mut buf) {
            Ok(n) if n > 0 => {
                seen.extend_from_slice(&buf[..n]);
                last_output = Instant::now();
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("reading the terminal: {e}"),
        }
        let drawn = seen.windows(8).any(|w| w == b"\x1b[?1049h") && seen.len() > 1000;
        if drawn && last_output.elapsed() > Duration::from_millis(300) {
            break;
        }
        assert!(Instant::now() < deadline, "no first frame: {seen:?}");
        std::thread::sleep(Duration::from_millis(10));
    }

    drop(master); // the window closes
    let closed = Instant::now();
    while closed.elapsed() < Duration::from_secs(5) {
        if child.try_wait().unwrap().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    panic!("still running 5 s after its terminal closed");
}

/// A pseudo-terminal of the given size: the terminal's side, non-blocking,
/// and the game's side. Neither is inherited by children unless passed on.
fn open_pty(rows: u16, cols: u16) -> (File, OwnedFd) {
    let (mut master, mut slave) = (-1, -1);
    let mut size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: valid out-pointers; no name wanted, default termios.
    let r = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    };
    assert_eq!(r, 0, "openpty: {}", io::Error::last_os_error());
    // SAFETY: openpty just opened both, and nothing else owns them.
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    for fd in [&master, &slave] {
        // SAFETY: fcntl on a descriptor we own.
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    // SAFETY: as above.
    unsafe {
        let flags = libc::fcntl(master.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    (File::from(master), slave)
}
