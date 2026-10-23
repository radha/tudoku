//! The tutor's pane, in the side panel under the number pad: first the
//! name of a strategy to look for, then the step itself in r#c# terms,
//! with the keys that go on from there.

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
use crate::app::{Action, Tutor, TutorStage};
use crate::logic::{Effect, LinkKind, Role, Step, digit_list, rc};
use crate::sudoku::{Board, CELLS, HOUSES};
use crate::tutor::{Advice, NoteMode};

/// Columns of text: the panel's width, less the border and a column of
/// padding on each side.
const TEXT_W: usize = PANEL_W as usize - 4;

/// `text` broken at spaces into lines at most `width` columns wide. A
/// word too long for a line of its own is split.
pub(super) fn wrap(text: &str, width: usize) -> Vec<String> {
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

/// `text`, wrapped to the pane, in `style`.
fn para(lines: &mut Vec<Line<'static>>, text: &str, style: Style) {
    lines.extend(
        wrap(text, TEXT_W)
            .into_iter()
            .map(|l| Line::styled(l, style)),
    );
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

/// Draw the pane into `area`. Each key line is also a click target.
pub(super) fn draw_pane(frame: &mut Frame, area: Rect, tutor: &Tutor, hits: &mut Vec<Hit>) {
    let title = match (&tutor.advice, tutor.stage) {
        (Advice::Step(step), TutorStage::ShowMe) => step.technique.name(),
        _ => "Tutor",
    };
    let block = popup_block(title);
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
        (Advice::Step(step), TutorStage::ShowMe) => {
            para(&mut lines, &step.why, ink);
            lines.push(Line::from(""));
            effect_lines(&mut lines, step);
            let apply = Key {
                key: "a",
                what: "do it for me",
                action: Action::TutorApply,
            };
            if tutor.can_apply() {
                vec![apply, BACK]
            } else {
                vec![BACK]
            }
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

    // The keys stay in view even if the text runs long.
    let room = usize::from(inner.height).saturating_sub(keys.len());
    if lines.len() > room {
        lines.truncate(room);
        if let Some(last) = lines.last_mut() {
            *last = Line::styled("…", muted);
        }
    }
    for key in keys {
        let y = inner.y + lines.len() as u16;
        push_hit(
            hits,
            Rect::new(area.x + 1, y, area.width.saturating_sub(2), 1),
            Some(key.action),
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<5}", key.key),
                Style::default().fg(INK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(key.what, muted),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
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
                let digit = char::from_u32(0xFF10 + u32::from(v)).unwrap();
                buf.set_string(
                    x + CELL_W / 2,
                    y + CELL_H / 2,
                    digit.to_string(),
                    Style::default()
                        .fg(if on { INK } else { DIM })
                        .bg(bg)
                        .add_modifier(if on {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                );
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
    use crate::logic::{Grid, Technique};
    use crate::store::Store;
    use crate::sudoku::{parse_board, test_board};
    use crate::ui::tests::{W, draw, text};
    use crate::ui::{MIN_H, MIN_W, board_origin};
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

    #[test]
    fn show_me_fits_the_minimum_terminal() {
        let (app, _) = xy_wing_app();
        let (lines, hits) = draw(&app, MIN_W, MIN_H);
        assert!(text(&lines).contains("back to solving"));
        let selects = hits
            .iter()
            .filter(|h| matches!(h.action, Some(Action::Board(BoardAction::Select(..)))))
            .count();
        assert_eq!(selects, 81, "every cell stays clickable");
    }
}
