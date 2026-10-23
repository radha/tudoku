//! The tutor's pane, in the side panel under the number pad: first the
//! name of a strategy to look for, then the strategy explained beside a
//! worked example on a sample board, then the step itself in r#c# terms
//! on your own board, with the keys that go on from there.

use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{
    ACCENT, BAND_BG, BG, BOARD_H, BOARD_W, CELL_H, CELL_W, DIM, ERR_BG, GREEN, GRID_THIN, Hit, INK,
    MUTED, NOTE, PANEL_W, PAT_BG, RED, SURFACE, VIOLET, cell_screen, grid_glyph, popup_block,
    push_hit,
};
use crate::app::{Action, EXAMPLE_STAGES, Tutor, TutorStage};
use crate::logic::{Effect, LinkKind, Role, Step, Technique, digit_list, rc};
use crate::sudoku::{Board, CELLS, HOUSES};
use crate::tutor::examples::Example;
use crate::tutor::lessons::{Lesson, lesson};
use crate::tutor::{Advice, NoteMode};

/// Columns of text: the panel's width, less the border and a column of
/// padding on each side.
pub(crate) const TEXT_W: usize = PANEL_W as usize - 4;

/// `text` broken at spaces into lines at most `width` columns wide. A
/// word too long for a line of its own is split.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for mut word in text.split_whitespace() {
        loop {
            let used = line.chars().count();
            let gap = usize::from(used > 0);
            if used + gap + word.chars().count() <= width {
                if gap > 0 {
                    line.push(' ');
                }
                line.push_str(word);
                break;
            }
            if used > 0 {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            let cut = word
                .char_indices()
                .nth(width)
                .map_or(word.len(), |(b, _)| b);
            lines.push(word[..cut].to_string());
            word = &word[cut..];
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// A strategy's name as a title: "Naked single", "X-Wing".
pub(super) fn heading(t: Technique) -> String {
    let mut chars = t.name().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// `text`, wrapped to the pane, in `style`.
fn para(lines: &mut Vec<Line<'static>>, text: &str, style: Style) {
    lines.extend(
        wrap(text, TEXT_W)
            .into_iter()
            .map(|l| Line::styled(l, style)),
    );
}

/// Wrapped `lines` in `style`, at most `room` of them: when some have to
/// go, the last one kept ends in "…".
fn cut(mut lines: Vec<String>, room: usize, style: Style) -> Vec<Line<'static>> {
    let cut = lines.len() > room;
    lines.truncate(room);
    let last = lines.pop().map(|mut last| {
        if !cut {
            return Line::styled(last, style);
        }
        // Make room for the "…", a word at a time.
        while last.chars().count() >= TEXT_W {
            match last.rfind(' ') {
                Some(at) => last.truncate(at),
                None => _ = last.pop(),
            }
        }
        Line::from(vec![
            Span::styled(last, style),
            Span::styled("…", Style::default().fg(MUTED)),
        ])
    });
    lines
        .into_iter()
        .map(|l| Line::styled(l, style))
        .chain(last)
        .collect()
}

/// A key the pane offers, and what it does.
struct Key {
    key: &'static str,
    what: &'static str,
    action: Action,
}

const BACK: Key = Key {
    key: "Esc",
    what: "back to solving",
    action: Action::CloseTutor,
};

/// The pane's frame alone, for while the game is paused: like the board,
/// what the tutor has to say waits for the clock.
pub(super) fn draw_pane_frame(frame: &mut Frame, area: Rect) {
    frame.render_widget(popup_block("Tutor"), area);
}

/// Draw the pane into `area`. Each key line is also a click target.
pub(super) fn draw_pane(frame: &mut Frame, area: Rect, tutor: &Tutor, hits: &mut Vec<Hit>) {
    let title = match (&tutor.advice, tutor.stage) {
        (Advice::Step(step), TutorStage::Explain { .. } | TutorStage::ShowMe) => {
            heading(step.technique)
        }
        _ => "Tutor".to_string(),
    };
    let block = popup_block(&title);
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);

    let ink = Style::default().fg(INK);
    let muted = Style::default().fg(MUTED);
    let mut lines = vec![Line::from("")];
    if tutor.mode == NoteMode::PairMarks {
        para(
            &mut lines,
            "Going by placed digits — your notes aren't filled in yet.",
            muted,
        );
        lines.push(Line::from(""));
    }
    let keys = match (&tutor.advice, tutor.stage) {
        (Advice::Step(step), TutorStage::Nudge) => {
            lines.push(Line::styled("Look for", ink));
            para(
                &mut lines,
                &step.technique.name().to_uppercase(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            );
            vec![
                Key {
                    key: "t",
                    what: "what's that?",
                    action: Action::Tutor,
                },
                Key {
                    key: "Esc",
                    what: "I'll find it",
                    action: Action::CloseTutor,
                },
            ]
        }
        // `draw_lesson` explains a strategy that has a worked example;
        // without one, the explaining is the step itself.
        (Advice::Step(step), TutorStage::Explain { .. } | TutorStage::ShowMe) => {
            let apply = Key {
                key: "a",
                what: "do it for me",
                action: Action::TutorApply,
            };
            let keys = if tutor.can_apply() {
                vec![apply, BACK]
            } else {
                vec![BACK]
            };
            // What the step does always shows in full: a reason too long
            // for the pane gives way. Blank lines go after the reason and
            // after what it does.
            let mut effect = Vec::new();
            effect_lines(&mut effect, step);
            let used = lines.len() + 1 + effect.len() + 1 + keys.len();
            let room = usize::from(inner.height).saturating_sub(used);
            lines.extend(cut(wrap(&step.why, TEXT_W), room, ink));
            lines.push(Line::from(""));
            lines.extend(effect);
            keys
        }
        (Advice::WrongDigit(cell), _) => {
            let text = format!("{} is wrong: clear it first.", rc(*cell));
            para(&mut lines, &text, ink);
            vec![BACK]
        }
        (Advice::WrongNotes(cell), _) => {
            let text = format!(
                "Your notes in {} cross out something that still fits.",
                rc(*cell)
            );
            para(&mut lines, &text, ink);
            vec![BACK]
        }
        (Advice::Stuck, _) => {
            para(&mut lines, "Nothing in my book applies here.", ink);
            let place = Key {
                key: "a",
                what: "place one digit",
                action: Action::TutorApply,
            };
            vec![place, BACK]
        }
    };
    lines.push(Line::from(""));
    put_keys(frame, area, inner, lines, &keys, hits);
}

/// Draw `lines` and then `keys` into the pane's `inner` area, each key a
/// click target across the pane. The keys stay in view even if the text
/// runs long. Returns the row each key is on.
fn put_keys(
    frame: &mut Frame,
    area: Rect,
    inner: Rect,
    mut lines: Vec<Line<'static>>,
    keys: &[Key],
    hits: &mut Vec<Hit>,
) -> Vec<u16> {
    let muted = Style::default().fg(MUTED);
    let room = usize::from(inner.height).saturating_sub(keys.len());
    if lines.len() > room {
        lines.truncate(room);
        if let Some(last) = lines.last_mut() {
            *last = Line::styled("…", muted);
        }
    }
    let mut rows = Vec::new();
    for key in keys {
        let y = inner.y + lines.len() as u16;
        push_hit(
            hits,
            Rect::new(area.x + 1, y, area.width.saturating_sub(2), 1),
            Some(key.action),
        );
        rows.push(y);
        // Keys line up in a column, unless that would run off the pane.
        let column = if 5 + key.what.chars().count() <= TEXT_W {
            5
        } else {
            key.key.chars().count() + 2
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<column$}", key.key),
                Style::default().fg(INK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(key.what, muted),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    rows
}

/// The pane beside a worked example, the whole side panel: the strategy
/// in numbered steps, a legend for the example's colours, and the keys.
pub(super) fn draw_lesson(frame: &mut Frame, area: Rect, example: &Example, hits: &mut Vec<Hit>) {
    let t = example.step.technique;
    let lesson = lesson(t);
    let block = popup_block(&heading(t));
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);

    let ink = Style::default().fg(INK);
    let number = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);
    let mut lines = vec![Line::from("")];
    for (n, step) in lesson.steps.iter().enumerate() {
        for (k, line) in wrap(&format!("{} {step}", n + 1), TEXT_W)
            .into_iter()
            .enumerate()
        {
            lines.push(match line.split_once(' ') {
                Some((num, rest)) if k == 0 => Line::from(vec![
                    Span::styled(format!("{num} "), number),
                    Span::styled(rest.to_string(), ink),
                ]),
                _ => Line::styled(line, ink),
            });
        }
        lines.push(Line::from(""));
    }
    legend(&mut lines, &lesson, &example.step);
    lines.push(Line::from(""));

    let keys = [
        Key {
            key: "← →",
            what: "step",
            action: Action::ExampleStep(1),
        },
        Key {
            key: "t",
            what: "show me in my game",
            action: Action::Tutor,
        },
        BACK,
    ];
    let rows = put_keys(frame, area, inner, lines, &keys, hits);
    // A click on the left arrow (or the padding beside it) steps back;
    // anywhere else on that line, on.
    if let Some(&y) = rows.first() {
        let back = Rect::new(area.x + 1, y, inner.x + 2 - (area.x + 1), 1);
        push_hit(hits, back, Some(Action::ExampleStep(-1)));
    }
}

/// What the example's colours mean: a chip for each role the strategy
/// marks, then what the step removes or places.
fn legend(lines: &mut Vec<Line<'static>>, lesson: &Lesson, step: &Step) {
    let muted = Style::default().fg(MUTED);
    let bold = Modifier::BOLD;
    let chip = |fill| Style::default().fg(BG).bg(fill).add_modifier(bold);
    for (role, name, fill) in [
        (Role::Key, lesson.roles.0, ACCENT),
        (Role::Other, lesson.roles.1, VIOLET),
    ] {
        let mark = step.pattern.marks.iter().find(|m| m.role == role);
        if let Some(m) = mark.filter(|_| !name.is_empty()) {
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", m.digit), chip(fill)),
                Span::styled(format!(" {name}"), muted),
            ]));
        }
    }
    lines.push(match &step.effect {
        Effect::Place { digit, .. } => Line::from(vec![
            Span::styled(format!(" {digit} "), chip(GREEN)),
            Span::styled(" placed", muted),
        ]),
        Effect::Eliminate(removals) => {
            let digit = removals
                .first()
                .map_or(0, |&(_, mask)| mask.trailing_zeros());
            let red = Style::default().fg(RED).bg(ERR_BG).add_modifier(bold);
            Line::from(vec![
                Span::styled(" ", red),
                Span::styled(digit.to_string(), red.add_modifier(Modifier::CROSSED_OUT)),
                Span::styled(" ", red),
                Span::styled(" removed", muted),
            ])
        }
    });
}

/// The header over a worked example: an EXAMPLE badge, so the sample
/// board never reads as your game, and how far the example has got.
pub(super) fn explain_header(t: Technique, stage: u8) -> (Line<'static>, Line<'static>) {
    let name = heading(t);
    let badge = |fill| {
        Style::default()
            .fg(BG)
            .bg(fill)
            .add_modifier(Modifier::BOLD)
    };
    let muted = Style::default().fg(MUTED);
    let top = Line::from(vec![
        Span::styled(" TUDOKU ", badge(ACCENT)),
        Span::raw("  "),
        Span::styled(" EXAMPLE ", badge(VIOLET)),
        Span::styled(format!("  {name}, on a sample board, not your game"), muted),
    ]);
    let bottom = Line::from(vec![
        Span::styled(
            name,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "  ·  step {} of {EXAMPLE_STAGES}   ← → steps through it",
                stage + 1
            ),
            muted,
        ),
    ]);
    (top, bottom)
}

/// What the step does, by cell name.
fn effect_lines(lines: &mut Vec<Line<'static>>, step: &Step) {
    let ink = Style::default().fg(INK);
    match &step.effect {
        Effect::Place { cell, digit } => lines.push(Line::styled(
            format!("{} is {digit}", rc(*cell)),
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        )),
        Effect::Eliminate(removals) => {
            // The same one digit out of every cell reads as a list of
            // cells, whatever digits the pattern itself is built on.
            let first = removals.first().map_or(0, |&(_, mask)| mask);
            if first.is_power_of_two() && removals.iter().all(|&(_, mask)| mask == first) {
                lines.push(Line::styled(
                    format!("Not {}:", first.trailing_zeros()),
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ));
                let cells: Vec<String> = removals.iter().map(|&(cell, _)| rc(cell)).collect();
                para(lines, &cells.join(", "), ink);
            } else {
                for &(cell, mask) in removals {
                    let text = format!("{}: not {}", rc(cell), digit_list(mask));
                    para(lines, &text, ink);
                }
            }
        }
    }
}

/// What the focus board draws: the player's own board, narrowed to a
/// step's pattern.
pub(super) struct FocusView<'a> {
    pub values: &'a Board,
    pub given: &'a [bool; CELLS],
    pub cands: &'a [u16; CELLS],
    pub step: &'a Step,
    /// 0 houses and cells, 1 adds marks and links, 2 adds the effect.
    pub stage: u8,
    /// The heavy grid lines: `GRID_BOX` for your game, `VIOLET` for an example.
    pub frame: Color,
}

/// Where candidate `digit` sits in the cell whose interior starts at `(x, y)`.
fn slot(x: u16, y: u16, digit: u8) -> (u16, u16) {
    let k = u16::from(digit - 1);
    (x + 1 + 2 * (k % 3), y + k / 3)
}

/// One candidate to draw.
struct Chip {
    x: u16,
    y: u16,
    ch: char,
    style: Style,
}

/// The board with a step's pattern drawn on it: the houses it looks
/// along, the cells it is built on, candidate chips, the links between
/// them and what the step removes or places. Everything else is quiet.
pub(super) fn draw_focus_board(
    frame: &mut Frame,
    ox: u16,
    oy: u16,
    labels_y: u16,
    view: &FocusView,
) {
    let buf = frame.buffer_mut();
    let pattern = &view.step.pattern;
    let effect = &view.step.effect;
    let marks = if view.stage >= 1 {
        &pattern.marks[..]
    } else {
        &[]
    };
    let links = if view.stage >= 1 {
        &pattern.links[..]
    } else {
        &[]
    };

    let mut band = [false; CELLS];
    for &h in &pattern.houses {
        for &c in &HOUSES[h] {
            band[usize::from(c)] = true;
        }
    }
    let mut pat = [false; CELLS];
    for &c in &pattern.cells {
        pat[c] = true;
    }
    for m in &pattern.marks {
        pat[m.cell] = true;
    }
    let mut touched = [false; CELLS];
    let mut removed = [0u16; CELLS];
    match effect {
        Effect::Place { cell, .. } => touched[*cell] = true,
        Effect::Eliminate(removals) => {
            for &(cell, mask) in removals {
                touched[cell] = true;
                removed[cell] |= mask;
            }
        }
    }
    let struck = view.stage >= 2;

    let label = Style::default().fg(MUTED);
    let mut chips: Vec<Chip> = Vec::new();
    // Placed digits, fullwidth: two columns each.
    let mut digits: Vec<Chip> = Vec::new();
    for r in 0..9 {
        let (_, y) = cell_screen(ox, oy, r, 0);
        buf[(ox - 1, y + CELL_H / 2)]
            .set_char(char::from(b'1' + r as u8))
            .set_style(label);
        for c in 0..9 {
            let i = r * 9 + c;
            let (x, y) = cell_screen(ox, oy, r, c);
            let bg = if struck && removed[i] != 0 {
                ERR_BG
            } else if pat[i] {
                PAT_BG
            } else if band[i] {
                BAND_BG
            } else {
                SURFACE
            };
            buf.set_style(Rect::new(x, y, CELL_W, CELL_H), Style::default().bg(bg));
            let v = view.values[i];
            if v != 0 {
                let on = pattern.digit == Some(v);
                digits.push(Chip {
                    x: x + CELL_W / 2,
                    y: y + CELL_H / 2,
                    ch: char::from_u32(0xFF10 + u32::from(v)).unwrap(),
                    style: Style::default()
                        .fg(if on { INK } else { DIM })
                        .bg(bg)
                        .add_modifier(if on {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                });
                continue;
            }
            let shown = pat[i] || touched[i];
            for d in 1..=9u8 {
                let bit = 1u16 << d;
                let wanted = match pattern.digit {
                    Some(only) => d == only,
                    None => shown,
                };
                if wanted && view.cands[i] & bit != 0 {
                    let (sx, sy) = slot(x, y, d);
                    chips.push(Chip {
                        x: sx,
                        y: sy,
                        ch: char::from(b'0' + d),
                        style: Style::default().fg(NOTE).bg(bg),
                    });
                }
            }
            let bold = Modifier::BOLD;
            for m in marks.iter().filter(|m| m.cell == i) {
                let fill = match m.role {
                    Role::Key => ACCENT,
                    Role::Other => VIOLET,
                };
                let (sx, sy) = slot(x, y, m.digit);
                chips.push(Chip {
                    x: sx,
                    y: sy,
                    ch: char::from(b'0' + m.digit),
                    style: Style::default().fg(BG).bg(fill).add_modifier(bold),
                });
            }
            if struck {
                for d in 1..=9u8 {
                    if removed[i] & (1 << d) != 0 {
                        let (sx, sy) = slot(x, y, d);
                        chips.push(Chip {
                            x: sx,
                            y: sy,
                            ch: char::from(b'0' + d),
                            style: Style::default()
                                .fg(RED)
                                .bg(ERR_BG)
                                .add_modifier(bold | Modifier::CROSSED_OUT),
                        });
                    }
                }
                if let Effect::Place { cell, digit } = effect
                    && *cell == i
                {
                    let (sx, sy) = slot(x, y, *digit);
                    chips.push(Chip {
                        x: sx,
                        y: sy,
                        ch: char::from(b'0' + digit),
                        style: Style::default().fg(BG).bg(GREEN).add_modifier(bold),
                    });
                }
            }
        }
    }
    // `given` only matters to the normal board; the pattern treats every
    // placed digit alike.
    let _ = view.given;

    // Grid lines, on the board's own surface.
    for gy in 0..BOARD_H {
        for gx in 0..BOARD_W {
            if let Some((glyph, heavy)) = grid_glyph(gx, gy) {
                buf[(ox + gx, oy + gy)]
                    .set_char(glyph)
                    .set_fg(if heavy { view.frame } else { GRID_THIN })
                    .set_bg(SURFACE);
            }
        }
    }

    let put = |buf: &mut ratatui::buffer::Buffer, chips: &[Chip]| {
        for chip in chips {
            buf[(chip.x, chip.y)]
                .set_char(chip.ch)
                .set_style(chip.style);
        }
    };
    put(buf, &chips);

    // Links run slot to slot, over whatever lies between, then the
    // candidates are drawn again on top of the lines.
    for link in links {
        let at = |(cell, d): (usize, u8)| {
            let (x, y) = cell_screen(ox, oy, cell / 9, cell % 9);
            slot(x, y, d)
        };
        let ((fx, fy), (tx, ty)) = (at(link.from), at(link.to));
        let (color, horizontal, vertical) = match link.kind {
            LinkKind::Strong => (ACCENT, '━', '┃'),
            LinkKind::Weak => (VIOLET, '╍', '╏'),
        };
        let mut line = |x: u16, y: u16, ch: char| {
            buf[(x, y)].set_char(ch).set_fg(color);
        };
        for x in fx.min(tx) + 1..fx.max(tx) {
            line(x, fy, horizontal);
        }
        if fx != tx && fy != ty {
            let corner = match (tx > fx, ty > fy) {
                (true, true) => '┐',
                (true, false) => '┘',
                (false, true) => '┌',
                (false, false) => '└',
            };
            line(tx, fy, corner);
        }
        for y in fy.min(ty) + 1..fy.max(ty) {
            line(tx, y, vertical);
        }
    }
    put(buf, &chips);
    // Placed digits go on last and whole, so a line through one looks
    // interrupted by it rather than splitting its two columns.
    for d in &digits {
        buf.set_string(d.x, d.y, d.ch.to_string(), d.style);
    }

    for c in 0..9 {
        let (x, _) = cell_screen(ox, oy, 0, c);
        buf[(x + CELL_W / 2, labels_y)]
            .set_char(char::from(b'1' + c as u8))
            .set_style(label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutor_text_wraps_at_21_columns() {
        assert_eq!(TEXT_W, 21);
        let text = "Going by placed digits — your notes aren't filled in yet.";
        let lines = wrap(text, TEXT_W);
        assert!(lines.iter().all(|l| l.chars().count() <= 21), "{lines:?}");
        assert_eq!(lines.join(" "), text, "breaks only at spaces");
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a bcdefg", 4), ["a", "bcde", "fg"]);
        assert!(wrap("  ", 21).is_empty());
    }

    use crate::app::{App, BoardAction};
    use crate::deal::Difficulty;
    use crate::game::Game;
    use crate::logic::{Grid, Link, Mark, Pattern};
    use crate::store::Store;
    use crate::sudoku::{dig, full_solution, idx, parse_board, solve_one, test_board};
    use crate::tutor::examples::example;
    use crate::ui::tests::{W, draw, text};
    use crate::ui::{MIN_H, MIN_W, PANEL_GAP, board_origin, hit_at};
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    const XY_WING: &str =
        "800000046007000020920004005000010800035740000080090004000000060600500000200006013";

    fn app_showing(game: Game, step: Step, mode: NoteMode) -> App {
        let mut app = App::new(Store::none());
        app.start_game(game);
        app.tutor = Some(Tutor {
            mode,
            advice: Advice::Step(step),
            stage: TutorStage::ShowMe,
        });
        app
    }

    /// An XY-Wing step on a game whose notes are the solver's candidates.
    fn xy_wing_app() -> (App, Step) {
        let puzzle = parse_board(XY_WING).unwrap();
        let mut grid = Grid::new(&puzzle);
        let step = loop {
            if let Some(step) = grid.find(Technique::XYWing) {
                break step;
            }
            let next = grid.next_step().expect("the puzzle has an XY-Wing");
            grid.apply(&next);
        };
        let mut game = Game::new(Difficulty::Hard, puzzle, puzzle);
        game.values = *grid.values();
        game.notes = *grid.cands();
        (app_showing(game, step.clone(), NoteMode::Full), step)
    }

    fn screen(app: &App, w: u16, h: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| _ = crate::ui::render(f, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    /// Where candidate `digit` of `cell` sits on a `W` x 50 screen.
    fn slot_at(cell: usize, digit: u8) -> (u16, u16) {
        let (ox, oy) = board_origin(Rect::new(0, 0, W, 50));
        let (x, y) = cell_screen(ox, oy, cell / 9, cell % 9);
        slot(x, y, digit)
    }

    #[test]
    fn show_me_narrows_a_single_digit_pattern() {
        let (puzzle, solution) = test_board();
        let game = Game::new(Difficulty::Easy, puzzle, solution);
        let step = Grid::new(&puzzle)
            .find(Technique::HiddenSingle)
            .expect("the board has a hidden single");
        let Effect::Place { cell, digit } = step.effect else {
            panic!("expected a placement")
        };
        assert_eq!(step.pattern.digit, Some(digit));
        let app = app_showing(game, step, NoteMode::PairMarks);
        let buf = screen(&app, W, 50);
        let (ox, oy) = board_origin(Rect::new(0, 0, W, 50));
        let mut seen = 0;
        for r in 0..9 {
            for c in 0..9 {
                let (x, y) = cell_screen(ox, oy, r, c);
                for dy in 0..CELL_H {
                    for dx in 0..CELL_W {
                        let ch = buf[(x + dx, y + dy)].symbol().chars().next().unwrap();
                        if ch.is_ascii_digit() {
                            assert_eq!(ch, char::from(b'0' + digit), "r{}c{}", r + 1, c + 1);
                            seen += 1;
                        }
                    }
                }
            }
        }
        assert!(seen > 1, "the digit's candidates stay");
        let (sx, sy) = slot_at(cell, digit);
        assert_eq!(buf[(sx, sy)].bg, GREEN);
    }

    #[test]
    fn show_me_draws_links_between_pattern_candidates() {
        let (app, step) = xy_wing_app();
        let buf = screen(&app, W, 50);
        let mut found = false;
        for link in &step.pattern.links {
            let (a, b) = (
                slot_at(link.from.0, link.from.1),
                slot_at(link.to.0, link.to.1),
            );
            for y in a.1.min(b.1)..=a.1.max(b.1) {
                for x in a.0.min(b.0)..=a.0.max(b.0) {
                    let cell = &buf[(x, y)];
                    if (x, y) != a && (x, y) != b && "╍╏".contains(cell.symbol()) {
                        assert_eq!(cell.fg, VIOLET);
                        found = true;
                    }
                }
            }
        }
        assert!(found, "no weak link line between the pattern's candidates");
        let Effect::Eliminate(_) = &step.effect else {
            panic!("an XY-Wing eliminates")
        };
        for m in step.pattern.marks.iter().filter(|m| m.role == Role::Key) {
            let (x, y) = slot_at(m.cell, m.digit);
            assert_eq!(buf[(x, y)].bg, ACCENT);
        }
    }

    #[test]
    fn removed_candidates_are_red_and_struck() {
        let (app, step) = xy_wing_app();
        let buf = screen(&app, W, 50);
        let Effect::Eliminate(removals) = &step.effect else {
            panic!("an XY-Wing eliminates")
        };
        let &(cell, mask) = removals.first().unwrap();
        let (x, y) = slot_at(cell, mask.trailing_zeros() as u8);
        let drawn = &buf[(x, y)];
        assert_eq!(drawn.fg, RED);
        assert_eq!(drawn.bg, ERR_BG);
        assert!(drawn.modifier.contains(Modifier::CROSSED_OUT));
    }

    fn selects(hits: &[Hit]) -> usize {
        hits.iter()
            .filter(|h| matches!(h.action, Some(Action::Board(BoardAction::Select(..)))))
            .count()
    }

    #[test]
    fn show_me_fits_the_minimum_terminal() {
        let (app, _) = xy_wing_app();
        let (lines, hits) = draw(&app, MIN_W, MIN_H);
        assert!(text(&lines).contains("back to solving"));
        assert_eq!(selects(&hits), 81, "every cell stays clickable");
    }

    /// The rows of text in the side panel, `w` columns wide.
    fn panel_rows(lines: &[String], w: u16) -> Vec<String> {
        let (ox, _) = board_origin(Rect::new(0, 0, w, 50));
        let x = usize::from(ox + BOARD_W + PANEL_GAP + 2);
        lines
            .iter()
            .map(|l| l.chars().skip(x).take(TEXT_W).collect::<String>())
            .map(|l| l.trim().to_string())
            .collect()
    }

    /// The lines saying what `step` removes or places, as text.
    fn effect_text(step: &Step) -> Vec<String> {
        let mut lines = Vec::new();
        effect_lines(&mut lines, step);
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    /// Draw `step` in show me at the minimum size, and check the pane
    /// says in full what it removes or places, and still offers its keys.
    /// Returns the pane's rows.
    fn shows_the_effect(game: Game, step: &Step, mode: NoteMode) -> Vec<String> {
        let app = app_showing(game, step.clone(), mode);
        let rows = panel_rows(&draw(&app, MIN_W, MIN_H).0, MIN_W);
        let effect = effect_text(step);
        let t = step.technique;
        assert!(
            rows.windows(effect.len()).any(|w| w == effect.as_slice()),
            "{t:?} in {mode:?}: {effect:?} not all shown in {rows:#?}"
        );
        assert!(rows.iter().any(|r| r == "Esc  back to solving"), "{t:?}");
        rows
    }

    #[test]
    fn show_me_always_says_what_the_step_does() {
        let game = |values: Board, solution: Board, notes: [u16; CELLS]| {
            let mut game = Game::new(Difficulty::Zen, values, solution);
            game.notes = notes;
            game
        };
        // The worked examples hold the longest reasons, and fit whole when
        // your notes are filled in.
        for &t in Technique::ALL {
            let ex = example(t).unwrap();
            let solution = solve_one(&ex.values).unwrap();
            let rows = shows_the_effect(
                game(ex.values, solution, ex.cands),
                &ex.step,
                NoteMode::Full,
            );
            assert!(!rows.iter().any(|r| r.contains('…')), "{t:?}: {rows:#?}");
            shows_the_effect(
                game(ex.values, solution, [0; CELLS]),
                &ex.step,
                NoteMode::PairMarks,
            );
        }
        // Dealt puzzles, solved with notes and without.
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..8 {
            let solution = full_solution(&mut rng);
            let puzzle = dig(&solution, CELLS, &mut rng);
            let mut grid = Grid::new(&puzzle);
            while let Some(step) = grid.next_step() {
                let values = *grid.values();
                if step.technique > Technique::HiddenSingle {
                    shows_the_effect(game(values, solution, *grid.cands()), &step, NoteMode::Full);
                }
                if let Some(fresh) = Grid::new(&values).next_step()
                    && fresh.technique > Technique::HiddenSingle
                {
                    shows_the_effect(
                        game(values, solution, [0; CELLS]),
                        &fresh,
                        NoteMode::PairMarks,
                    );
                }
                grid.apply(&step);
            }
        }
    }

    #[test]
    fn titles_name_the_strategy_with_a_capital() {
        assert_eq!(heading(Technique::NakedSingle), "Naked single");
        assert_eq!(heading(Technique::Pointing), "Pointing");
        assert_eq!(heading(Technique::XWing), "X-Wing");
        let (app, _) = xy_wing_app();
        assert!(text(&draw(&app, W, 50).0).contains("╭ XY-Wing ─"));
    }

    #[test]
    fn links_never_split_a_placed_digit() {
        // Row 1 reads 5 3 . . 7 . . . . : a link between the 4s of r1c3
        // and r1c6 runs through the 7 in r1c5.
        let (puzzle, solution) = test_board();
        let game = || Game::new(Difficulty::Easy, puzzle, solution);
        let ends = [(idx(0, 2), 4), (idx(0, 5), 4)];
        let step = Step {
            technique: Technique::XYWing,
            effect: Effect::Eliminate(vec![(idx(0, 8), 1 << 4)]),
            pattern: Pattern {
                cells: vec![idx(0, 2), idx(0, 5)],
                marks: ends
                    .iter()
                    .map(|&(cell, digit)| Mark {
                        cell,
                        digit,
                        role: Role::Other,
                    })
                    .collect(),
                links: vec![Link {
                    from: ends[0],
                    to: ends[1],
                    kind: LinkKind::Weak,
                }],
                ..Pattern::default()
            },
            why: "Because.".to_string(),
        };
        let mut plain = App::new(Store::none());
        plain.start_game(game());
        let plain = screen(&plain, W, 50);
        let shown = screen(&app_showing(game(), step, NoteMode::PairMarks), W, 50);

        let (ox, oy) = board_origin(Rect::new(0, 0, W, 50));
        let (x, y) = cell_screen(ox, oy, 0, 4);
        let (x, y) = (x + CELL_W / 2, y + CELL_H / 2);
        assert_eq!(shown[(x, y)].symbol(), "７");
        assert_eq!(shown[(x + 1, y)].symbol(), plain[(x + 1, y)].symbol());
        assert_eq!(
            shown[(x + 1, y)],
            plain[(x + 1, y)],
            "the digit's other half"
        );
        for x in [x - 1, x + 2] {
            assert_eq!(shown[(x, y)].symbol(), "╍", "the line runs on either side");
        }
    }

    /// Your easy game, with the tutor explaining `t` at `stage`.
    fn app_explaining(t: Technique, stage: u8) -> App {
        let (puzzle, solution) = test_board();
        let mut app = App::new(Store::none());
        app.start_game(Game::new(Difficulty::Easy, puzzle, solution));
        let step = example(t)
            .unwrap_or_else(|| panic!("{t:?} has no example"))
            .step;
        app.tutor = Some(Tutor {
            mode: NoteMode::Full,
            advice: Advice::Step(step),
            stage: TutorStage::Explain { stage },
        });
        app
    }

    /// The text inside the side panel, rows joined, for a screen `w` wide.
    fn panel_text(lines: &[String], w: u16) -> String {
        let (ox, _) = board_origin(Rect::new(0, 0, w, 50));
        let x = usize::from(ox + BOARD_W + PANEL_GAP + 2);
        lines
            .iter()
            .map(|l| {
                l.chars()
                    .skip(x)
                    .take(TEXT_W)
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn explain_shows_the_example_not_your_game() {
        let t = Technique::Pointing;
        let app = app_explaining(t, 2);
        let ex = example(t).unwrap();
        let buf = screen(&app, W, 50);
        let (lines, hits) = draw(&app, W, 50);
        let (ox, oy) = board_origin(Rect::new(0, 0, W, 50));

        let header = text(&lines[..usize::from(oy)]);
        assert!(
            header.contains(" EXAMPLE   Pointing, on a sample board, not your game"),
            "{header}"
        );
        assert!(
            header.contains("Pointing  ·  step 3 of 3   ← → steps through it"),
            "{header}"
        );
        let badge = lines.iter().position(|l| l.contains("EXAMPLE")).unwrap();
        let bx = lines[badge].chars().position(|c| c == 'E').unwrap() as u16;
        assert_eq!(buf[(bx, badge as u16)].bg, VIOLET);
        assert_eq!(buf[(bx, badge as u16)].fg, BG);

        assert_eq!(buf[(ox, oy)].symbol(), "┏");
        assert_eq!(buf[(ox, oy)].fg, VIOLET, "a violet frame");
        for i in 0..CELLS {
            let (x, y) = cell_screen(ox, oy, i / 9, i % 9);
            let centre = buf[(x + CELL_W / 2, y + CELL_H / 2)].symbol();
            let v = ex.values[i];
            if v != 0 {
                let digit = char::from_u32(0xFF10 + u32::from(v)).unwrap();
                assert_eq!(centre, digit.to_string(), "{}", rc(i));
            } else {
                assert!(!centre.starts_with(|c| ('０'..='９').contains(&c)));
            }
        }
        assert_eq!(selects(&hits), 0, "the example can't be played");

        let pane = panel_text(&lines, W);
        assert!(text(&lines).contains("╭ Pointing ─"));
        for (k, step) in lesson(t).steps.iter().enumerate() {
            assert!(pane.contains(&format!("{} {step}", k + 1)), "{pane}");
        }
        assert!(pane.contains(&format!(" {}", lesson(t).roles.0)), "{pane}");
        assert!(pane.contains("removed"), "{pane}");
        assert!(!pane.contains("Going by placed digits"), "{pane}");
        for want in [Action::Tutor, Action::CloseTutor] {
            assert!(hits.iter().any(|h| h.action == Some(want)), "{want:?}");
        }
        // The arrows' line: a click on the left arrow steps back, and
        // anywhere else on it steps on.
        let y = lines.iter().position(|l| l.contains("← →  step")).unwrap();
        let x = lines[y].chars().position(|c| c == '←').unwrap();
        let click = |x: usize| hit_at(&hits, x as u16, y as u16);
        assert_eq!(click(x), Some(Action::ExampleStep(-1)));
        assert_eq!(click(x + 2), Some(Action::ExampleStep(1)));
        assert_eq!(click(x + 6), Some(Action::ExampleStep(1)));
        assert!(
            !hits
                .iter()
                .any(|h| matches!(h.action, Some(Action::Board(_))))
        );
        let footer = &lines[usize::from(oy + BOARD_H)];
        assert!(footer.contains("show me in my game"), "{footer}");
        assert!(!footer.contains("fill"), "{footer}");
    }

    #[test]
    fn explain_steps_through_the_example() {
        let t = Technique::XYWing;
        let ex = example(t).unwrap();
        let Effect::Eliminate(removals) = &ex.step.effect else {
            panic!("an XY-Wing eliminates")
        };
        let &(cell, mask) = removals.first().unwrap();
        let key = ex.step.pattern.marks.iter().find(|m| m.role == Role::Key);
        let key = key.unwrap();
        let (ox, oy) = board_origin(Rect::new(0, 0, W, 50));
        let at = |cell: usize, digit: u8| {
            let (x, y) = cell_screen(ox, oy, cell / 9, cell % 9);
            slot(x, y, digit)
        };
        let (kx, ky) = at(key.cell, key.digit);
        let (rx, ry) = at(cell, mask.trailing_zeros() as u8);
        let stage = |n| screen(&app_explaining(t, n), W, 50);
        let first = stage(0);
        assert_ne!(first[(kx, ky)].bg, ACCENT, "no chips yet");
        assert_ne!(first[(rx, ry)].fg, RED);
        let second = stage(1);
        assert_eq!(second[(kx, ky)].bg, ACCENT, "now the chips");
        assert_ne!(second[(rx, ry)].fg, RED);
        let third = stage(2);
        assert_eq!(third[(rx, ry)].fg, RED, "and last what goes");
        let (lines, _) = draw(&app_explaining(t, 0), W, 50);
        assert!(text(&lines).contains("step 1 of 3"));
    }

    #[test]
    fn explain_fits_the_minimum_terminal() {
        for &t in Technique::ALL {
            let (lines, hits) = draw(&app_explaining(t, 2), MIN_W, MIN_H);
            let screen = text(&lines);
            assert!(screen.contains("EXAMPLE"), "{t:?}");
            let pane = panel_text(&lines, MIN_W);
            for (k, step) in lesson(t).steps.iter().enumerate() {
                assert!(pane.contains(&format!("{} {step}", k + 1)), "{t:?}: {pane}");
            }
            for want in ["← →  step", "show me in my game", "Esc  back to solving"] {
                assert!(pane.contains(want), "{t:?}: {pane}");
            }
            assert!(!pane.contains('…'), "{t:?}: {pane}");
            assert_eq!(selects(&hits), 0, "{t:?}");
        }
    }
}
