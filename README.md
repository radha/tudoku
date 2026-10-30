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

| Level  | Strategies it can need                                                                       | Clues  |
| ------ | -------------------------------------------------------------------------------------------- | ------ |
| Easy   | full house, naked & hidden singles                                                           | ~31    |
| Medium | pointing, claiming, naked & hidden pairs and triples                                         | ~22-27 |
| Hard   | quads, X-Wing, Skyscraper, 2-String Kite, Crane, Empty Rectangle, XY-Wing, Unique Rectangles | ~22-27 |
| Master | Swordfish, Jellyfish, finned fish, XYZ/W/WXYZ-Wing, Remote Pair, Simple Colouring, BUG+1     | ~22-27 |
| Zen    | X-Chain, XY-Chain, AIC, ALS-XZ, ALS-XY-Wing, Sue de Coq, Death Blossom                       | ~22-27 |

Every puzzle has exactly one solution (cells are removed from a random
complete grid only while a solver confirms uniqueness) and can be finished
by logic alone — never by guessing. Puzzles are dealt on all CPU cores in
the background; every level typically takes well under a second.

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

- `t` again explains the strategy in a few plain steps, beside a worked
  example on a sample board (violet frame, EXAMPLE in the header, so it
  never reads as your game). `←` `→` (or `h` `l`) step through it: what's
  involved, then the links, then the result.
- `t` once more shows the step on your board: why it holds, and which
  digit it places or which candidates it removes, by cell name (`r3c4` is
  row 3, column 4).
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

### Strategies

The tutor knows 39 strategies, easiest first within each level:

- **Easy:** full house, naked single, hidden single
- **Medium:** pointing, claiming, naked pair, hidden pair, naked triple,
  hidden triple
- **Hard:** naked quad, hidden quad, X-Wing, Skyscraper, 2-String Kite,
  Crane, Empty Rectangle, XY-Wing, Unique Rectangle types 1, 2, 3 and 4
- **Master:** Swordfish, XYZ-Wing, W-Wing, finned X-Wing, Remote Pair,
  Simple Colouring, BUG+1, Jellyfish, finned Swordfish, WXYZ-Wing,
  finned Jellyfish
- **Zen:** X-Chain, XY-Chain, AIC, ALS-XZ, ALS-XY-Wing, Sue de Coq,
  Death Blossom

## Library

To learn the names before you need them, pick **Library** (`6`) on the
title screen. It lays out every strategy the tutor knows, one column per
level, easiest first, with a one-line gist of the highlighted one.

- Arrows (or `hjkl`) move around; Enter, or a click, opens a strategy:
  the tutor's explanation beside its worked example, as `t` `t` shows it.
- `←` `→` step through the example; `↑` `↓` go to the previous or next
  strategy, so you can read them all in order.
- `Esc` goes back to the map, and from there to the title screen.

It's only on the title screen, and reading it doesn't touch your saved
game, your best times or the 🎓 counter.

## Controls

### Keyboard

| Key                            | Action                                    |
| ------------------------------ | ----------------------------------------- |
| arrows / hjkl                  | move selection                            |
| 1-9                            | fill cell (or pencil mark in notes mode)  |
| n                              | toggle pencil-mark mode                   |
| 0 / x / e / Backspace / Delete | erase cell                                |
| u / Ctrl+Z                     | undo                                      |
| t                              | tutor: names it, explains it, shows it    |
| a                              | tutor: do it for me                       |
| N (capital n)                  | new puzzle, same difficulty               |
| d                              | difficulty picker                         |
| p                              | pause (automatic when you switch away)    |
| ? / F1                         | help                                      |
| q                              | quit (your game is saved)                 |
| Esc                            | close popup or tutor, see finished board  |

On the title screen: `c` or `Enter` continues a saved game; `1-5`,
arrows/hjkl + `Enter`, or a click starts a level; `6` opens the library.

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
cargo run --release -- --offline-check   # deals 5 puzzles per level, checks them, reports median deal time
cargo run --release -- --find-examples   # finds the tutor's worked examples still missing
```

`--find-examples` is a developer tool for the tutor's worked examples. It
takes strategy names (`--find-examples X-Wing "naked pair"`) and prints
lines for `src/tutor/examples.txt`; `--puzzles N` sets how many dealt
puzzles to search (3000 by default).
