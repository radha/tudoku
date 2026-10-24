# Techniques library — design

Date: 2026-10-24. Status: agreed in conversation (map layout picked from
mockups built on real renders); builder check done, and its one open
question (reachable mid-game?) answered: title screen only.

## Intent

The tutor (see `2026-10-23-tutor-design.md`) teaches inside your own
puzzle, and that stays the main way to learn. What's missing is an
overview. Right now you only meet a strategy when the tutor happens to name
one, so you can't know what else exists or what comes next.

The library gives you **the vocabulary first**: every technique's name,
its level, and what it looks like on a board. Then when the tutor says
"Look for a Crane" mid-game, it's a word you've already seen, and play
reinforces it.

The owner earlier ruled a library out ("learning happens in my own
puzzle"). They changed their mind for this reason: learning still happens
in the puzzle, but you should know what's possible. The tutor design's
"No strategy library" non-goal gets rewritten to say so (see below).

How it's used: read top to bottom once, like a short course; later, come
back now and then (at launch) to look up a name the tutor gave you.

## Goals

- From the title menu you can pick **Library** instead of a level.
- The library shows every technique the tutor knows, all 36, graded by
  level, on one screen.
- Picking one shows exactly what the tutor shows after `t`,`t`: the worked
  example on a sample board beside the numbered lesson and legend, stepped
  with `← →`.
- You can read the techniques straight through in order without going
  back to the list each time.

## Non-goals

- **Not a practice mode.** You read and look; you don't solve anything.
- **No tracking** of which techniques you've read or "learned", and no
  marks on the map for them. (The tutor design already rules this out.)
- **No change to the tutor in your games.** The nudge still names only the
  strategy and doesn't show the new gist, and there's no jump from the
  tutor into the library.
- No change to saves, best times or the 🎓 counter. Reading the library
  never counts as tutor use.

## Anti-goals (a failure even if it works)

- The library's lesson screen differs from the tutor's `t`,`t` screen in
  anything except the header and the keys listed below. It must be the
  same example, steps, legend and drawing. Reuse the code; don't keep a
  second copy that can drift.
- Opening the library loses or alters a saved game.
- A gist names specific cells or digits, or says something a strategy
  doesn't guarantee.

## The screens

Three screens: the title menu (changed), the map (new), and the lesson
screen (the tutor's explain screen, reused).

### 1. Title menu

- A sixth row, **`6. Library`**, sits under the five levels, set apart by
  a thin rule. Its blurb is "Every strategy, with a worked example".
- It is reached with `6`, with the arrows/`jk` + Enter like any row, or by
  a click. The menu's hint line says "keys 1-6".
- When there's a saved game, the menu still starts with Continue, and
  Library is still the last row. The menu's starting highlight doesn't
  change (Continue, or the last level played).
- Library appears **only on the title screen**, not in the in-game
  "pick difficulty" popup (`d`), which stays at "keys 1-5". The title
  screen only shows at launch (nothing in a game leads back to it), so
  the library is reachable before the first game of a session and not
  after. **The owner chose this** over adding it to the `d` popup: to look
  something up later, quit and relaunch; the game is saved and Continue
  picks it up. Don't add a way back to the title screen.
- A saved game is untouched by visiting the library: Continue is still
  offered (and still the first row) when you come back to the menu.

### 2. The map

All 36 techniques at once in five columns, one per level, easiest to
hardest left to right (Easy, Medium, Hard, Master, Zen). Each column is
headed by the level name and how many techniques it has. Within a column,
techniques run top to bottom in `Technique::ALL` order. A technique's level
is `deal::level_of(t)`, the same one used to grade puzzles.

```
 TUDOKU   LIBRARY    every strategy the tutor knows, easiest first

 ╭ Techniques library ─────────────────────────────────────────────────╮
 │  arrows/hjkl to move, Enter to open, or click                       │
 │                                                                     │
 │  EASY 3          MEDIUM 6        HARD 8          MASTER 12    ZEN 7  │
 │  ─────────────   ─────────────   ─────────────   ──────────   ────── │
 │  Full house      Pointing        Naked quad      Swordfish    X-Chain│
 │  Naked single    Claiming        Hidden quad     XYZ-Wing     XY-... │
 │  Hidden single   Naked pair      X-Wing          W-Wing       AIC    │
 │                  ...             [Skyscraper]    ...          ...    │
 │                                                                     │
 │  Skyscraper  two rows (or columns), two spots each, with a shared…  │
 │                                                                     │
 │  Enter open   Esc back to the menu                                  │
 ╰─────────────────────────────────────────────────────────────────────╯
```

(Sketch only: the mockup used real column widths. Names are shown with
the same capitalised heading the lesson pane uses.)

- **Moving:** `↑ ↓` / `k j` move within a column; `← →` / `h l` move to
  the next column, landing on the same row or, if that column is shorter,
  its last technique. Movement stops at the edges and doesn't wrap.
- **Click** on a technique opens it.
- **The gist line:** under the columns, the highlighted technique's name
  and a one-line gist of what it is.
- **Enter** opens the highlighted technique on the lesson screen.
- **Esc** goes back to the title menu.
- Each time it opens from the menu, the highlight starts on the first
  technique (Full house). (Designer's call: lookups happen at a fresh
  launch, so remembering the last one within a run would rarely help.)
- It must fit the app's minimum terminal, 103x39. The longest names are
  16 characters ("Simple Colouring", "Unique Rectangle", "Finned
  Swordfish", "Finned Jellyfish").

### 3. The lesson screen

The tutor's `t`,`t` (Explain) screen, unchanged: the example board in its
violet frame, the numbered steps, the legend, and `← →` / `h l` through
the example's three stages. Each technique opens at stage 1.

Only these differ:

- **Header**: the `EXAMPLE` badge line becomes a `LIBRARY` badge with the
  technique's level and place in the whole list, e.g. "Hard · 13 of 36"
  for Skyscraper.
  The second header line stays as the tutor has it ("Skyscraper · step 1
  of 3 ← → steps through it").
- **Keys at the foot of the pane**:
  - `← →  step` (as now)
  - `↑ ↓  other techniques`: the previous or next technique in
    `Technique::ALL` order, crossing levels (from Empty Rectangle, `↓` goes
    to XY-Wing, then to Swordfish). It stops at the first and last
    technique rather than wrapping. `k j` do the same.
  - `Esc  back to the list`: back to the map, highlight on the technique
    you were reading, even if you got there with `↑ ↓`.
  - No `t show me in my game` (there's no game), and `a` does nothing.
- The keys are clickable like the tutor pane's keys.

## The gists (new text)

36 one-line gists, one per technique, shown only on the map's gist line.
They are new text alongside the lessons in `src/tutor/lessons.rs` (or
next to them, builder's call).

Rules, matching the lessons:

- General terms only: no specific cells (`r3c4`), digits or houses. Say "a
  digit" and "a row", not "7" and "row 3".
- Short enough to fit the gist line beside the name at the 103-wide
  minimum (about 60 characters; a test should check every gist fits).
- True for every case the detector finds, not just the worked example.
- Use tudoku's own names and words, the same ones that technique's
  lesson steps and legend use (Skyscraper's "base" and "tops", the Kite's
  "knot", XY-Wing's "pincers", W-Wing's "bridge").

Drafts (re-worded after the builder check to use each lesson's own terms:
base and tops, knot, link, pincers, bridge, fins, colours A and B, ALS).
The owner reviews them during the build:

| Technique | Gist |
|---|---|
| Full house | the one empty cell left in a row, column or box |
| Naked single | a cell where only one digit fits |
| Hidden single | a digit that fits only one cell in a row, column or box |
| Pointing | a box's spots for a digit all in one row or column |
| Claiming | a row or column's spots for a digit all in one box |
| Naked pair | two cells where only the same two digits fit |
| Hidden pair | two digits that fit only the same two cells |
| Naked triple | three cells where only three digits fit |
| Hidden triple | three digits that fit only the same three cells |
| Naked quad | four cells where only four digits fit |
| Hidden quad | four digits that fit only the same four cells |
| X-Wing | a digit's two spots in two rows make a rectangle |
| Skyscraper | two rows (or columns), two spots each, with a shared base |
| 2-String Kite | a row and a column, two spots each, tied at a knot |
| Crane | a box and a line, two spots each, joined by a link |
| Empty Rectangle | a box's spots in one row and column, and a pair outside |
| XY-Wing | a two-digit pivot and two pincers sharing a third digit |
| Swordfish | an X-Wing on three rows and three columns |
| XYZ-Wing | an XY-Wing whose pivot fits three digits |
| W-Wing | two matching two-digit ends joined by a bridge |
| Finned X-Wing | an X-Wing with fins in one box |
| Remote Pair | a group of cells fitting the same two digits, coloured A and B |
| Simple Colouring | one digit's pairs chained and coloured A and B |
| Unique Rectangle | four corners that could swap would mean two solutions |
| BUG+1 | every empty cell fits two digits but one fits three |
| Jellyfish | an X-Wing on four rows and four columns |
| Finned Swordfish | a Swordfish with fins in one box |
| WXYZ-Wing | a pivot and three cells fitting four digits, one loose |
| Finned Jellyfish | a Jellyfish with fins in one box |
| X-Chain | one digit's strong and weak links, chained |
| XY-Chain | a path of cells that each fit only two digits |
| AIC | strong and weak links across cells and digits |
| ALS-XZ | two ALSs joined by a link digit |
| ALS-XY-Wing | three ALSs joined by two link digits |
| Sue de Coq | where a box meets a line, digits split between them |
| Death Blossom | a stem cell with a petal for each of its digits |

Every technique in `Technique::ALL` must have a gist, a lesson and a worked
example; a test should fail if one is missing. (All 36 have examples
today.)

## Help, README, and the old non-goal

- The help popup (`?`) gains a line for the library.
- README gets a short **Library** section after **Tutor**: how to open it,
  the map, `↑ ↓` to read straight through, `Esc` back.
- In `docs/superpowers/specs/2026-10-23-tutor-design.md`, the "No strategy
  library" non-goal is rewritten: the tutor still only appears when
  called, and the library (this document) exists for vocabulary, not as
  a substitute for in-puzzle learning.

## Constraints

- Rust + Ratatui, as now. No new dependencies.
- Everything in `App` stays one state machine: keys and clicks both become
  `Action`s, and `App::apply` decides what they do (see `src/app.rs`
  header). The library is a base screen like the title menu and the board,
  not a popup on the overlay stack: it fills the screen, and Help (`?`)
  should still open over it.
- The lesson screen's drawing currently lives in the game screen
  (`ui::draw_game` with `app.tutor` in `TutorStage::Explain`) and assumes a
  game exists. It needs to be drawable without a game. Reuse
  `tutor::draw_focus_board`, `tutor::draw_lesson` (with the pane's key list
  made a parameter or otherwise swappable) and `tutor::explain_header`.
  Don't duplicate them.
- Worked examples come from `tutor::examples::example(t)`, which parses
  `examples.txt` and re-finds the step each call. That's fine for a key
  press; don't re-parse on every frame if it shows up as slow.
- Existing tests keep passing. New behaviour gets tests in the style of
  the existing `ui.rs` / `ui/tutor.rs` / `app.rs` tests (TestBackend
  renders, `Action` sequences).
- Quitting (`q`, Ctrl+C) works from every library screen, as everywhere.

## Left to the builder

- Data structure for the library's state (which screen, highlighted
  technique), and whether it lives in `App` as a field or an enum of base
  screens.
- Exact column widths and spacing on the map within the 103x39 minimum.
- Exact styling (colours of the LIBRARY badge, highlight). The mockup used
  the green `GREEN` for the badge and `SEL_BG` for the highlight, as the
  level menu does.
- Where the gists live in code.
