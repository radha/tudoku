# tudoku — offline Sudoku in your terminal

A good-looking Sudoku TUI built with Rust and Ratatui. Puzzles are generated
locally and graded by a human-style logic solver — no network access, ever.
Play with the keyboard, the mouse, or both.

## Run

```sh
cargo run --release
```

Requires a terminal at least **103x39** (every cell gets its own frame, and
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
| Easy   | singles                                           | ~31    |
| Medium | pointing, claiming, pairs & triples               | ~22-27 |
| Hard   | quads, X-Wing, XY-Wing                            | ~22-27 |
| Master | Swordfish, XYZ-Wing, Jellyfish                    | ~22-27 |
| Zen    | chains & sets (coming)                            | ~22-27 |

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
the tutor**. Everything lives in two small JSON files:

- Linux: `~/.local/share/tudoku/` (or `$XDG_DATA_HOME/tudoku/`)
- macOS: `~/Library/Application Support/tudoku/`

Delete that folder to start over.

## Tutor

Stuck? Press `t`. The tutor reads your board and names the easiest
strategy that makes progress, and only its name ("Look for HIDDEN
SINGLE"), so you can still find it yourself. The board stays in play
while the nudge is up.

- `t` again shows the step on your board: why it holds, and which digit
  it places or which candidates it removes, by cell name (`r3c4` is row
  3, column 4).
- `a` does it for you: it places the digit, or clears those candidates
  from your notes.
- `Esc` closes the tutor. The next `t` starts afresh from the board as it
  is then.

A wrong digit, or notes that cross out the digit a cell needs, come
first. Until every empty cell has notes, the tutor takes your notes for
pair marks, goes by the placed digits alone and says so; it can still
place a digit for you then, but leaves eliminations to you. If no
strategy it knows applies, it offers to place one correct digit.

Every opening of the tutor counts on the header's 🎓 counter, and a
puzzle you asked it about can't set a best time.

## Controls

### Keyboard

| Key                            | Action                                    |
| ------------------------------ | ----------------------------------------- |
| arrows / hjkl                  | move selection                            |
| 1-9                            | fill cell (or pencil mark in notes mode)  |
| n                              | toggle pencil-mark mode                   |
| 0 / x / e / Backspace / Delete | erase cell                                |
| u / Ctrl+Z                     | undo                                      |
| t                              | tutor: names a strategy; t again shows it |
| a                              | tutor: do it for me                       |
| N (capital n)                  | new puzzle, same difficulty               |
| d                              | difficulty picker                         |
| p                              | pause (automatic when you switch away)    |
| ? / F1                         | help                                      |
| q                              | quit (your game is saved)                 |
| Esc                            | close popup or tutor, see finished board  |

On the title screen: `c` or `Enter` continues a saved game; `1-5`,
arrows/hjkl + `Enter`, or a click starts a level.

### Mouse

Everything is clickable: cells to select, the number pad to fill, and all
action buttons (Notes, Undo, Tutor, Erase, New, Level, Pause, Help), and
the keys in the tutor's pane.
Clicking outside a popup closes it, just like Esc.

## Features

- 5 difficulty levels graded by solving technique, all pure logic
- Pencil marks, undo, erase
- A tutor that names the next strategy, shows it on your board and can
  apply it, with a 🎓 counter of how often you asked
- Mistake highlighting, remaining-digit counters, progress bar, timer
- Autosave and resume, tutor-free best times per level
- Pause that hides the board and stops the clock, automatically when
  the terminal loses focus
- Win screen with time / mistakes / tutor use and your record

## Self-checks

```sh
cargo test                               # solver, grader, generator + UI tests
cargo run --release -- --offline-check   # deals one puzzle per level, checks + reports it
```
