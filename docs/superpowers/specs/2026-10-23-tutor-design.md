# Tutor mode — design

Date: 2026-10-23. Status: agreed in conversation; builder check done, and
its one open question (how notes are read) is answered below.

## Intent

tudoku is going to be its owner's sudoku coach. Today, when you're stuck,
`H` (Hint) just fills in a cell, which gets the puzzle moving but teaches
nothing. The owner learns strategies from Sudoku Coach, and they want to
learn them **in their own puzzles, at the moment they're stuck**. They
sometimes get stuck on easy things too, not only the exotic ones.

Hint is removed and replaced by a **tutor**: you ask, and it helps one step
at a time, giving away no more than you ask for:

1. **Nudge**: it names only the strategy: "Look for a Skyscraper."
2. **Explain**: it describes the strategy and walks through a worked
   example on a sample board, step by step.
3. **Show me**: it draws that same pattern on *your* board: the cells, the
   links and what gets removed.

To teach the harder strategies, the solver learns a much larger set of
strategies, and the levels are regraded around them, adding a fifth level.

## Goals

- The tutor replaces Hint completely: the `H` key, the Hint button, hinted
  (locked, green) cells and the hint counter all go away.
- The tutor always points at the **easiest strategy that makes progress
  from your board as it is right now**.
- A broad set of strategies, grouped into families, each with a nudge, an
  explanation, a worked example, and a drawing on your own board.
- Levels regraded around those families: Easy, Medium, Hard, **Master**
  (new), Zen.
- Every puzzle still has exactly one solution and can be solved by logic
  from the set of strategies alone. No guessing, no network.

## Non-goals

- **No strategy library.** The owner said a library to browse won't help;
  learning happens in their own puzzle. The tutor only appears when called.
- No forcing chains, Nishio, or any trial-and-error method. Puzzles that
  need them are thrown away during dealing.
- No tracking of which strategies you've "learned", and no adaptive
  difficulty.
- No change to how mistakes work: a wrong digit is still flagged at once
  and counted.

## Anti-goals (would make it a failure even if it "works")

- The tutor gives away more than you asked for. A nudge names the strategy
  and nothing else: no digit, no cell, no region.
- The tutor is wrong: it claims a pattern that isn't valid, or removes a
  candidate that is the answer. It must never mislead.
- The tutor ignores your own eliminations, either by putting back a
  candidate you correctly crossed out or by pointing at a pattern that only
  exists in candidates you've already removed.
- The tutor misreads pair marks as candidate lists, and nags about "wrong
  notes" or claims a single that you never deduced.
- The worked example could be mistaken for your game.

## The interaction (agreed from mockups)

The mockups that were approved are in `.superpowers/brainstorm/` (the
`tutor-flow-v3.html` screen), drawn at the real 103×41 layout.

- **Key `t`** opens the tutor and takes you one step deeper each time it's
  pressed: nudge, then explain, then show me. **Esc** closes it.
  The next `t` after closing starts again from a nudge, worked out afresh
  from the board as it is then.
- **The Hint button becomes a Tutor button** (`t`) in the side panel.
- **Nudge.** The number pad stays in the side panel; the tutor pane takes
  the area where the action buttons sit. The nudge is the strategy's name
  ("Look for a **Skyscraper**"), plus `t what's that?` and `Esc I'll find
  it`. The board stays fully playable while the nudge is up, so the nudge
  stays in sight while you hunt.
- **Explain.** The big board is replaced by a worked example on a sample
  board: a **violet board frame** and an **EXAMPLE** badge in the header,
  so it can never be mistaken for your game. The tutor pane takes the whole
  side panel and explains the strategy in a few short numbered steps, with
  a legend for the highlight colours. **← →** steps through the example: the
  houses/cells involved, then the links, then the result. `t` moves on to
  show me, and Esc goes back to solving (your board returns untouched).
- **Show me.** Your own board comes back, narrowed to what the pattern is
  about, with the pattern drawn on it. The pane says what it is in your
  terms, in r#c# notation, e.g. "On the 1s: columns 1 and 6 each have only
  two 1s. The base is row 7. So r6c1 or r5c6 is a 1. Not 1: r5c3."
  **`a`** applies the result for you (removes those candidates, or places
  the digit for a single). Esc returns to normal play.
- **The drawing**, as in the mockups:
  - The board is narrowed to the pattern. For a single-digit pattern, only
    that digit's candidates show, and other placed digits are dimmed. For
    multi-digit patterns, only the pattern cells (and the cells losing
    candidates) show their notes, and everything else is dimmed.
  - Links run between the candidate positions themselves. A **strong link**
    is a solid cyan line (`┃ ━`); a **weak link** is a dashed violet line
    (`╏ ╍`). Links between cells that don't share a row or column use an
    L-shaped path.
  - Pattern candidates are coloured chips (e.g. cyan for the Skyscraper's
    tops, violet for its base). Chains alternate between two chip colours.
    Candidates being removed are **red and struck through** on a dark red
    cell. A digit being placed is shown in its own colour.
  - Lines must never hide a candidate that matters; candidates are drawn on
    top of lines.
- **Counter.** The header's 💡 hint counter becomes a 🎓 tutor counter. The
  win screen shows "Tutor n" in place of "Hints n".

## What the tutor reasons from

**How the owner takes notes.** They start each puzzle with *pair marks*:
a digit noted only in the two (later three) cells of a box where it can go,
which makes singles, pairs and triples easy to spot. Notes like that are
not candidate lists. For example, `{1}` in r1c8 means "the 1 of box 3 is
here or in r1c9", not "r1c8 is a 1". By the time they ask the tutor, they
have filled in full candidates everywhere and already spent time hunting
for patterns. (The owner's own saved game confirms the pair-mark phase:
20 of 51 empty cells noted, many with single box-pair marks.)

So the tutor reads notes in one of two ways, for the whole board at once.
A single cell can't tell you which style it's in:

- **Full candidates: every empty cell has at least one note.** Your notes
  are the truth. When a candidate is missing, it's most likely because you
  eliminated it with a technique, so the tutor never puts it back. A cell's
  candidates are its notes, minus any digit already placed in a peer cell.
- **Still in pair marks: any empty cell has no notes.** The tutor sets the
  notes aside completely and reasons from the placed digits alone: each
  empty cell gets every digit not placed in a peer cell. The pane says so
  in one line, e.g. "Going by placed digits — your notes aren't filled in
  yet." This must never read a pair mark as a candidate list (no "naked
  single" from a lone mark). *(My call.)*

**Checks before the nudge, in this order:**

1. A wrong digit on the board (it's already shown red): "r4c5 is wrong:
   clear it first." Runs in both modes.
2. **Full-candidates mode only:** a cell whose notes rule out the digit
   that actually goes there: "Your notes in r4c5 cross out something that
   still fits." Don't say which digit. The owner expects this to be rare.
   It also catches a cell left with only pair marks while switching over,
   which is fair feedback.

If neither check finds anything, the tutor gives the nudge.

- The nudge is the **easiest** applicable step in the strategy order below.
  "Show me" shows the same instance of that step that the nudge was about.
- **`a` (apply)** places the digit for a single in either mode; placing it
  clears that digit from peer notes, as a correct entry does today. It
  removes the eliminated candidates from your notes in full-candidates
  mode. In pair-mark mode, an elimination has nowhere to be recorded, so
  `a` isn't offered for it; the tutor's pane says what can be removed and
  leaves the rest to you. *(My call.)*
- If no strategy in the set applies (possible in principle after unusual
  play), the tutor says so plainly and offers to place one correct digit
  instead. *(My call.)*

## Cost

Any tutor use, even a nudge, means that puzzle doesn't count for a best
time, just as a hint does today ("I pay"). Each time the tutor is opened
fresh, the 🎓 counter goes up by one; going deeper (explain, show me) on the
same nudge doesn't count again. *(My call on how it's counted.)*

## Strategies and levels

A puzzle's level is the level of the hardest strategy it needs, as today.
Strategies are listed easiest first; that order is the tutor's "easiest
applicable" order and the grader's order. The builder may reorder within a
level but not across levels.

| Level | Family | Strategies |
|---|---|---|
| **Easy** | Singles | full house (last empty cell in a house), naked single, hidden single |
| **Medium** | Intersections | pointing, claiming (box/line reduction) |
| | Subsets | naked pair, hidden pair, naked triple, hidden triple |
| **Hard** | Subsets | naked quad, hidden quad |
| | Fish | X-Wing |
| | Single-digit patterns | Skyscraper, 2-String Kite, Crane (Sudoku Coach's name for a Turbot Fish with a box link), Empty Rectangle |
| | Wings | XY-Wing |
| **Master** | Fish | Swordfish, Jellyfish, finned and sashimi X-Wing / Swordfish / Jellyfish |
| | Wings | XYZ-Wing, W-Wing, WXYZ-Wing |
| | Uniqueness | Unique Rectangle (types 1, 2 and 4 at minimum; more types and Hidden Rectangle at the builder's discretion), BUG+1 |
| | Colouring | Simple Colouring (colour trap, colour wrap) |
| **Zen** | Chains | Remote Pair, X-Chain, XY-Chain, alternating inference chains (AIC) |
| | Sets | ALS-XZ, ALS-XY-Wing, Sue de Coq, Death Blossom |

- Uniqueness strategies are safe because every tudoku puzzle has exactly
  one solution.
- Names: the owner is fine with tudoku using its own names. Use the widely
  known names above unless a clearer one is obvious.
- Easy keeps digging to ~31 clues (50 holes). The other levels dig until no
  more cells can go, as today.
- Level blurbs (shown in the menu and header): Easy "Warm-up: just
  singles", Medium "Needs notes: pointing & pairs", Hard "Patterns:
  X-Wing, Skyscraper, XY-Wing", Master "Fish, wings & rectangles", Zen
  "Chains & sets". *(My calls; adjust if they don't fit the menu width.)*
- **Master** sits between Hard and Zen. Today's Zen strategies (Swordfish,
  XYZ-Wing, Jellyfish) move to Master. Zen becomes the chains-and-sets level.
- Number keys on the level menus become `1-5`.

## Explanations and worked examples

Every strategy needs:

- a **name** (the nudge);
- a **general explanation** of 3 to 5 short numbered steps, written for
  someone meeting it for the first time, in plain words, fitting a pane
  about 21 characters wide;
- a **worked example**: a sample board state with notes, plus the
  ← → stages (what's involved, then the links, then the result);
- a **description of the specific instance** for show me, worded around your
  board (houses, cells in r#c#, digits, what's removed or placed).

Worked examples are **chosen ahead of time and built into the program**,
not found at runtime, so every example is clean and easy to read: the
pattern stands out and there are few distractions. *(My call.)* A
developer-only command that searches dealt puzzles for good instances of
each strategy is the suggested way to produce them. How examples are stored
is up to the builder.

## Saved data

The level rename from earlier today (Easy dropped; Medium→Easy, Hard→Medium,
Expert→Hard) **is not committed yet**. Saved files on disk still use the
original five names. They are converted **once**, when loaded, to the
final names:

| On disk (original) | Becomes |
|---|---|
| Easy | dropped (stats); an in-progress Easy game becomes Easy |
| Medium | Easy |
| Hard | Medium |
| Expert | Hard |
| Zen | **Master** (old Zen puzzles are the Swordfish/XYZ/Jellyfish band) |

- **Stats** (`stats.json`) gain a version, so already-converted files are
  never converted twice. *(My call; this also fixes the problem flagged
  earlier, where the rename would have attributed old records to the wrong
  levels or set aside a stats file containing "Expert".)*
- **Games** (`game.json`) move to save version 2. Version 1 saves are
  converted: hinted cells become ordinary entries, and the hint count
  carries over as the tutor count, so a hinted game still can't set a best
  time.
- The owner's own data today: stats hold `Hard` (1 solve) and `Zen` (32
  solves). After conversion, these become Medium and Master, and their
  in-progress Zen game resumes as Master.

## Constraints

- Terminal app in Rust with Ratatui, as today. The minimum terminal size
  stays 103×39 (`MIN_W`/`MIN_H` in `ui.rs`). The tutor pane fits in the
  existing 25-column side panel, and the tutor must work at that minimum
  size too.
- Fully offline. Nothing ships that needs the network.
- Colours are explicit RGB like the rest of the palette. New colours for the
  tutor (violet, chip and pattern backgrounds, a dimmed-digit colour) go
  with the existing palette in `ui.rs`. Strikethrough uses the terminal's
  crossed-out attribute; colour also carries the meaning, so a terminal
  without strikethrough still reads correctly.
- Dealing at every level must stay quick enough that the existing "shuffling
  a fresh puzzle" spinner is a brief wait (ideally well under a second, at
  most a few seconds). If puzzles for some level turn out to be too rare
  to generate quickly, a pool of pre-generated puzzles can be built into the
  program, which is still offline.
- The solver must stay correct: every step it reports must be a valid
  deduction. Existing tests that check deductions against the known
  solution carry over to every new strategy.
- Mouse: the Tutor button, and the key lines in the tutor pane, are
  clickable like the rest of the UI. A click outside the pane does not
  close it during nudge (the board stays playable).

## Left to the builder

- The internal shape of a solver step that carries what the tutor needs:
  pattern cells and their roles, links, strong vs. weak, removals or
  placement, and the houses involved.
- Exact wording of explanations and instance descriptions, within the
  intent above.
- How the board narrows for each multi-digit family, within the rules
  above.
- What happens when you make a board move while "show me" is up. A natural
  choice: the move goes through, and the board returns to normal view.
- What happens if the board changes so the nudged step no longer applies
  (e.g. you made the elimination yourself). A natural choice: the pane
  closes or updates on its own.
- Help screen, footer and README updates to match (`t` tutor, `a` apply,
  1-5 level keys, the Master level, no Hint).
- Test approach, following the existing style: unit tests per strategy on
  set-up grids, deduction-vs-solution checks over many dealt puzzles, and
  UI tests on rendered frames.
