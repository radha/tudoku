# tudoku — offline Sudoku in your terminal

A good-looking Sudoku TUI built with Rust and Ratatui. Puzzles are generated
locally with a backtracking solver — no network access, ever. Play with the
keyboard, the mouse, or both.

## Run

```sh
cargo run --release
```

Requires a terminal at least 67x48 (the grid uses near-square cells).
Mouse support needs a terminal that
reports mouse clicks (most modern ones do).

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

| Key                        | Action                        |
| -------------------------- | ----------------------------- |
| arrows / hjkl              | move selection                |
| 1-9                        | fill cell (or pencil mark)    |
| n                          | toggle pencil-mark mode       |
| 0 / Backspace / Delete / e | erase cell                    |
| u / Ctrl+Z                 | undo                          |
| H (capital h)              | hint (reveals and locks cell) |
| N (capital n)              | new puzzle, same difficulty   |
| d                          | difficulty picker             |
| p                          | pause (hides board, stops timer) |
| ? / F1                     | help                          |
| q                          | quit                          |
| Esc                        | close popup                   |

On the difficulty picker: `1-5`, arrows/hjkl + `Enter`, or click.

### Mouse

Everything is clickable: cells to select, the number bar to fill, and all
action buttons (Notes, Undo, Hint, Erase, New, Level, Pause, Help).

## Features

- 5 difficulty levels with unique-solution offline generation
- Pencil marks, undo, hints (locked cells), erase
- Mistake highlighting, remaining-digit counters, progress bar, timer
- Pause that hides the board and stops the clock
- Win screen with time / mistakes / hints summary

## Self-checks

```sh
cargo test                               # solver, grader, generator + UI tests
cargo run --release -- --offline-check   # deals one puzzle per level, checks + reports it
```
