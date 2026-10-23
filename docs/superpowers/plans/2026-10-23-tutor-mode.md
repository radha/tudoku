# Tutor Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Hint with a step-by-step tutor (nudge, then explain with a worked example, then show me on your board), on a solver that knows ~40 strategies, with levels regraded to Easy, Medium, Hard, Master and Zen.

**Architecture:** Every solver deduction becomes a `Step` that carries its pattern (candidates with roles, strong/weak links, houses) and a plain-words `why`. A small `tutor` module reads the player's board (notes as full candidates, or pair marks set aside), runs the checks, and returns `Advice`. The app keeps an optional `Tutor` state (Nudge, Explain, ShowMe), and `ui/tutor.rs` draws the pane, the narrowed board with links and chips, and the EXAMPLE board. New strategies arrive one family per task through a fixed recipe.

**Tech Stack:** Rust 2024, ratatui 0.30, crossterm 0.29, serde/serde_json, rand 0.10. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-23-tutor-design.md`. Read it first. The approved mockup is `.superpowers/brainstorm/*/content/tutor-flow-v3.html`, if it's still on disk.

## Global Constraints

- Fully offline; no network code, no new crates.
- The minimum terminal size stays 103×39 (`MIN_W`, `MIN_H` in `src/ui.rs`). The side panel stays 25 columns wide, and tutor text wraps at **21** columns.
- Colours are explicit RGB. New palette constants in `src/ui.rs`: `VIOLET = Rgb(176,140,255)`, `DIM = Rgb(66,74,96)`, `PAT_BG = Rgb(22,52,64)`, `BAND_BG = Rgb(27,33,50)`.
- Cell names in all text are `r{row}c{col}`, 1-based. Houses are `row n`, `column n` and `box n`, 1-based, with boxes numbered left to right, top to bottom.
- Keys: `t` tutor, `a` apply, `Esc` close the tutor, `← →` (also `h`/`l`) step the example, `1-5` pick a level on the level menus. `H` (hint) is removed.
- Any tutor use means the puzzle can't set a best time.
- Level names and order: Easy, Medium, Hard, Master, Zen. Blurbs, verbatim: Easy "Warm-up: just singles", Medium "Needs notes: pointing & pairs", Hard "Patterns: X-Wing, Skyscraper, XY-Wing", Master "Fish, wings & rectangles", Zen "Chains & sets".
- Every task ends with `cargo fmt --check`, `cargo clippy --all-targets` (no warnings) and `cargo test` all clean.
- Commit messages follow the repo style: one imperative sentence, no `feat:` prefix, ending with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Undo while the tutor is open** can bring back a candidate or digit the advice relied on. The tutor must re-check and close if its advice no longer holds. Test: Task 4 `undo_closes_a_stale_tutor`.
2. **`a` placing the last digit** finishes the puzzle. The win screen must appear with the tutor gone and no best time. Test: Task 4 `a_can_finish_the_puzzle`.
3. **The window losing focus during Explain** auto-pauses. The paused screen must hide the example, and resuming must return to Explain. Test: Task 6 `pausing_during_explain_hides_the_example`.
4. **The 103×39 minimum terminal** must fit every tutor stage: no panic, and the pane text stays inside its frame. Tests: Task 5 `show_me_fits_the_minimum_terminal`, Task 6 `explain_fits_the_minimum_terminal`.
5. **Clicks on the example board** must never touch your game. Test: Task 6 `clicks_on_the_example_do_not_touch_your_game`.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/deal.rs` | `Difficulty` (5 levels), `level_of(Technique)`, `accepts`, legacy name mapping, dealing |
| `src/logic/mod.rs` (was `src/logic.rs`) | `Technique`, `Step`/`Effect`/`Pattern`/`Mark`/`Link`, `Grid`, `grade`, shared helpers, cross-cutting tests |
| `src/logic/basic.rs` | singles, pointing, claiming, naked/hidden subsets |
| `src/logic/fish.rs` | X-Wing/Swordfish/Jellyfish and finned/sashimi fish |
| `src/logic/single_digit.rs` | Skyscraper, 2-String Kite, Crane, Empty Rectangle |
| `src/logic/wings.rs` | XY-, XYZ-, W-, WXYZ-Wing |
| `src/logic/uniqueness.rs` | Unique Rectangle, BUG+1 |
| `src/logic/colouring.rs` | Simple Colouring |
| `src/logic/chains.rs` | Remote Pair, X-Chain, XY-Chain, AIC (one chain engine) |
| `src/logic/sets.rs` | ALS enumeration, ALS-XZ, ALS-XY-Wing, Sue de Coq, Death Blossom |
| `src/tutor/mod.rs` | `NoteMode`, `Advice`, reading notes, checks, `consult` |
| `src/tutor/lessons.rs` | per-technique explanation steps and role labels |
| `src/tutor/examples.rs`, `src/tutor/examples.txt` | worked examples (data + finder) |
| `src/game.rs` | Hint removed, `tutor_used`, `apply_step`, `place_answer`, save version 2 |
| `src/store.rs` | versioned stats file and conversion |
| `src/app.rs` | `Tutor` state, actions, keys |
| `src/ui.rs`, `src/ui/tutor.rs` | pane, narrowed board, links, EXAMPLE view |
| `src/main.rs` | `--find-examples`, `--offline-check` update |

## Final strategy order and levels

`Technique` variants are declared in exactly this order, which is the tutor's and grader's "easiest first". Each technique task inserts its variants at these positions.

| Level | Variants (declaration order) |
|---|---|
| Easy | `FullHouse`, `NakedSingle`, `HiddenSingle` |
| Medium | `Pointing`, `Claiming`, `NakedPair`, `HiddenPair`, `NakedTriple`, `HiddenTriple` |
| Hard | `NakedQuad`, `HiddenQuad`, `XWing`, `Skyscraper`, `TwoStringKite`, `Crane`, `EmptyRectangle`, `XYWing` |
| Master | `Swordfish`, `XYZWing`, `WWing`, `FinnedXWing`, `SimpleColouring`, `UniqueRectangle`, `BugPlusOne`, `Jellyfish`, `FinnedSwordfish`, `WXYZWing`, `FinnedJellyfish` |
| Zen | `RemotePair`, `XChain`, `XYChain`, `Aic`, `AlsXz`, `AlsXyWing`, `SueDeCoq`, `DeathBlossom` |

Display names (`Technique::name()`): "full house", "naked single", "hidden single", "pointing", "claiming", "naked pair", "hidden pair", "naked triple", "hidden triple", "naked quad", "hidden quad", "X-Wing", "Skyscraper", "2-String Kite", "Crane", "Empty Rectangle", "XY-Wing", "Swordfish", "XYZ-Wing", "W-Wing", "finned X-Wing", "Simple Colouring", "Unique Rectangle", "BUG+1", "Jellyfish", "finned Swordfish", "WXYZ-Wing", "finned Jellyfish", "Remote Pair", "X-Chain", "XY-Chain", "AIC", "ALS-XZ", "ALS-XY-Wing", "Sue de Coq", "Death Blossom".

## Recipe: adding a technique (Tasks 7–13)

For each new technique `T`:

1. Add the variant at its position in the table above. Add it to `Technique::ALL` (same position), `Technique::name()`, `deal::level_of()` and the `Grid::find()` dispatch. All of these are exhaustive matches or a test-checked list, so the compiler and `technique_list_is_complete` catch a miss.
2. Write the detector as `pub(super) fn <snake_name>(&self) -> Option<Step>` in the family's file. Build the result with `Grid::eliminating(...)` or `Grid::placing(...)` (Task 2).
3. Write the staged unit test(s) named in the task. Stage with `logic::testing::staged`, and pad houses with extra candidates so that only the intended houses have exactly two of the digit. Assert the exact `removals(...)` or placement.
4. Add a lesson entry in `src/tutor/lessons.rs`: 3–5 steps and the two role labels. The exhaustive match won't compile without it.
5. Generate the example: `cargo run --release -- --find-examples "<display name>"`. Append the printed line to `src/tutor/examples.txt`.
6. `cargo test`. The cross-cutting tests must stay green: `every_detector_is_sound_on_dealt_puzzles`, `patterns_are_consistent`, `technique_levels_never_go_down`, `every_technique_has_a_worked_example`, `lesson_text_fits_the_pane`.
7. A new technique may solve one of the fixed puzzles in `real_puzzles_grade_by_their_hardest_technique` earlier than before. If so, set that fixture's expected technique to the new `grade()` result, and say so in the commit message.

---

### Task 1: Five levels and saved-data conversion

**Files:**
- Modify: `src/deal.rs`, `src/logic.rs` (add `Technique::ALL`), `src/store.rs`, `src/game.rs` (`SavedGame`), `src/main.rs` (`offline_check`), `src/ui.rs` (menu hint "keys 1-5"), `src/app.rs` (tests), `README.md`, `.gitignore`

**Interfaces:**
- Produces:
  - `Difficulty::{Easy, Medium, Hard, Master, Zen}`; `Difficulty::ALL: [Difficulty; 5]`
  - `Difficulty::accepts(self, hardest: Technique) -> bool`
  - `Difficulty::from_name(&str) -> Option<Difficulty>` (current names)
  - `Difficulty::from_legacy_name(&str) -> Option<Difficulty>` (pre-tutor names)
  - `deal::level_of(t: Technique) -> Difficulty`
  - `Technique::ALL: &'static [Technique]`
  - `SAVE_VERSION = 2`; `STATS_VERSION = 1`
- Removes: `Difficulty::techniques()`; serde derives on `Difficulty` and `Stats`.

- [ ] **Step 1: Branch and commit the work already in the tree**

The level rename (Easy dropped, the other names moved up) and the design spec are uncommitted on `main`.
```bash
git switch -c tutor-mode
printf '.superpowers/\n.playwright-mcp/\n' >> .gitignore
git add .gitignore README.md src
git commit -m "Drop the old Easy level and move the other names up" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git add docs/superpowers
git commit -m "Add the tutor mode design and plan" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 2: Write the failing tests**

In `src/deal.rs` tests:
```rust
#[test]
fn five_levels_in_order() {
    assert_eq!(Difficulty::ALL.map(Difficulty::name), ["Easy", "Medium", "Hard", "Master", "Zen"]);
}
#[test]
fn technique_levels_never_go_down() {
    let levels: Vec<_> = Technique::ALL.iter().map(|&t| level_of(t)).collect();
    assert!(levels.windows(2).all(|w| w[0] <= w[1]), "{levels:?}");
    assert_eq!(level_of(Technique::XWing), Difficulty::Hard);
    assert_eq!(level_of(Technique::Swordfish), Difficulty::Master);
}
#[test]
fn zen_deals_master_puzzles_until_it_has_strategies() {
    assert!(Difficulty::Zen.accepts(Technique::Jellyfish));
    assert!(Difficulty::Master.accepts(Technique::Jellyfish));
    assert!(!Difficulty::Master.accepts(Technique::XWing));
}
#[test]
fn legacy_names_map_to_the_same_puzzles() {
    use Difficulty::*;
    for (old, new) in [("Easy", Easy), ("Medium", Easy), ("Hard", Medium), ("Expert", Hard), ("Zen", Master)] {
        assert_eq!(Difficulty::from_legacy_name(old), Some(new), "{old}");
    }
    assert_eq!(Difficulty::from_legacy_name("Nope"), None);
}
```
Replace `levels_cover_every_technique_once_and_in_order` with `technique_levels_never_go_down` (above). Change `level.techniques().contains(..)` to `level.accepts(..)` in the dealing tests.

In `src/store.rs` tests:
```rust
#[test]
fn legacy_stats_move_to_the_levels_their_puzzles_belong_to() {
    // Written by the five-level build: Easy, Medium, Hard, Expert, Zen.
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(STATS_FILE), r#"{"levels":{"Easy":{"solved":3,"best_ms":1000},"Hard":{"solved":1,"best_ms":594328},"Expert":{"solved":2,"best_ms":2000},"Zen":{"solved":32,"best_ms":330137}}}"#).unwrap();
    let stats = Store::at(dir.path()).load_stats().unwrap();
    assert_eq!(stats.level(Difficulty::Easy), LevelStats::default(), "old Easy (45 clues) is dropped");
    assert_eq!(stats.level(Difficulty::Medium), LevelStats { solved: 1, best_ms: Some(594328) });
    assert_eq!(stats.level(Difficulty::Hard).solved, 2);
    assert_eq!(stats.level(Difficulty::Master), LevelStats { solved: 32, best_ms: Some(330137) });
    assert_eq!(stats.level(Difficulty::Zen), LevelStats::default());
    assert!(!dir.path().join("stats.json.damaged").exists());
}
#[test]
fn converted_stats_are_never_converted_again() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let mut stats = Stats::default();
    stats.record(Difficulty::Hard, Duration::from_secs(90), 0);
    store.save_stats(&stats).unwrap();
    let text = fs::read_to_string(dir.path().join(STATS_FILE)).unwrap();
    assert!(text.contains("\"version\":1"), "{text}");
    assert_eq!(store.load_stats(), Some(stats), "Hard stays Hard");
}
#[test]
fn unknown_level_names_are_skipped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(STATS_FILE), r#"{"version":1,"levels":{"Nope":{"solved":1,"best_ms":null},"Zen":{"solved":4,"best_ms":9}}}"#).unwrap();
    let stats = Store::at(dir.path()).load_stats().unwrap();
    assert_eq!(stats.level(Difficulty::Zen).solved, 4);
}
```
In `src/game.rs` tests:
```rust
#[test]
fn version_1_saves_resume_at_the_level_their_puzzle_belongs_to() {
    let g = test_game();
    for (old, new) in [("Zen", Difficulty::Master), ("Expert", Difficulty::Hard), ("Medium", Difficulty::Easy)] {
        let mut v = serde_json::to_value(g.snapshot()).unwrap();
        v["version"] = 1.into();
        v["difficulty"] = old.into();
        let back = Game::restore(serde_json::from_value(v).unwrap()).unwrap();
        assert_eq!(back.difficulty, new, "{old}");
    }
}
#[test]
fn version_2_saves_use_current_names_and_reject_unknown_ones() {
    let mut v = serde_json::to_value(test_game().snapshot()).unwrap();
    assert_eq!(v["version"], 2);
    v["difficulty"] = "Master".into();
    assert_eq!(Game::restore(serde_json::from_value(v.clone()).unwrap()).unwrap().difficulty, Difficulty::Master);
    v["difficulty"] = "Expert".into();
    assert!(Game::restore(serde_json::from_value(v).unwrap()).is_err());
}
```
In `src/app.rs`, update `number_keys_and_enter_start_levels`: `'4'` deals `Master` and `'5'` deals `Zen`.

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test`
Expected: compile errors (`Master`, `accepts`, `level_of`, `from_legacy_name`, `Technique::ALL` don't exist yet).

- [ ] **Step 4: Implement**

- `deal.rs`:
  - Add `Master` between `Hard` and `Zen`, plus the blurbs from Global Constraints.
  - `holes()`: Easy 50, all other levels `CELLS`.
  - `level_of` is an exhaustive match. Easy: `NakedSingle`, `HiddenSingle`. Medium: `LockedCandidates`, the pairs and the triples. Hard: `XWing`, `XYWing`, `NakedQuad`, `HiddenQuad`. Master: `Swordfish`, `XYZWing`, `Jellyfish`.
  - `accepts`: `level_of(hardest) == self || (self == Zen && level_of(hardest) == Master)`. Comment it as temporary: Zen has no strategies until Task 12.
  - `try_deal` uses `accepts`.
- `logic.rs`: add `Technique::ALL` listing the 14 current variants in declaration order.
- `store.rs`:
  - On disk: `struct StatsFile { #[serde(default)] version: u32, levels: BTreeMap<String, LevelStats> }`. In memory, `Stats.levels` stays a `BTreeMap<Difficulty, LevelStats>`.
  - Loading: version 0 maps names with `from_legacy_name`, skipping `"Easy"`. Version 1 uses `from_name`. Unknown names are skipped. A file that doesn't parse at all still goes to `.damaged`.
  - Saving writes `version: STATS_VERSION`.
- `game.rs`:
  - `SavedGame.difficulty` becomes a `String`, and `SAVE_VERSION` becomes 2.
  - `restore` resolves version 1 with `from_legacy_name` and version 2 with `from_name`. Any other version, or an unknown name, is an `Err`.
- `main.rs`: in `offline_check`, `level.accepts(d.grade.hardest)`.
- `ui.rs`: menu hint `" arrows/hjkl + Enter, keys 1-5, or click"`.
- `README.md`:
  - Levels table: Easy | singles | ~31; Medium | pointing, claiming, pairs & triples | ~22-27; Hard | quads, X-Wing, XY-Wing | ~22-27; Master | Swordfish, XYZ-Wing, Jellyfish | ~22-27; Zen | chains & sets (coming) | ~22-27.
  - `1-5` keys, "5 difficulty levels".

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo fmt --check && cargo clippy --all-targets && cargo test`
Expected: all pass, no warnings. Then `cargo run --release -- --offline-check` prints five OK lines.

- [ ] **Step 6: Commit**

```bash
git add -A src README.md
git commit -m "Add the Master level and convert saved games and stats to the new level names" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Steps that carry their pattern

**Files:**
- Move: `src/logic.rs` → `src/logic/mod.rs`. Create `src/logic/basic.rs`, `src/logic/fish.rs` and `src/logic/wings.rs`, and move the existing detectors into them.
- Modify: `src/game.rs` (`hint` uses the new `Step`), `src/deal.rs` (`level_of` for new variants)

**Interfaces:**
- Produces (in `src/logic/mod.rs`):
```rust
pub enum Technique { FullHouse, NakedSingle, HiddenSingle, Pointing, Claiming, NakedPair, HiddenPair,
    NakedTriple, HiddenTriple, NakedQuad, HiddenQuad, XWing, XYWing, Swordfish, XYZWing, Jellyfish }
impl Technique { pub const ALL: &'static [Technique]; pub fn name(self) -> &'static str }
#[derive(Debug, Clone, PartialEq, Eq)] pub enum Effect { Place { cell: usize, digit: u8 }, Eliminate(Vec<(usize, u16)>) }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Role { Key, Other }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Mark { pub cell: usize, pub digit: u8, pub role: Role }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum LinkKind { Strong, Weak }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Link { pub from: (usize, u8), pub to: (usize, u8), pub kind: LinkKind }
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pattern { pub digit: Option<u8>, pub cells: Vec<usize>, pub houses: Vec<usize>, pub marks: Vec<Mark>, pub links: Vec<Link> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step { pub technique: Technique, pub effect: Effect, pub pattern: Pattern, pub why: String }
impl Grid {
    pub fn new(values: &Board) -> Grid;
    pub fn with_candidates(values: &Board, cands: &[u16; CELLS]) -> Grid;
    pub fn values(&self) -> &Board;
    pub fn cands(&self) -> &[u16; CELLS];
    pub fn find(&self, t: Technique) -> Option<Step>;   // exhaustive dispatch
    pub fn next_step(&self) -> Option<Step>;             // first hit over Technique::ALL; None if broken
    fn eliminating(&self, t: Technique, removals: Vec<(usize, u16)>, pattern: Pattern, why: String) -> Option<Step>; // None if removals empty after masking with cands
    fn placing(&self, t: Technique, cell: usize, digit: u8, pattern: Pattern, why: String) -> Step;
    fn conjugates(&self, house: usize, digit: u8) -> Option<(usize, usize)>; // exactly two cells
}
pub fn rc(cell: usize) -> String;          // "r3c4"
pub fn house_name(house: usize) -> String; // "row 3" | "column 4" | "box 5"
pub fn digit_list(mask: u16) -> String;    // "7" | "2 and 7" | "1, 4 and 9"
#[cfg(test)] pub(crate) mod testing { pub fn staged(cands: &[(usize, &[u8])]) -> Grid; pub fn removals(step: Option<Step>) -> Vec<(usize, u16)>; }
```
- `Pattern` meaning, used by the drawing in Task 5:
  - `digit: Some(d)`: the pattern is about one digit, so the board narrows to `d`.
  - `cells`: for multi-digit patterns, the cells whose notes stay visible.
  - `houses`: indexes into `HOUSES` (0–8 rows, 9–17 columns, 18–26 boxes), tinted on the board.
  - `marks`: chips. `Role::Key` is cyan, `Role::Other` violet.
  - `links`: lines between two marks.
- `LockedCandidates` is split into `Pointing` (box → line) and `Claiming` (line → box). `FullHouse` is new: the last empty cell of a house, a `Place`.

Patterns for the existing techniques:

| Technique | digit | cells | houses | marks | links | why (model) |
|---|---|---|---|---|---|---|
| FullHouse | Some(d) | — | [h] | — | — | "r3c4 is the last empty cell in row 3, so it's a 7." |
| NakedSingle | None | [c] | — | — | — | "r3c4 has only one candidate left: 7." |
| HiddenSingle | Some(d) | — | [h] | — | — | "In box 5, 7 fits only in r5c4." |
| Pointing | Some(d) | — | [box, line] | Key: box's d-cells | — | "In box 1, every 4 is in row 3, so the rest of row 3 can't be 4." |
| Claiming | Some(d) | — | [line, box] | Key: line's d-cells | — | "In row 3, every 4 is in box 1, so the rest of box 1 can't be 4." |
| Naked subset | None | subset | [h] | Key: every candidate of the subset cells | — | "r1c2 and r1c5 hold only 2 and 7 between them, so nothing else in row 1 can be 2 or 7." |
| Hidden subset | None | subset | [h] | Key: subset digits in subset cells | — | "In column 4, 1 and 9 fit only in r2c4 and r6c4, so those cells can't hold anything else." |
| Fish (k) | Some(d) | — | base + cover lines | Key: base-line d-cells | — | "In rows 2 and 7, every 3 is in columns 2 and 8, so the rest of those columns can't be 3." |
| XY-Wing | None | pivot + 2 pincers | — | Key: z in pincers; Other: pivot's two digits and the pincers' non-z digit | Weak: (pivot,x)–(pincer1,x), (pivot,y)–(pincer2,y) | "r1c1 is 1 or 2. If 1, r1c6 is 3; if 2, r5c1 is 3. Either way one is 3, so r5c6 can't be 3." |
| XYZ-Wing | None | pivot + 2 pincers | — | Key: z in all three; Other: the rest | Weak: pivot–pincer on each shared non-z digit | similar, "one of the three is 3" |

In a `Place` step, the placed cell and digit come from `effect`, not from `marks`.

- [ ] **Step 1: Write the failing tests** (in `src/logic/mod.rs` unless noted)

```rust
#[test]
fn technique_list_is_complete() {
    // ALL lists every variant in declaration order with no gaps, so the
    // derived Ord (easiest first) agrees with it.
    for (i, &t) in Technique::ALL.iter().enumerate() {
        assert_eq!(t as usize, i, "{t:?}");
    }
    assert_eq!(*Technique::ALL.last().unwrap(), Technique::Jellyfish); // later tasks: their last variant
}
#[test]
fn find_runs_only_the_asked_technique() {
    let (p, _) = test_board();
    let g = Grid::new(&p);
    assert_eq!(g.find(Technique::XWing), None);
    let s = g.next_step().unwrap();
    assert_eq!(g.find(s.technique), Some(s));
}
#[test]
fn full_house_fills_the_last_cell_of_a_house() {
    let (_, s) = test_board();
    let mut b = s;
    b[0] = 0;
    let step = Grid::new(&b).find(Technique::FullHouse).unwrap();
    assert_eq!(step.effect, Effect::Place { cell: 0, digit: s[0] });
    assert_eq!(step.why, format!("r1c1 is the last empty cell in row 1, so it's a {}.", s[0]));
}
#[test]
fn every_detector_is_sound_on_dealt_puzzles() {
    // Every detector, run at every state of 20 seeded minimal puzzles, must
    // agree with the real solution: placements match it, eliminations never
    // remove its digit.
}
#[test]
fn patterns_are_consistent() {
    // On the same states, every step from next_step():
    //  - marks sit on empty cells, on digits that are candidates there before the step;
    //  - every link's endpoints are marks or removed candidates;
    //  - removal masks are non-zero, are subsets of cands, and never hit a mark;
    //  - digit == Some(d) means every mark and every removal is d;
    //  - houses < 27, cells are empty, why is non-empty.
}
```
- `every_detector_is_sound_on_dealt_puzzles` replaces `deductions_never_contradict_the_solution`. Use `StdRng::seed_from_u64(99)`, `sudoku::full_solution`, `sudoku::dig(&s, CELLS, ..)`. At each state, call `grid.find(t)` for every `t` in `Technique::ALL`, then advance with `next_step()`.
- In `basic.rs`, split the pointing test into `pointing_clears_the_rest_of_the_line` (the existing fixture, now `Technique::Pointing`) and `claiming_clears_the_rest_of_the_box`. The claiming fixture: `(0,0){5,6}`, `(0,1){5,7}`, `(1,2){5,8}`; expect `[(idx(1,2), 1<<5)]`.
- Update the other existing staged tests to call their detectors from the new files.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test logic`
Expected: compile errors for the new types and functions.

- [ ] **Step 3: Implement**

- `git mv src/logic.rs src/logic/mod.rs`, then move the detectors as listed under Files. Submodules can read `Grid`'s private fields.
- Every detector returns `Step` with a pattern per the table above and a `why` in the model's wording.
- `next_step` is `Technique::ALL.iter().find_map(|&t| self.find(t))`, guarded by `is_broken`.
- Detectors that scan houses go through them in `HOUSES` order (rows, then columns, then boxes). That makes the first hit predictable: `full_house_fills_the_last_cell_of_a_house` expects "row 1".
- `grade`, `next_placement` and `Game::hint` switch to `step.effect`.
- `deal::level_of` maps `FullHouse` to Easy, and `Pointing` and `Claiming` to Medium.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo fmt --check && cargo clippy --all-targets && cargo test`
Expected: all pass. `real_puzzles_grade_by_their_hardest_technique` keeps its expectations, with `LockedCandidates` replaced by `Pointing`/`Claiming` wherever it was named.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "Give every solver step its pattern and a plain-words reason" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The tutor's reading of the board

**Files:**
- Create: `src/tutor/mod.rs` (declare `mod tutor;` in `src/main.rs`)
- Modify: `src/game.rs` (`apply_step`, `place_answer`)

**Interfaces:**
- Consumes: `Grid::with_candidates`, `Grid::next_step`, `Step`, `Effect` (Task 2); `sudoku::candidates_mask`.
- Produces:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum NoteMode { Full, PairMarks }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advice { WrongDigit(usize), WrongNotes(usize), Step(Step), Stuck }
pub fn read(values: &Board, notes: &[u16; CELLS]) -> (NoteMode, Grid);
pub fn consult(game: &Game) -> (NoteMode, Advice);
// game.rs
impl Game {
    pub fn apply_step(&mut self, step: &Step, mode: NoteMode) -> bool; // false = nothing applied
    pub fn place_answer(&mut self, cell: usize);
}
```
- `read`:
  - `Full` when every empty cell (`values[i] == 0`) has `notes[i] != 0`. A cell's candidates are then `notes[i] & candidates_mask(values, i)`.
  - Otherwise `PairMarks`, and every empty cell's candidates are `candidates_mask(values, i)`. Notes are ignored entirely.
- `consult` checks in this order:
  1. The first cell (index order) whose value is non-zero and differs from the solution gives `WrongDigit`.
  2. In `Full` mode only, the first empty cell whose raw notes lack its answer gives `WrongNotes`.
  3. Otherwise `Step(next_step)`, or `Stuck` when there is no next step.
- `apply_step`:
  - `Place`: select the cell, place the digit, clear it from peer notes (as a correct entry does), push one undo entry, and check for a finished puzzle.
  - `Eliminate` in `Full` mode: one undo entry whose `cell` is the first removal cell, with the other removal cells' previous notes in `peer_notes`. Then clear the masks from the notes.
  - `Eliminate` in `PairMarks` mode: return `false` and change nothing.
- `place_answer`: place `solution[cell]` the same way as `Place`.

- [ ] **Step 1: Write the failing tests** (`src/tutor/mod.rs`)

```rust
fn full_notes(g: &mut Game) { for i in 0..CELLS { if g.values[i] == 0 { g.notes[i] = candidates_mask(&g.values, i); } } }

#[test] fn full_mode_needs_notes_in_every_empty_cell() { /* test_board game: no notes -> PairMarks; full_notes -> Full; clear one cell's notes -> PairMarks */ }
#[test] fn pair_marks_are_never_read_as_candidates() {
    // A lone mark {1} on an empty cell whose answer isn't 1, nothing else noted.
    let (mode, advice) = consult(&g);
    assert_eq!(mode, NoteMode::PairMarks);
    assert!(!matches!(advice, Advice::WrongNotes(_)));
    if let Advice::Step(s) = advice { assert_ne!(s.effect, Effect::Place { cell: marked, digit: 1 }); }
}
#[test] fn a_wrong_digit_comes_first() { /* wrong digit placed AND a wrong note -> WrongDigit(cell) */ }
#[test] fn wrong_notes_are_caught_in_full_mode() { /* full_notes, then remove the answer bit from cell c -> WrongNotes(c) */ }
#[test] fn the_tutor_moves_on_once_you_make_the_elimination() {
    // Full notes on the X-Wing puzzle "087000500003000060150600000200500003000089002000073900000430000900005706040007000",
    // advanced with placements until the easiest step is an elimination. Apply its removals to the notes by hand:
    // the next consult gives a different step.
}
#[test] fn easiest_step_first_in_pair_mark_mode() { /* test_board, no notes -> Step with technique <= HiddenSingle */ }
#[test] fn stuck_when_no_strategy_applies() {
    // Arto Inkala's puzzle needs more than this set. Solve until next_step is None,
    // copy that state into a Game (values + notes = cands, solution from solve_one) -> Stuck.
    let p = parse_board("800000000003600000070090200050007000000045700000100030001000068008500010090000400").unwrap();
}
```
In `src/game.rs`:
```rust
#[test] fn apply_step_places_a_single_and_clears_peer_notes() { /* undo restores value and peer notes */ }
#[test] fn apply_step_removes_notes_in_full_mode_and_one_undo_restores_them() {}
#[test] fn apply_step_refuses_eliminations_in_pair_mark_mode() {}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test tutor game::tests::apply_step`
Expected: compile errors (the `tutor` module doesn't exist).

- [ ] **Step 3: Implement** `read`, `consult`, `apply_step` and `place_answer` as specified above.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo fmt --check && cargo clippy --all-targets && cargo test`
Expected: all pass. While the `tutor` module has no callers outside tests, it may need `#[allow(dead_code)]`. Remove that allow in Task 4.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "Add the tutor's reading of the board: note modes, checks and advice" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The tutor replaces Hint (nudge, apply, counter)

**Files:**
- Create: `src/ui/tutor.rs` (declare `mod tutor;` in `src/ui.rs`), `src/tutor/lessons.rs`
- Modify: `src/game.rs`, `src/app.rs`, `src/ui.rs`, `src/store.rs` (`record` parameter name), `src/logic/mod.rs` (remove `next_placement`), `README.md`

**Interfaces:**
- Consumes: `tutor::{consult, NoteMode, Advice}`, `Game::{apply_step, place_answer}` (Task 3).
- Produces:
```rust
// app.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum TutorStage { Nudge, Explain { stage: u8 }, ShowMe }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct Tutor { pub mode: NoteMode, pub advice: Advice, pub stage: TutorStage }
pub struct App { /* … */ pub tutor: Option<Tutor> }
pub enum Action { /* … */ Tutor, TutorApply, CloseTutor, ExampleStep(i8) }   // BoardAction::Hint removed
impl Tutor { pub fn can_apply(&self) -> bool } // ShowMe+Step(Place) | ShowMe+Step(Eliminate)&&Full | Stuck (any stage)
// game.rs
pub struct Game { /* … */ pub tutor_used: u32 }   // replaces hints_used; `hinted`, `hint()`, prev_hinted removed
impl Game { pub fn use_tutor(&mut self) }          // saturating += 1
// tutor/lessons.rs
pub struct Lesson { pub steps: &'static [&'static str], pub roles: (&'static str, &'static str) }
pub fn lesson(t: Technique) -> Lesson;             // exhaustive; steps may be empty until Task 6
// ui/tutor.rs
pub(super) fn wrap(text: &str, width: usize) -> Vec<String>;
pub(super) fn draw_pane(frame: &mut Frame, area: Rect, tutor: &Tutor, hits: &mut Vec<Hit>);
```
- Saved games:
  - `SavedGame` drops `hinted`.
  - `hints_used` becomes `tutor_used` with `#[serde(alias = "hints_used")]`.
  - `UndoEntry` drops `prev_hinted`.
  - Old files still parse, because serde ignores unknown fields. No version bump.
- Behaviour:
  - `Action::Tutor`:
    - Ignored with no game, a finished game or a paused game.
    - When the tutor is closed: `consult`, open at `Nudge`, call `use_tutor()`. For `WrongDigit`/`WrongNotes`, also select that cell.
    - When open, go one stage deeper: Step advice goes `Nudge` → `ShowMe` (Task 6 inserts `Explain`). Any other advice stays put.
  - `Action::TutorApply`, when `can_apply`:
    - `Step` advice: `apply_step`.
    - `Stuck`: `place_answer` on the selected cell if it's empty, else the first empty cell.
    - Then close the tutor, save, and finish the game if it's complete.
  - `CloseTutor` sets `tutor = None`.
  - After any non-navigation `BoardAction` (including undo) while the tutor is open: `consult` again. If `(mode, advice)` changed, close the tutor.
  - The tutor closes on new game, level start, continue and `start_game`.
- Keys, in `base_key` with a game in play:
  - `t` → `Tutor`.
  - `a` → `TutorApply`, only when `tutor.can_apply()`.
  - `Esc` → `CloseTutor` when the tutor is open.
  - `H` is removed.
- Drawing:
  - Header: `💡` becomes `🎓` showing `tutor_used`.
  - Win screen: `Time … Mistakes … Tutor n`, and `"best times count tutor-free solves only"`.
  - Help row: `("Tutor", "t — names a strategy; t again shows it")`.
  - Footer: `("t", "tutor")`.
  - Button: `Tutor` / `t` / `Action::Tutor`.
- Pane layout:
  - While the tutor is open, the number pad sits at the board's top (`oy`).
  - The pane is `Rect(panel_x, oy + DIGIT_GRID_H + PANEL_GROUP_GAP, PANEL_W, BOARD_H - DIGIT_GRID_H - PANEL_GROUP_GAP)`, 26 rows, with a rounded ACCENT border on `POPUP_BG`.
  - Its title is ` Tutor ` at Nudge and ` {name} ` at ShowMe.
- Pane text. Lines are wrapped at 21. Spec copy is verbatim.
  - PairMarks mode, first line (MUTED): "Going by placed digits — your notes aren't filled in yet."
  - Nudge on a Step: "Look for" (INK), then the technique name in upper case (ACCENT, bold). Keys: `t  what's that?`, `Esc  I'll find it`.
  - WrongDigit: "{rc} is wrong: clear it first."
  - WrongNotes: "Your notes in {rc} cross out something that still fits."
  - Stuck: "Nothing in my book applies here." with key `a  place one digit`.
  - ShowMe:
    - First `why`, then a blank line, then the effect.
    - Place: "{rc} is {d}" (GREEN, bold).
    - A single-digit elimination: "Not {d}:" (RED, bold), then the cells comma-joined.
    - A multi-digit elimination: one line per cell, "{rc}: not {digit_list}".
    - Keys: `a  do it for me` (only when `can_apply`), `Esc  back to solving`.
  - Each key line is a click target for its action.

- [ ] **Step 1: Write the failing tests**

In `src/app.rs`:
```rust
#[test] fn t_opens_a_nudge_and_counts_once_per_opening() { /* t: Nudge, tutor_used 1; t: ShowMe, still 1; Esc: closed; t: Nudge, tutor_used 2 */ }
#[test] fn board_keys_still_work_during_the_nudge() {}
#[test] fn a_applies_the_step_in_show_me() { /* test_board, no notes: t t a -> the single is placed, tutor closed */ }
#[test] fn a_is_not_offered_for_eliminations_in_pair_mark_mode() {
    // X-Wing puzzle advanced with placements until the easiest step is an
    // elimination, no notes: t t then a changes nothing, and the pane has no "do it for me".
}
#[test] fn entering_the_nudged_digit_yourself_closes_the_tutor() {}
#[test] fn undo_closes_a_stale_tutor() { /* enter a wrong digit at W; t -> Advice::WrongDigit(W); u removes it -> tutor is None */ }
#[test] fn a_can_finish_the_puzzle() { /* all but one cell solved; t until ShowMe, then a -> Won overlay, tutor None, !new_best */ }
#[test] fn tutor_is_ignored_on_finished_or_paused_games() {}
#[test] fn starting_a_new_game_closes_the_tutor() {}
#[test] fn tutor_use_costs_the_best_time() { /* replaces a_hinted_solve_counts_but_sets_no_best_time */ }
```
In `src/game.rs`:
```rust
#[test] fn old_saves_with_hints_resume_as_ordinary_entries() {
    // JSON v2 snapshot with "hints_used": 2 and a "hinted": [i] list, and "prev_hinted" in undo entries
    // -> restores; tutor_used == 2; the formerly hinted cell can be erased.
}
```
In `src/ui.rs`, update the labels test to `["Notes", "Undo", "Tutor", "Erase", "Level", "Pause", "Help"]`, and add:
```rust
#[test] fn nudge_pane_names_only_the_strategy() { /* t on test_board: screen contains "Look for" and "HIDDEN SINGLE" or "NAKED SINGLE" and no "r" + digit + "c" cell name in the pane */ }
#[test] fn pair_mark_mode_says_so() { /* "Going by placed digits" visible */ }
#[test] fn wrong_digit_advice_names_the_cell() {}
```
Delete the three `hint_*` tests in `game.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test`
Expected: compile errors (`Action::Tutor`, `tutor_used` and the rest don't exist yet).

- [ ] **Step 3: Implement** everything listed under Behaviour, Keys, Drawing, Pane layout and Pane text above.

`draw_panel` splits into `draw_numpad` and `draw_buttons`. While the tutor is open, `draw_game` draws `draw_numpad` at `oy` and `tutor::draw_pane` below it; otherwise the layout is unchanged. Remove `logic::next_placement`. Update README: a "Tutor" section describing `t`, `t` again, `a` and `Esc`; remove the Hint key; add the `🎓` counter to the features list.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo fmt --check && cargo clippy --all-targets && cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A src README.md
git commit -m "Replace Hint with the tutor: nudge, show me and apply" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Show me: the pattern drawn on your board

**Files:**
- Modify: `src/ui/tutor.rs`, `src/ui.rs` (palette constants, `draw_game` uses the focus board at ShowMe)

**Interfaces:**
- Consumes: `Pattern`, `Effect`, `Role`, `LinkKind` (Task 2); `tutor::read` (Task 3); `Tutor` (Task 4).
- Produces:
```rust
pub(super) struct FocusView<'a> {
    pub values: &'a Board, pub given: &'a [bool; CELLS], pub cands: &'a [u16; CELLS],
    pub step: &'a Step, pub stage: u8,      // 0 houses+cells, 1 +marks+links, 2 +effect; ShowMe uses 2
    pub frame: Color,                        // GRID_BOX for your game, VIOLET for an example (Task 6)
}
pub(super) fn draw_focus_board(frame: &mut Frame, ox: u16, oy: u16, labels_y: u16, view: &FocusView);
```
- Drawing rules (from the spec and the approved mockup):
  - **Backgrounds:** cells in `pattern.houses` get `BAND_BG`. Pattern cells (marks' cells, and `pattern.cells`) get `PAT_BG`. Cells with removals get `ERR_BG`. Every other cell gets `SURFACE`.
  - **Placed digits:** in `INK` bold when they equal `pattern.digit`, otherwise in `DIM`. Fullwidth, as in `draw_board`.
  - **Candidates:**
    - With `digit == Some(d)`, only candidate `d` is drawn, everywhere, in `NOTE`.
    - Otherwise, notes are drawn only in `pattern.cells` and in the cells the effect touches.
    - Candidates come from `view.cands`, the tutor's reading, not the raw notes.
  - **Links (stage ≥ 1):**
    - Each line runs between the two candidate slots. A slot is `(x + 1 + 2*((d-1)%3), y + (d-1)/3)` in the cell's interior.
    - Strong links: `ACCENT`, `┃`/`━`. Weak links: `VIOLET`, `╏`/`╍`.
    - When the endpoints share no row or column, the path goes horizontal from `from`, then vertical to `to`. The corner is `┐ ┘ ┌ └` in the link's colour.
    - Lines overwrite whatever lies between; candidates are drawn again on top.
  - **Chips (stage ≥ 1):** `Key` is fg `BG` on `ACCENT`; `Other` is fg `BG` on `VIOLET`; both bold.
  - **Effect (stage 2):**
    - A removed candidate is fg `RED` on `ERR_BG`, bold, with `Modifier::CROSSED_OUT`.
    - A placed digit's slot is fg `BG` on `GREEN`, bold.
  - **Heavy grid lines** use `view.frame`; thin ones stay `GRID_THIN`.
  - **Clicks:** cells keep their `Select` hit areas.

- [ ] **Step 1: Write the failing tests** (`src/ui/tutor.rs` tests, using the `draw`/`cell_row` helpers from `ui.rs` tests, which are made `pub(super)`)

```rust
#[test] fn show_me_narrows_a_single_digit_pattern() {
    // test_board in pair-mark mode, tutor at ShowMe on a hidden single for digit d:
    // in the board area only the digit d appears among candidates; the placed cell's slot has bg GREEN.
}
#[test] fn show_me_draws_links_between_pattern_candidates() {
    // XY-Wing puzzle "800000046007000020920004005000010800035740000080090004000000060600500000200006013",
    // advanced (Grid + next_step) until grid.find(Technique::XYWing) is Some(step). Copy the state into a Game
    // (notes = cands) and set app.tutor = Some(Tutor { mode: Full, advice: Advice::Step(step), stage: ShowMe })
    // directly, so later strategies can't change which step is shown.
    // Expect: some cell strictly between the pivot and a pincer slot holds '╍' or '╏' with fg VIOLET;
    // the pincers' z slots have bg ACCENT.
}
#[test] fn removed_candidates_are_red_and_struck() { /* same state: a removal slot has fg RED and CROSSED_OUT */ }
#[test] fn show_me_fits_the_minimum_terminal() { /* draw at MIN_W x MIN_H: no panic, "back to solving" visible */ }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test ui::tutor`
Expected: FAIL (ShowMe still draws the normal board).

- [ ] **Step 3: Implement** `draw_focus_board`, and use it from `draw_game` when `tutor.stage == ShowMe` and the advice is a `Step`. Pass `stage: 2`, `frame: GRID_BOX`, and `cands` from `tutor::read`.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo fmt --check && cargo clippy --all-targets && cargo test`
Expected: all pass. Then check by eye: `cargo run --release`, start an Easy game, press `t t`. You should see the narrowed board with a green placement chip.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "Draw the tutor's pattern on your board: chips, links and what goes" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Explain: lessons and worked examples

**Files:**
- Create: `src/tutor/examples.rs`, `src/tutor/examples.txt`
- Modify: `src/tutor/lessons.rs` (steps for all 16 techniques), `src/logic/mod.rs` (`Technique::from_name`), `src/app.rs`, `src/ui/tutor.rs`, `src/ui.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `draw_focus_board`, `FocusView` (Task 5); `Grid::find` (Task 2).
- Produces:
```rust
// logic/mod.rs
impl Technique { pub fn from_name(name: &str) -> Option<Technique> }  // case-insensitive on name()
// tutor/examples.rs
pub struct Example { pub values: Board, pub cands: [u16; CELLS], pub step: Step }
pub fn example(t: Technique) -> Option<Example>;   // parsed from include_str!("examples.txt"); step = Grid::with_candidates(..).find(t)
pub fn find_examples(wanted: &[Technique], seed: u64, max_puzzles: usize) -> Vec<String>;
```
- `examples.txt` format:
  - One example per line: `<display name>|<81 digits, 0 = empty>|<81 candidate masks as 3 lowercase hex digits each, 243 chars>`.
  - Lines starting with `#` are comments.
- `find_examples` picks the examples:
  - Deal minimal puzzles from `StdRng::seed_from_u64(seed)` (`full_solution` + `dig(.., CELLS, ..)`) and walk each solve with `next_step`.
  - At every state, for each wanted `t` with `grid.find(t)` → `Some(step)`, the state is a candidate.
  - Keep the candidate with the fewest empty cells, breaking ties by fewest `pattern.marks`.
  - Stop looking for a technique after 25 candidates. Stop altogether after `max_puzzles`.
  - Return one line per technique found, and print the missing ones to stderr.
- `main.rs`: `--find-examples [names…]`.
  - With no names, it looks for every technique with no example yet; otherwise for the named ones.
  - Seed 7, 3000 puzzles. Print the lines to stdout.
- Explain stage: `Action::Tutor` from `Nudge` with Step advice goes to `Explain { stage: 0 }`. From `Explain`, it goes to `ShowMe`. Update Task 4's tests that press `t` to reach ShowMe (`t_opens_a_nudge…`, `a_applies…`, `a_is_not_offered…`, `a_can_finish…`): they now need one more `t`.
- Keys in Explain:
  - `←`/`h` and `→`/`l` send `ExampleStep(-1/+1)`, clamped to stages 0..=2.
  - `t` and `Esc` work as before.
  - All board keys are ignored.
- The board in Explain:
  - `draw_focus_board` with the example's `values`/`cands`/`step`, `given` all true on placed cells, `frame: VIOLET`, at the current stage.
  - Board cells register no `Select` hits.
- The header in Explain:
  - Top line: ` TUDOKU `, then ` EXAMPLE ` (fg `BG` on `VIOLET`, bold), then `"  {name}, on a sample board, not your game"` (MUTED).
  - Bottom line: `"{name}  ·  step {n} of 3   ← → steps through it"`.
- The pane in Explain:
  - Full height: `Rect(panel_x, oy, PANEL_W, BOARD_H)`, titled ` {name} `.
  - The lesson steps, numbered `1 ` to `5 `, each followed by a blank line.
  - A legend of three lines: a Key chip plus `roles.0`, an Other chip plus `roles.1`, and a struck red chip plus "removed". For a `Place` step, the third line is a green chip plus "placed".
  - Keys: `← →  step`, `t  show me in my game`, `Esc  back to solving`.
- Lessons: plain words, 3–5 steps. The Skyscraper lesson in Task 7 is the model for tone. The lessons for the 16 techniques here are written in the same voice.
- While the game is paused, the Explain board and pane aren't drawn (the paused game view takes over). The tutor stage is kept.

- [ ] **Step 1: Write the failing tests**

```rust
// tutor/lessons.rs
#[test] fn lesson_text_fits_the_pane() {
    // for every t: 3..=5 steps; every wrapped line <= 21 cols; numbered steps + blanks + legend(3) + keys(3) + 2 spacing rows <= BOARD_H - 3
}
// tutor/examples.rs
#[test] fn every_technique_has_a_worked_example() {
    for &t in Technique::ALL { let e = example(t).unwrap_or_else(|| panic!("{t:?}")); assert_eq!(e.step.technique, t); }
}
#[test] fn find_examples_finds_a_common_technique() { let lines = find_examples(&[Technique::Pointing], 7, 200); assert_eq!(lines.len(), 1); assert!(lines[0].starts_with("pointing|")); }
// app.rs
#[test] fn explain_comes_between_nudge_and_show_me() {}            // update t_opens_a_nudge… to Nudge → Explain{0} → ShowMe
#[test] fn arrows_step_through_the_example() { /* → → → stays at 2; ← ← ← stays at 0 */ }
#[test] fn pausing_during_explain_hides_the_example() { /* set_focus(false): Paused overlay, screen lacks "EXAMPLE"; p resumes, stage still Explain */ }
#[test] fn clicks_on_the_example_do_not_touch_your_game() { /* click every board cell in Explain: game.selected and values unchanged */ }
// ui/tutor.rs
#[test] fn explain_shows_the_example_not_your_game() { /* "EXAMPLE" in header; board corner '┏' has fg VIOLET */ }
#[test] fn explain_fits_the_minimum_terminal() {}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test`
Expected: compile errors or FAIL (no `examples` module, no Explain stage yet).

- [ ] **Step 3: Implement** `from_name`, `examples.rs`, the `--find-examples` CLI, the Explain stage (keys, board, header, pane), and the lesson steps for all 16 techniques.

- [ ] **Step 4: Generate the examples**

Run: `cargo run --release -- --find-examples > /tmp/ex.txt; cat /tmp/ex.txt`
Expected: 16 lines, one per current technique. Write them to `src/tutor/examples.txt` after a `# name|values|candidate masks (hex)` header comment.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo fmt --check && cargo clippy --all-targets && cargo test`
Expected: all pass. Then check by eye: `t t` shows the EXAMPLE board, and `← →` steps through it.

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "Explain each strategy with steps and a worked example on a sample board" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Single-digit patterns (Hard)

**Files:** Create `src/logic/single_digit.rs`. Modify the files the Recipe lists.

Follow the Recipe. Variants: `Skyscraper`, `TwoStringKite`, `Crane`, `EmptyRectangle`, all at level Hard. "Conjugate pair" means a house with exactly two cells holding the digit `d`.

- **Skyscraper.** Two parallel lines (both rows or both columns), each a conjugate pair on `d`. One end of each (the bases) shares a cross line. The other ends (the tops) don't share a cross line; if they did, it would be an X-Wing. Remove `d` from cells that see both tops.
  - Pattern: digit `d`; houses = the two lines; Key = tops; Other = bases; links: Strong top–base on each line, Weak base–base.
  - Test `skyscraper_clears_cells_seeing_both_tops`, staged on digit 4: `(0,1)`, `(6,1)`, `(1,5)`, `(6,5)`, the target `(0,3){4,9}`, and padding `(0,7){4,8}`, `(6,8){4,8}`. Expect `[(idx(0,3), 1<<4)]`.
  - Lesson (model text):
    1. "Find two columns where 4 fits in only two cells."
    2. "One end of each sits in the same row: the base. Both can't be the digit."
    3. "So at least one of the other ends, the tops, is the digit."
    4. "Any cell that sees both tops can't be it."
  - Roles: ("top", "base").
- **2-String Kite.** A row conjugate pair `{R1, R2}` and a column conjugate pair `{C1, C2}` on `d`, with `R2` and `C1` in the same box. None of the four cells is shared. Remove `d` from cells seeing both `R1` and `C2`.
  - Pattern: houses = [row, column, box]; Key = `R1`, `C2`; Other = `R2`, `C1`; links: Strong `R1–R2`, Strong `C1–C2`, Weak `R2–C1`.
  - Test: `kite_clears_the_cell_where_the_strings_meet`. Roles: ("end", "knot").
- **Crane.** A box conjugate pair `{A1, A2}` and a line conjugate pair `{B1, B2}` (row or column, outside that box) on `d`, where `A2` and `B1` share a row or column. Remove `d` from cells seeing both `A1` and `B2`.
  - Pattern: houses = [box, line]; Key = `A1`, `B2`; Other = `A2`, `B1`; links: Strong, Strong, Weak `A2–B1`.
  - Test: `crane_links_a_box_pair_to_a_line_pair`. Roles: ("end", "link").
- **Empty Rectangle.**
  - In box `B`, every `d` lies in row `R` or column `C` of the box, with at least two `d`s, and they aren't all in `R` nor all in `C`.
  - Case 1: a column conjugate pair `{(R, X), (r2, X)}` with `X` outside `B`'s columns and `r2` outside `B`'s rows removes `d` from `(r2, C)`.
  - Case 2: a row conjugate pair `{(r1, C), (r1, c2)}` with `r1` outside `B`'s rows and `c2` outside `B`'s columns removes `d` from `(R, c2)`.
  - Pattern: houses = [box, pair line]; Other = the box's `d`s; Key = the pair; links: Strong across the pair.
  - Tests: `empty_rectangle_with_a_column_pair` and `empty_rectangle_with_a_row_pair`. Roles: ("pair", "box").

- [ ] **Step 1:** Write the five staged tests above.
- [ ] **Step 2:** Run `cargo test single_digit`. Expected: compile errors.
- [ ] **Step 3:** Implement the detectors, and the variants per the Recipe.
- [ ] **Step 4:** Add the lessons, then `cargo run --release -- --find-examples "Skyscraper" "2-String Kite" "Crane" "Empty Rectangle"` and append the output to `examples.txt`.
- [ ] **Step 5:** Run `cargo fmt --check && cargo clippy --all-targets && cargo test`. Expected: all pass.
- [ ] **Step 6:** Commit: `"Teach the single-digit patterns: Skyscraper, 2-String Kite, Crane, Empty Rectangle"`

---

### Task 8: Finned and sashimi fish (Master)

**Files:** Modify `src/logic/fish.rs`, plus the files the Recipe lists.

Variants: `FinnedXWing` (k=2), `FinnedSwordfish` (k=3), `FinnedJellyfish` (k=4).

- **The rule:**
  - Base lines: choose `k` lines (rows, or columns), each with at least one `d`.
  - Cover lines: choose `k` cross lines.
  - Fins are the base `d`s outside the cover lines. There must be at least one fin, and all fins must sit in one box `F`.
  - Remove `d` from cells that are in a cover line, not in a base line, in box `F`, and hold `d`.
  - It's sashimi when some base line has fewer than two `d`s inside the cover lines. Say "sashimi" in `why` in that case; the technique stays the same.
  - Skip any combination that is a plain fish (no fins). Plain fish are found earlier.
- **Pattern:** digit `d`; houses = base lines + cover lines + box `F`; Key = base `d`s inside the cover lines; Other = fins.
- **Tests:** `finned_x_wing_clears_inside_the_fin_box` and `sashimi_swordfish_says_sashimi` (`why` contains "sashimi"). Roles: ("fish", "fin").

- [ ] **Step 1:** Write the two staged tests above.
- [ ] **Step 2:** Run `cargo test fish`. Expected: compile errors.
- [ ] **Step 3:** Implement `finned_fish(k)`, and the variants per the Recipe.
- [ ] **Step 4:** Add the lessons and generate the examples per the Recipe.
- [ ] **Step 5:** Run the full check. Expected: all pass.
- [ ] **Step 6:** Commit: `"Teach finned and sashimi fish"`

---

### Task 9: W-Wing and WXYZ-Wing (Master)

**Files:** Modify `src/logic/wings.rs`, plus the files the Recipe lists.

- **W-Wing.**
  - Two bivalue cells `A` and `B` with the same pair `{x, y}` that don't see each other.
  - A conjugate pair on `x`, `{P, Q}`, with `P` seeing `A`, `Q` seeing `B`, and `P`, `Q` distinct from `A`, `B`.
  - Remove `y` from cells seeing both `A` and `B`.
  - Pattern: cells `[A, B, P, Q]`; Key = `y` in `A` and `B`; Other = `x` in `A`, `B`, `P`, `Q`; links: Weak `A(x)–P(x)`, Strong `P(x)–Q(x)`, Weak `Q(x)–B(x)`.
  - Test: `w_wing_clears_the_other_digit`. Roles: ("removed digit", "bridge").
- **WXYZ-Wing.**
  - A pivot cell with 2..=4 candidates, plus three wing cells among its peers, each with 2..=4 candidates.
  - The union of all four cells' candidates is exactly 4 digits.
  - For every digit but one (`Z`), all cells holding it see each other. `Z` is in at least two of the cells.
  - Remove `Z` from cells (outside the four) that see every `Z`-cell of the four.
  - Pattern: cells = the four; Key = `Z` candidates; Other = the rest.
  - Test: `wxyz_wing_clears_the_loose_digit`. Roles: ("loose digit", "locked digits").

- [ ] **Step 1:** Write the two staged tests above.
- [ ] **Step 2:** Run `cargo test wings`. Expected: compile errors.
- [ ] **Step 3:** Implement the two detectors, and the variants per the Recipe.
- [ ] **Step 4:** Add the lessons and generate the examples per the Recipe.
- [ ] **Step 5:** Run the full check. Expected: all pass.
- [ ] **Step 6:** Commit: `"Teach W-Wing and WXYZ-Wing"`

---

### Task 10: Unique Rectangle and BUG+1 (Master)

**Files:** Create `src/logic/uniqueness.rs`. Modify the files the Recipe lists.

These are safe only because every dealt puzzle has exactly one solution. Say so in the module doc.

- **Unique Rectangle.**
  - Four empty cells at rows `{r1, r2}` × columns `{c1, c2}`, lying in exactly two boxes, all holding both `x` and `y`.
  - **Type 1:** three corners are exactly `{x, y}`. Remove `x` and `y` from the fourth.
  - **Type 2:** two corners sharing a row or column are exactly `{x, y}`. The other two are exactly `{x, y, z}` with the same `z`. Remove `z` from cells seeing both of those.
  - **Type 4:** two corners are exactly `{x, y}`. The other two (the roof) share a house `H`, and in `H` the digit `x` appears only in the roof cells. Remove `y` from both roof cells. Also try with `x` and `y` swapped.
  - `why` names the type, e.g. "…would leave two solutions (Unique Rectangle type 2)…".
  - Pattern: cells = the four.
    - Other = `x` and `y` in the corners that are exactly `{x, y}`.
    - Key = in the remaining corners, `x` and `y` where they aren't removed: none in type 1; both in type 2; only the conjugate digit in type 4.
    - Extra digits aren't marked.
  - Tests: `unique_rectangle_type_1`, `unique_rectangle_type_2` and `unique_rectangle_type_4`. Roles: ("roof", "floor").
- **BUG+1.**
  - Every empty cell has exactly two candidates except one cell `T`, which has three.
  - Place in `T` the digit that appears three times in `T`'s row. If the row is ambiguous, use the column, then the box.
  - Pattern: cells `[T]`; Key = `T`'s three candidates.
  - Test: `bug_plus_one_places_the_odd_digit`. Roles: ("three-way cell", "-").

- [ ] **Step 1:** Write the four staged tests above.
- [ ] **Step 2:** Run `cargo test uniqueness`. Expected: compile errors.
- [ ] **Step 3:** Implement the two detectors, and the variants per the Recipe.
- [ ] **Step 4:** Add the lessons and generate the examples per the Recipe.
- [ ] **Step 5:** Run the full check. Expected: all pass.
- [ ] **Step 6:** Commit: `"Teach Unique Rectangles and BUG+1"`

---

### Task 11: Simple Colouring (Master)

**Files:** Create `src/logic/colouring.rs`. Modify the files the Recipe lists.

- **The rule:**
  - For digit `d`, join conjugate pairs into connected groups and colour each group alternately A and B. Only groups with at least 3 cells count.
  - **Colour wrap:** two cells of the same colour see each other. That colour is false, so remove `d` from every cell of that colour.
  - **Colour trap:** a cell outside the group that holds `d` and sees one cell of each colour. Remove `d` from it.
  - `why` says "wrap" or "trap".
- **Pattern:** digit `d`; links: Strong along each conjugate pair used.
  - For a trap: Key = colour A, Other = colour B.
  - For a wrap: Key = the surviving colour only. The clashing colour's candidates appear only as removals, so they aren't also marks.
- **Tests:** `colour_trap_clears_a_cell_seeing_both_colours` and `colour_wrap_clears_the_clashing_colour`. Roles: ("colour A", "colour B").

- [ ] **Step 1:** Write the two staged tests above.
- [ ] **Step 2:** Run `cargo test colouring`. Expected: compile errors.
- [ ] **Step 3:** Implement the detector, and the variant per the Recipe.
- [ ] **Step 4:** Add the lesson and generate the example per the Recipe.
- [ ] **Step 5:** Run the full check. Expected: all pass.
- [ ] **Step 6:** Commit: `"Teach Simple Colouring"`

---

### Task 12: Chains, and Zen gets its own strategies

**Files:** Create `src/logic/chains.rs`. Modify the files the Recipe lists, plus `src/deal.rs` (remove the temporary Zen rule).

- **One engine.** Nodes are candidates `(cell, digit)`.
  - Strong links: a conjugate pair on a digit, or the two candidates of a bivalue cell.
  - Weak links: the same digit in cells that see each other, or two digits in the same cell.
  - A chain alternates strong and weak links, starts and ends with a strong link, has at least 3 links, at most **12** links, and never repeats a node.
- **Search.** Breadth-first from each start node over `(node, next link must be strong)` states, keeping parent pointers. Over all starts, report the shortest chain that removes something. Skip a path whose reconstruction repeats a node, and skip any chain whose removals would hit one of its own nodes.
- **What a chain removes.** Endpoints are `(A, x)` and `(B, y)`.
  - If `x == y`, remove `x` from cells (not `A` or `B`) that see both.
  - If `x != y` and `A` sees `B` (`A ≠ B`), remove `y` from `A` and `x` from `B`, where present.
- **The flavours**, in this order:
  1. `RemotePair`:
     - Take the bivalue cells that all hold the same pair `{x, y}`, joined when they see each other, in groups of at least 4. Colour each group by parity.
     - A cell outside the group that sees one cell of each colour loses both `x` and `y`.
     - Pattern: Key = the pair in cells of one colour; Other = the other colour; links: Weak between consecutive cells along the path used.
  2. `XChain`: only digit `d`. Strong links are conjugate pairs; weak links are same-digit sees.
  3. `XYChain`: only bivalue cells. Strong links are inside a cell; weak links are between cells on the same digit.
  4. `Aic`: all links.
- **Chain patterns:** digit `Some(d)` only for `XChain`; Key = the two endpoints; Other = the inner nodes; links = the chain's links with their kinds.
- **Tests:**
  - `remote_pair_clears_both_digits`
  - `x_chain_of_five_links` (a staged 5-link chain whose shorter sub-patterns aren't present)
  - `xy_chain_clears_the_end_digit`
  - `aic_with_mixed_links`
  - `chains_never_exceed_twelve_links`: the longest chain found on 20 seeded puzzles is ≤ 12 links.
- **Roles:** ("ends", "chain").
- **Zen:** `level_of` maps all four to Zen. Remove the temporary clause from `accepts`. Replace `zen_deals_master_puzzles_until_it_has_strategies` with `zen_deals_only_chain_and_set_puzzles`: `!Difficulty::Zen.accepts(Technique::Jellyfish) && Difficulty::Zen.accepts(Technique::XChain)`.

- [ ] **Step 1:** Write the tests above, and change `zen_deals_master_puzzles_until_it_has_strategies` as described.
- [ ] **Step 2:** Run `cargo test chains deal`. Expected: compile errors.
- [ ] **Step 3:** Implement the engine, the four flavours, and the variants per the Recipe. Remove the temporary rule from `accepts`.
- [ ] **Step 4:** Add the lessons and generate the examples per the Recipe.
- [ ] **Step 5:** Run the full check. Expected: all pass. Then `cargo run --release -- --offline-check`: the Zen line names a chain technique.
- [ ] **Step 6:** Commit: `"Teach chains, and make Zen the chains-and-sets level"`

---

### Task 13: Sets (Zen)

**Files:** Create `src/logic/sets.rs`. Modify the files the Recipe lists.

- **ALS.** An almost locked set is `n` empty cells (1 ≤ `n` ≤ 4) inside one house, holding exactly `n + 1` digits between them.
  - Enumerate them once per call: `struct Als { cells: Vec<usize>, digits: u16, house: usize }`.
- **Restricted common digit (RCC)** of two disjoint ALSs: a digit in both, where every cell holding it in one ALS sees every cell holding it in the other.
- **ALS-XZ.**
  - ALSs `A` and `B` (disjoint), with RCC `X`, and a common digit `Z ≠ X`.
  - Remove `Z` from cells outside `A ∪ B` that see every `Z`-cell of `A ∪ B`.
  - Pattern: houses = [A.house, B.house]; cells = A ∪ B; Key = `Z`; Other = `X`; links: Weak between one `X`-cell of `A` and one of `B`.
- **ALS-XY-Wing.**
  - ALSs `A`, `B` and `C`, pairwise disjoint. RCC(A, C) = `X`, RCC(B, C) = `Y`, with `X ≠ Y`.
  - `Z` is in both `A` and `B`, with `Z ∉ {X, Y}`.
  - Remove `Z` from cells seeing every `Z`-cell of `A ∪ B`.
  - Pattern: Key = `Z`; Other = `X` and `Y`.
- **Sue de Coq** (basic form).
  - Two or three empty cells `C` where box `B` meets line `L`, holding digits `V`, with `|V| = |C| + 2`.
  - A bivalue cell `Dl` in `L` outside `B`, with `cands ⊂ V`.
  - A bivalue cell `Db` in `B` outside `L`, with `cands ⊂ V`.
  - `Dl` and `Db` share no digit.
  - Let `rest = V ∖ (Dl ∪ Db)`. Remove `Dl ∪ rest` from `L` (except `C` and `Dl`), and `Db ∪ rest` from `B` (except `C` and `Db`).
  - Pattern: houses = [B, L]; cells = C + Dl + Db; Key = `V` in `C`; Other = `Dl` and `Db` digits.
- **Death Blossom.**
  - A stem cell `S` with 2 or 3 candidates.
  - For each stem candidate `dᵢ`, a petal: an ALS of at most 3 cells, not containing `S`, holding `dᵢ`, where every `dᵢ`-cell sees `S`. Petals are pairwise disjoint.
  - A digit `Z`, not a stem candidate, in every petal.
  - Remove `Z` from cells outside the petals and stem that see every `Z`-cell of every petal.
  - Pattern: cells = S + petals; Key = `Z`; Other = the stem's candidates and each petal's `dᵢ`; links: Weak `(S, dᵢ)`–one `dᵢ`-cell of petal `i`.
- **Tests:** `als_xz_clears_the_shared_digit`, `als_xy_wing_clears_z`, `sue_de_coq_splits_the_intersection` and `death_blossom_with_two_petals`.
- **Roles:** ALS-XZ and ALS-XY-Wing ("removed digit", "link digit"); Sue de Coq ("intersection", "helpers"); Death Blossom ("removed digit", "stem & petals").

- [ ] **Step 1:** Write the four staged tests above.
- [ ] **Step 2:** Run `cargo test sets`. Expected: compile errors.
- [ ] **Step 3:** Implement ALS enumeration and the four detectors, and the variants per the Recipe.
- [ ] **Step 4:** Add the lessons and generate the examples per the Recipe. If a set technique isn't found within 3000 puzzles, rerun with `--find-examples` and a larger `max_puzzles` passed through an optional `--puzzles N` flag.
- [ ] **Step 5:** Run the full check. Expected: all pass.
- [ ] **Step 6:** Commit: `"Teach ALS-XZ, ALS-XY-Wing, Sue de Coq and Death Blossom"`

---

### Task 14: Dealing speed, final docs, end-to-end check

**Files:** Modify `src/main.rs` (`offline_check` prints the median of 5 deals per level), `README.md`, and `src/deal.rs` (only if the pool is needed).

- [ ] **Step 1: Measure.** Run `cargo run --release -- --offline-check`, changed to deal 5 puzzles per level with `deal_fast` and print the median time.
Expected: every level's median is under **2 s**.
- [ ] **Step 2: Add the pool, only if Step 1 failed.**
  - Generate 200 puzzles per slow level, using `--offline-check --pool <level>`, into `src/pool/<level>.txt` (81 digits per line).
  - `deal_fast(level)` picks a random line, verifies `is_unique` and `accepts`, and remaps digits with a random permutation of 1-9 so repeats aren't obvious.
  - Add a test that every pool line is unique and graded at its level.
- [ ] **Step 3: Check the debug test time.** `cargo test` (debug) must finish in under 60 s.
  - If `every_level_deals_a_unique_puzzle_graded_at_that_level` is the slow one, deal Zen there with `deal_fast`.
  - If `every_detector_is_sound_on_dealt_puzzles` is the slow one, cut it from 20 puzzles to 10.
- [ ] **Step 4: Update the README.**
  - The levels table, with the final families per level (from the spec).
  - A Tutor section: the three steps, `a`, "notes as full candidates vs pair marks", and that it costs the best time.
  - The strategy list.
  - The `--find-examples` developer note.
- [ ] **Step 5: Play through by hand.**
  1. Deal a Hard puzzle and fill in full candidates at some point.
  2. Press `t` (nudge), `t` (explain; step through with `← →`), `t` (show me), `a`.
  3. Repeat on Zen.
  4. Resume your own converted save and check that it opens as Master.
- [ ] **Step 6: Run the full check.** `cargo fmt --check && cargo clippy --all-targets && cargo test`. Expected: all pass.
- [ ] **Step 7: Commit:** `"Check dealing speed and document the tutor"`
