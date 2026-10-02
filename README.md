# tudoku — offline Sudoku in your terminal

A good-looking Sudoku TUI built with Rust and Ratatui. Puzzles are generated
locally and graded by a human-style logic solver — no network access, ever.
Play with the keyboard, the mouse, or both.

## Run

```sh
cargo run --release
```

Requires a terminal at least **103x40** (every cell gets its own frame, and
the grid is drawn square on typical 2:1 fonts). A 24-bit-color terminal
such as Ghostty, kitty, WezTerm or iTerm2 looks best; mouse support needs
one that reports clicks (most modern ones do).

## Difficulties

Levels are graded by **what it takes to solve**, not just how many cells
are blank. A built-in human-style solver works each puzzle with the
techniques people use, easiest first, and a level is defined by the
hardest one the puzzle needs:

| Level  | Hardest technique needed                          | Clues  |
| ------ | ------------------------------------------------- | ------ |
| Easy   | singles                                           | 45     |
| Medium | singles                                           | ~31    |
| Hard   | locked candidates, naked/hidden pairs & triples   | ~22-27 |
| Expert | X-Wing, XY-Wing, naked/hidden quads               | ~22-27 |
| Zen    | Swordfish, XYZ-Wing, Jellyfish                    | ~22-27 |

Every puzzle has exactly one solution (cells are removed from a random
complete grid only while a solver confirms uniqueness) and can be finished
by logic alone — never by guessing. Puzzles are dealt on all CPU cores in
the background; even Zen typically takes a fraction of a second.

## Saves and best times

Your game is saved after every move, and every few seconds while you
think, so quitting, closing the window or a crash never costs you a
puzzle. Next launch, the title screen offers **Continue** (Enter or `c`);
picking a level instead starts fresh.

Best times are kept per level and only count puzzles solved **without
hints**. Everything lives in two small JSON files:

- Linux: `~/.local/share/tudoku/` (or `$XDG_DATA_HOME/tudoku/`)
- macOS: `~/Library/Application Support/tudoku/`

Delete that folder to start over.

## Controls

### Keyboard

| Key                            | Action                                    |
| ------------------------------ | ----------------------------------------- |
| arrows / hjkl                  | move selection                            |
| 1-9                            | fill cell (or pencil mark in notes mode)  |
| n                              | toggle pencil-mark mode                   |
| 0 / x / e / Backspace / Delete | erase cell                                |
| u / Ctrl+Z                     | undo                                      |
| H (capital h)                  | hint: fills the next logical cell, locked |
| N (capital n)                  | new puzzle, same difficulty               |
| d                              | difficulty picker                         |
| p                              | pause (hides board, stops timer)          |
| ? / F1                         | help                                      |
| q                              | quit (your game is saved)                 |
| Esc                            | close popup / view the finished board     |

On the title screen: `c` or `Enter` continues a saved game; `1-5`,
arrows/hjkl + `Enter`, or a click starts a level. Hints name the technique
that finds the cell ("Hint: X-Wing, then a single"), so they teach as well
as help.

### Mouse

Everything is clickable: cells to select, the number pad to fill, and all
action buttons (Notes, Undo, Hint, Erase, New, Level, Pause, Help).
Clicking outside a popup closes it, just like Esc.

## Features

- 5 difficulty levels graded by solving technique, all pure logic
- Pencil marks, undo, hints that name their technique, erase
- Mistake highlighting, remaining-digit counters, progress bar, timer
- Autosave and resume, hint-free best times per level
- Pause that hides the board and stops the clock
- Win screen with time / mistakes / hints and your record

## Self-checks

```sh
cargo test                               # solver, grader, generator + UI tests
cargo run --release -- --offline-check   # deals one puzzle per level, checks + reports it
```
