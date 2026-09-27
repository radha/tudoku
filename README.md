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

| Level  | Givens | Holes |
| ------ | ------ | ----- |
| Easy   | ~45    | 36    |
| Medium | ~37    | 44    |
| Hard   | ~31    | 50    |
| Expert | ~26    | 55    |
| Zen    | ~21    | 60    |

Every puzzle is guaranteed to have exactly one solution: cells are removed
from a random complete grid only while a solver confirms uniqueness.

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
cargo test                      # solver + generator unit tests
cargo run --release -- --offline-check   # deals one puzzle per level, asserts uniqueness
```
