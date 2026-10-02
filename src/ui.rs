//! Rendering: board, number pad, buttons, popups. Pure view code —
//! `render` draws the app and returns the clickable areas for the event
//! loop to hit-test mouse clicks against.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Margin, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::app::{Action, App, BoardAction, Overlay};
use crate::deal::Difficulty;
use crate::game::Game;
use crate::sudoku::{box_of, idx};

// ---------------------------------------------------------------- palette

const BG: Color = Color::Reset;
const INK: Color = Color::Rgb(226, 232, 240);
const DIM: Color = Color::Rgb(100, 116, 139);
const ACCENT: Color = Color::Rgb(34, 211, 238);
const AMBER: Color = Color::Rgb(251, 191, 36);
const GREEN: Color = Color::Rgb(52, 211, 153);
const RED: Color = Color::Rgb(248, 113, 113);
const GRID_THIN: Color = Color::Rgb(71, 85, 105);
const GRID_BOX: Color = Color::Rgb(34, 211, 238);
const PEER_BG: Color = Color::Rgb(30, 41, 59);
const SAME_BG: Color = Color::Rgb(51, 65, 85);
const SEL_BG: Color = Color::Rgb(37, 99, 235);
const POPUP_BG: Color = Color::Rgb(15, 23, 42);

// ------------------------------------------------------------ sizing

// Terminal glyph cells run ~2x taller than wide, so a 6x3 terminal-cell
// interior renders near-square on most fonts. 6 wide is also the minimum
// that still fits the 3x3 pencil-mark mini-grid ("d d d").
pub const CELL_W: u16 = 6;
pub const CELL_H: u16 = 3;
pub const BOARD_W: u16 = 9 * CELL_W + 10; // 64
pub const BOARD_H: u16 = 9 * CELL_H + 4; // 31 (horizontal guides only around boxes)

// The number pad and action buttons sit beside the board rather than below
// it, since terminals are almost always much wider than they are tall — that
// keeps the required height pinned to the board's own height instead of
// stacking on top of it.
const DIGIT_W: u16 = 6;
const DIGIT_GAP: u16 = 1;
const DIGIT_GRID_W: u16 = 3 * DIGIT_W + 2 * DIGIT_GAP; // 20
const DIGIT_GRID_H: u16 = 3 * CELL_H; // 9
const BTN_W: u16 = 12;
const BTN_GAP: u16 = 1;
const BTN_GRID_W: u16 = 2 * BTN_W + BTN_GAP; // 25
const BTN_GRID_H: u16 = 4 * CELL_H; // 12
const PANEL_GROUP_GAP: u16 = 2;
const PANEL_W: u16 = if DIGIT_GRID_W > BTN_GRID_W {
    DIGIT_GRID_W
} else {
    BTN_GRID_W
};
const PANEL_H: u16 = DIGIT_GRID_H + PANEL_GROUP_GAP + BTN_GRID_H; // 23
const PANEL_GAP: u16 = 3;
const BLOCK_W: u16 = BOARD_W + PANEL_GAP + PANEL_W; // 92

pub const MIN_W: u16 = BLOCK_W + 2;
pub const MIN_H: u16 = 3 + BOARD_H; // header (2 lines) + divider + board

// ------------------------------------------------------------ clicks

/// A clickable area. `action: None` swallows the click — popups use that
/// to keep clicks from reaching whatever is drawn underneath them.
#[derive(Debug, Clone, Copy)]
pub struct Hit {
    pub rect: Rect,
    pub action: Option<Action>,
}

fn push_hit(hits: &mut Vec<Hit>, rect: Rect, action: Option<Action>) {
    hits.push(Hit { rect, action });
}

/// The action under `(x, y)`, from the topmost (last drawn) hit area.
pub fn hit_at(hits: &[Hit], x: u16, y: u16) -> Option<Action> {
    hits.iter()
        .rev()
        .find(|h| h.rect.contains(Position { x, y }))
        .and_then(|h| h.action)
}

// ------------------------------------------------------------ helpers

pub fn format_time(d: std::time::Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

fn centered_rect(area: Rect, w: u16, h: u16) -> Rect {
    area.centered(Constraint::Length(w), Constraint::Length(h))
}

fn cell_screen(ox: u16, oy: u16, row: usize, col: usize) -> (u16, u16) {
    let x = ox + 1 + col as u16 * (CELL_W + 1);
    let y = oy + 1 + row as u16 * CELL_H + u16::from(row >= 3) + u16::from(row >= 6);
    (x, y)
}

fn is_hline(gy: u16) -> bool {
    // One horizontal guide per 3-row box band: top, between bands, bottom.
    gy.is_multiple_of(3 * CELL_H + 1)
}

// ------------------------------------------------------------ board

fn draw_board(frame: &mut Frame, game: &Game, ox: u16, oy: u16, hits: &mut Vec<Hit>) {
    let buf = frame.buffer_mut();
    let sel = game.selected_idx();
    let sel_val = game.values[sel];
    let (sel_r, sel_c) = game.selected;

    // Column numbers above the board.
    for c in 0..9 {
        let (x, _) = cell_screen(ox, oy, 0, c);
        buf[(x + CELL_W / 2, oy - 1)]
            .set_char(char::from(b'1' + c as u8))
            .set_fg(DIM);
    }

    // Cell interiors.
    for r in 0..9 {
        // Row number to the left, vertically centered on the cell.
        let (_, y) = cell_screen(ox, oy, r, 0);
        buf[(ox - 1, y + CELL_H / 2)]
            .set_char(char::from(b'1' + r as u8))
            .set_fg(DIM);
        for c in 0..9 {
            let i = idx(r, c);
            let (x, y) = cell_screen(ox, oy, r, c);
            let is_sel = r == sel_r && c == sel_c;
            let peer =
                !is_sel && (r == sel_r || c == sel_c || box_of(r, c) == box_of(sel_r, sel_c));
            let same_val = !is_sel && sel_val != 0 && game.values[i] == sel_val;
            let bg = if game.paused {
                BG
            } else if is_sel {
                SEL_BG
            } else if same_val {
                SAME_BG
            } else if peer {
                PEER_BG
            } else {
                BG
            };
            let cell_rect = Rect::new(x, y, CELL_W, CELL_H);
            buf.set_style(cell_rect, Style::default().bg(bg));
            push_hit(
                hits,
                cell_rect,
                Some(Action::Board(BoardAction::Select(r, c))),
            );
            if game.paused {
                continue;
            }
            let ym = y + CELL_H / 2;
            let v = game.values[i];
            if v != 0 {
                let err = game.cell_error(i);
                let fg = if err {
                    RED
                } else if game.given[i] {
                    INK
                } else if game.hinted[i] {
                    GREEN
                } else {
                    AMBER
                };
                let mut style = Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD);
                if is_sel {
                    style = style.fg(Color::White);
                }
                buf[(x + CELL_W / 2, ym)]
                    .set_char(char::from(b'0' + v))
                    .set_style(style);
                // Conflict marker: duplicates show even without color.
                if err {
                    buf[(x, ym)]
                        .set_char('!')
                        .set_style(Style::default().fg(RED).bg(bg).add_modifier(Modifier::BOLD));
                }
            } else {
                // Pencil marks as a 3x3 mini-grid filling the cell.
                let note_style = Style::default().fg(DIM).bg(bg).add_modifier(Modifier::DIM);
                for d in 1..=9u8 {
                    if game.notes[i] & (1 << d) != 0 {
                        let k = u16::from(d - 1);
                        buf[(x + 2 * (k % 3), y + k / 3)]
                            .set_char(char::from(b'0' + d))
                            .set_style(note_style);
                    }
                }
                if game.notes[i] == 0 && is_sel {
                    buf[(x + CELL_W / 2, ym)]
                        .set_char('·')
                        .set_style(Style::default().fg(Color::White).bg(bg));
                }
            }
        }
    }

    // Grid lines.
    for gy in 0..BOARD_H {
        let y = oy + gy;
        let hline = is_hline(gy);
        for gx in 0..BOARD_W {
            let x = ox + gx;
            let vline = gx % (CELL_W + 1) == 0;
            if !hline && !vline {
                continue;
            }
            let on_box_v = [0, 3 * (CELL_W + 1), 6 * (CELL_W + 1), 9 * (CELL_W + 1)].contains(&gx);
            let cell = &mut buf[(x, y)];
            if hline && vline {
                cell.set_char('┼');
                cell.set_fg(if on_box_v { GRID_BOX } else { GRID_THIN });
            } else if hline {
                cell.set_char('─');
                cell.set_fg(GRID_BOX);
            } else {
                cell.set_char('│');
                cell.set_fg(if on_box_v { GRID_BOX } else { GRID_THIN });
            }
            if hline {
                // Thicken box guides with bold. Every hline is a box guide.
                cell.set_style(Style::default().fg(GRID_BOX).add_modifier(Modifier::BOLD));
                if vline && !on_box_v {
                    cell.set_char('┿');
                } else if vline {
                    cell.set_char('╂');
                } else {
                    cell.set_char('═');
                }
            }
            // Outer corners.
            if (gx == 0 || gx == BOARD_W - 1) && (gy == 0 || gy == BOARD_H - 1) {
                let ch = match (gx == 0, gy == 0) {
                    (true, true) => '╔',
                    (false, true) => '╗',
                    (true, false) => '╚',
                    (false, false) => '╝',
                };
                cell.set_char(ch);
            }
        }
    }
}

// ------------------------------------------------------------ widgets

struct Button<'a> {
    title: &'a str,
    hint: &'a str,
    active: bool,
    action: Action,
}

fn draw_button(frame: &mut Frame, rect: Rect, btn: &Button, hits: &mut Vec<Hit>) {
    let border = if btn.active {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(GRID_THIN)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    push_hit(hits, rect, Some(btn.action));
    let line = Line::from(vec![
        Span::styled(
            btn.title,
            Style::default()
                .fg(if btn.active { ACCENT } else { INK })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", btn.hint),
            Style::default().fg(DIM).add_modifier(Modifier::DIM),
        ),
    ]);
    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), inner);
}

fn header_lines(game: &Game) -> (Line<'static>, Line<'static>) {
    let diff = game.difficulty;
    let (done, total) = game.progress();
    let bar_w = 14;
    let filled = done * bar_w / total;
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_w - filled);
    let top = Line::from(vec![
        Span::styled(
            " TUDOKU ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!(" {} ", diff.name()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", diff.blurb()),
            Style::default().fg(DIM).add_modifier(Modifier::DIM),
        ),
    ]);
    if game.completed {
        let bottom = Line::from(vec![
            Span::styled(
                format!("✓ Solved in {}   ", format_time(game.elapsed())),
                Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("✖ {}   💡 {}   ", game.mistakes, game.hints_used),
                Style::default().fg(DIM),
            ),
            Span::styled("N new puzzle · d change level", Style::default().fg(INK)),
        ]);
        return (top, bottom);
    }
    let status_msg = game
        .message(std::time::Duration::from_secs(4))
        .unwrap_or("")
        .to_string();
    let bottom = Line::from(vec![
        Span::styled(
            format!("⏱ {}  ", format_time(game.elapsed())),
            Style::default().fg(INK),
        ),
        Span::styled(
            format!("✖ {}   ", game.mistakes),
            Style::default().fg(if game.mistakes > 0 { RED } else { DIM }),
        ),
        Span::styled(
            format!("💡 {}   ", game.hints_used),
            Style::default().fg(DIM),
        ),
        Span::styled(format!("{bar} {done}/{total}   "), Style::default().fg(DIM)),
        Span::styled(
            if game.notes_mode {
                "✎ NOTES ON"
            } else {
                "✎ notes off"
            },
            Style::default()
                .fg(if game.notes_mode { AMBER } else { DIM })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("   {status_msg}"), Style::default().fg(AMBER)),
    ]);
    (top, bottom)
}

fn popup_block(title: &str) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(POPUP_BG))
}

/// Clear a centered `w`x`h` box, frame it, and return `(frame, content)`
/// where content is the inside with one column of padding.
fn popup(frame: &mut Frame, area: Rect, w: u16, h: u16, title: &str) -> (Rect, Rect) {
    let r = centered_rect(area, w, h);
    frame.render_widget(Clear, r);
    let block = popup_block(title);
    let inner = block.inner(r).inner(Margin::new(1, 0));
    frame.render_widget(block, r);
    (r, inner)
}

fn draw_levels_menu(frame: &mut Frame, area: Rect, cursor: usize, hits: &mut Vec<Hit>) -> Rect {
    let (r, inner) = popup(frame, area, 50, 16, "New game — pick difficulty");
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " arrows/hjkl + Enter, keys 1-5, or click ",
            Style::default().fg(DIM).add_modifier(Modifier::DIM),
        )),
        Line::from(""),
    ];
    for (i, d) in Difficulty::ALL.iter().enumerate() {
        let sel = i == cursor;
        let marker = if sel { "▶" } else { " " };
        lines.push(Line::from(vec![Span::styled(
            format!(" {marker} {}. {:<6}  {} ", i + 1, d.name(), d.blurb()),
            Style::default()
                .fg(if sel { Color::White } else { INK })
                .bg(if sel { SEL_BG } else { POPUP_BG })
                .add_modifier(Modifier::BOLD),
        )]));
        let row = Rect::new(r.x + 1, inner.y + 2 + i as u16, r.width - 2, 1);
        push_hit(hits, row, Some(Action::StartLevel(*d)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Enter start   Esc cancel ",
        Style::default().fg(DIM),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
    r
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let (_, inner) = popup(frame, area, 56, 21, "How to play  (?)");
    let rows = [
        ("Move", "arrows / hjkl, or click a cell"),
        ("Fill", "1-9  (click a cell, then a number)"),
        ("Notes", "n toggles pencil marks, then 1-9"),
        ("Erase", "0 / x / e / Backspace / Delete"),
        ("Undo", "u  or Ctrl+Z"),
        ("Hint", "H (capital) — reveals & locks a cell"),
        ("New", "N (capital) — fresh puzzle, same level"),
        ("Level", "d — change difficulty"),
        ("Pause", "p — hides the board, stops timer"),
        ("Quit", "q    •    close popups: Esc"),
        ("Mouse", "click cells, the number pad, buttons"),
    ];
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Fill every row, column and 3x3 box with 1-9. ",
            Style::default().fg(INK),
        )),
        Line::from(""),
    ];
    for (k, v) in rows {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {k:<6}"),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {v}"), Style::default().fg(INK)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " press Esc or click anywhere to close ",
        Style::default().fg(DIM).add_modifier(Modifier::DIM),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_paused(frame: &mut Frame, area: Rect) {
    let (_, inner) = popup(frame, area, 41, 6, "Paused");
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled("board hidden, timer stopped", Style::default().fg(INK)),
            Line::styled(
                "press p or click anywhere to resume",
                Style::default().fg(DIM),
            ),
        ])
        .alignment(Alignment::Center),
        inner,
    );
}

/// Returns the popup's frame and its two buttons.
fn draw_won(frame: &mut Frame, area: Rect, game: &Game) -> (Rect, Rect, Rect) {
    let (r, inner) = popup(frame, area, 48, 13, "Solved!");
    let lines = vec![
        Line::styled(
            format!("{} puzzle cleared", game.difficulty.name()),
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::styled(
            format!(
                "Time {}   Mistakes {}   Hints {}",
                format_time(game.elapsed()),
                game.mistakes,
                game.hints_used
            ),
            Style::default().fg(INK),
        ),
        Line::from(""),
        Line::styled(
            "Enter new · d levels · Esc view board",
            Style::default().fg(DIM),
        ),
    ];
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
    // Clickable footer buttons.
    let bw = 12u16;
    let by = r.y + r.height - 4;
    let bx = r.x + (r.width.saturating_sub(bw * 2 + 2)) / 2;
    let new_btn = Rect::new(bx, by, bw, 3);
    let levels_btn = Rect::new(bx + bw + 2, by, bw, 3);
    let button = || {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
    };
    frame.render_widget(
        Paragraph::new(Line::styled(
            " ↻ New ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .block(button()),
        new_btn,
    );
    frame.render_widget(
        Paragraph::new(Line::styled(" ◈ Levels ", Style::default().fg(INK)))
            .alignment(Alignment::Center)
            .block(button()),
        levels_btn,
    );
    (r, new_btn, levels_btn)
}

fn draw_dealing(frame: &mut Frame, area: Rect, level: Difficulty) {
    const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let tick = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() / 80);
    let spin = SPINNER[(tick % SPINNER.len() as u128) as usize];
    let (_, inner) = popup(frame, area, 36, 6, "Dealing");
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled(
                format!("{spin} shuffling a fresh {} puzzle", level.name()),
                Style::default().fg(INK),
            ),
            Line::styled("graded by technique, logic only", Style::default().fg(DIM)),
        ])
        .alignment(Alignment::Center),
        inner,
    );
}

/// Draw one overlay and register its click areas. Every overlay first
/// covers the whole screen, so nothing underneath can be clicked.
fn draw_overlay(frame: &mut Frame, area: Rect, app: &App, overlay: Overlay, hits: &mut Vec<Hit>) {
    // Clicking outside a popup is the same as pressing Esc.
    push_hit(hits, area, Some(Action::Close));
    match overlay {
        Overlay::Help => draw_help(frame, area),
        Overlay::Paused => draw_paused(frame, area),
        Overlay::Levels { cursor } => {
            let mut rows = Vec::new();
            let r = draw_levels_menu(frame, area, cursor, &mut rows);
            push_hit(hits, r, None);
            hits.extend(rows);
        }
        Overlay::Won => {
            let Some(game) = &app.game else { return };
            let (r, new_btn, levels_btn) = draw_won(frame, area, game);
            push_hit(hits, r, None);
            push_hit(hits, new_btn, Some(Action::NewGame));
            push_hit(hits, levels_btn, Some(Action::OpenLevels));
        }
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let (_, inner) = popup(frame, area, 44, 7, "Terminal too small");
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(format!("need at least {MIN_W}x{MIN_H}")),
            Line::raw(format!("now {}x{}", area.width, area.height)),
            Line::raw("enlarge the window, then keep playing"),
        ])
        .alignment(Alignment::Center),
        inner,
    );
}

fn draw_title(frame: &mut Frame, area: Rect, cursor: usize, hits: &mut Vec<Hit>) {
    let title = vec![
        Line::styled(
            "TUDOKU",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Line::styled("offline sudoku for your terminal", Style::default().fg(DIM)),
        Line::from(""),
        Line::styled(
            "pick a difficulty to deal a fresh puzzle",
            Style::default().fg(INK),
        ),
    ];
    let menu = centered_rect(area, 50, 16);
    let tr = Rect::new(area.x, menu.y.saturating_sub(6), area.width, 5);
    frame.render_widget(Paragraph::new(title).alignment(Alignment::Center), tr);
    draw_levels_menu(frame, area, cursor, hits);
}

fn draw_panel(frame: &mut Frame, game: &Game, x: u16, y: u16, hits: &mut Vec<Hit>) {
    // Number pad: 3x3 grid of digit buttons.
    let remaining = game.remaining();
    for d in 1..=9u8 {
        let col = u16::from(d - 1) % 3;
        let row = u16::from(d - 1) / 3;
        let r = Rect::new(
            x + col * (DIGIT_W + DIGIT_GAP),
            y + row * CELL_H,
            DIGIT_W,
            CELL_H,
        );
        let left = remaining[usize::from(d)];
        let done = left == 0;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if done { GRID_THIN } else { ACCENT }));
        let inner = block.inner(r);
        frame.render_widget(block, r);
        push_hit(hits, r, Some(Action::Board(BoardAction::Digit(d))));
        let label = if done {
            Line::styled(
                " ✓ ",
                Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
            )
        } else {
            Line::from(vec![
                Span::styled(
                    format!("{d}"),
                    Style::default().fg(INK).add_modifier(Modifier::BOLD),
                ),
                Span::styled("·", Style::default().fg(DIM).add_modifier(Modifier::DIM)),
                Span::styled(
                    format!("{left}"),
                    Style::default().fg(DIM).add_modifier(Modifier::DIM),
                ),
            ])
        };
        frame.render_widget(Paragraph::new(label).alignment(Alignment::Center), inner);
    }

    // Action buttons: two columns of four, below the number pad.
    let btn_y0 = y + DIGIT_GRID_H + PANEL_GROUP_GAP;
    let specs = [
        Button {
            title: "Notes",
            hint: if game.notes_mode { "on" } else { "n" },
            active: game.notes_mode,
            action: Action::Board(BoardAction::ToggleNotes),
        },
        Button {
            title: "Undo",
            hint: "u",
            active: false,
            action: Action::Board(BoardAction::Undo),
        },
        Button {
            title: "Hint",
            hint: "H",
            active: false,
            action: Action::Board(BoardAction::Hint),
        },
        Button {
            title: "Erase",
            hint: "e",
            active: false,
            action: Action::Board(BoardAction::Erase),
        },
        Button {
            title: "New",
            hint: "N",
            active: false,
            action: Action::NewGame,
        },
        Button {
            title: "Level",
            hint: "d",
            active: false,
            action: Action::OpenLevels,
        },
        Button {
            title: "Pause",
            hint: "p",
            active: false,
            action: Action::Pause,
        },
        Button {
            title: "Help",
            hint: "?",
            active: false,
            action: Action::OpenHelp,
        },
    ];
    for (k, btn) in specs.iter().enumerate() {
        let col = k as u16 % 2;
        let row = k as u16 / 2;
        let r = Rect::new(
            x + col * (BTN_W + BTN_GAP),
            btn_y0 + row * CELL_H,
            BTN_W,
            CELL_H,
        );
        draw_button(frame, r, btn, hits);
    }
}

fn draw_game(frame: &mut Frame, area: Rect, game: &Game, hits: &mut Vec<Hit>) {
    // Header (2 lines).
    let (top, bottom) = header_lines(game);
    frame.render_widget(
        Paragraph::new(vec![top, bottom]),
        Rect::new(area.x, area.y, area.width, 2),
    );
    // Divider.
    let divider = Rect::new(area.x, area.y + 2, area.width, 1);
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(area.width))).style(Style::default().fg(GRID_THIN)),
        divider,
    );

    // Board on the left, number pad + action buttons in a side panel on the
    // right — centered together as one block. This pins the required
    // terminal height to the board's own height instead of stacking the
    // panel underneath it, since terminals are almost always much wider
    // than they are tall.
    let board_y = area.y + 3;
    let ox = area.x + area.width.saturating_sub(BLOCK_W) / 2;
    draw_board(frame, game, ox, board_y, hits);
    let panel_x = ox + BOARD_W + PANEL_GAP;
    let panel_y = board_y + (BOARD_H - PANEL_H) / 2;
    draw_panel(frame, game, panel_x, panel_y, hits);

    // Footer hints, directly under the board.
    let footer_y = board_y + BOARD_H;
    if footer_y < area.bottom() {
        let footer =
            "arrows/hjkl  1-9 fill  n notes  u undo  H hint  e erase  d level  ? help  q quit";
        frame.render_widget(
            Paragraph::new(Line::styled(footer, Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            Rect::new(area.x, footer_y, area.width, 1),
        );
    }
}

// ------------------------------------------------------------ main render

/// Draw the whole screen; return click targets for the event loop.
pub fn render(frame: &mut Frame, app: &App) -> Vec<Hit> {
    let mut hits = Vec::new();
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(INK)),
        area,
    );

    if area.width < MIN_W || area.height < MIN_H {
        draw_too_small(frame, area);
        return hits;
    }

    match &app.game {
        None => draw_title(frame, area, app.menu_cursor, &mut hits),
        Some(game) => draw_game(frame, area, game, &mut hits),
    }
    for &overlay in &app.overlays {
        draw_overlay(frame, area, app, overlay, &mut hits);
    }
    if let Some(level) = app.dealing {
        push_hit(&mut hits, area, None);
        draw_dealing(frame, area, level);
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sudoku::test_board;
    use ratatui::{Terminal, backend::TestBackend};

    fn app_with_game() -> App {
        let mut app = App::new();
        let (p, s) = test_board();
        app.start_game(Game::new(Difficulty::Easy, p, s));
        app
    }

    fn draw(app: &App, w: u16, h: u16) -> (Vec<String>, Vec<Hit>) {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut hits = Vec::new();
        terminal.draw(|f| hits = render(f, app)).unwrap();
        let buf = terminal.backend().buffer();
        let lines = (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect();
        (lines, hits)
    }

    fn text(lines: &[String]) -> String {
        lines.join("\n")
    }

    fn cell_hits(hits: &[Hit]) -> usize {
        hits.iter()
            .filter(|h| matches!(h.action, Some(Action::Board(BoardAction::Select(..)))))
            .count()
    }

    #[test]
    fn cells_render_square_on_typical_fonts() {
        // Terminal glyph cells run ~2x taller than wide, so CELL_W should be
        // ~2x CELL_H for a visually square grid. Tolerate ±15% for fonts
        // whose glyph aspect strays from exactly 2:1.
        let ratio = f32::from(CELL_W) / f32::from(CELL_H);
        assert!(
            (ratio - 2.0).abs() <= 0.3,
            "cell aspect {CELL_W}x{CELL_H} is not square on ~2:1 fonts"
        );
    }

    #[test]
    fn minimum_terminal_still_fits_board_and_controls() {
        let app = app_with_game();
        let (lines, hits) = draw(&app, MIN_W, MIN_H);
        let screen = text(&lines);
        assert!(
            !screen.contains("too small"),
            "board should fit at {MIN_W}x{MIN_H}"
        );
        assert_eq!(cell_hits(&hits), 81);
        for label in ["Notes", "Undo", "Hint", "Erase"] {
            assert!(
                screen.contains(label),
                "{label} button missing at minimum size"
            );
        }
    }

    #[test]
    fn full_game_screen_has_board_and_controls() {
        let app = app_with_game();
        let (lines, hits) = draw(&app, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("TUDOKU"), "title missing");
        assert!(screen.contains("Easy"), "difficulty missing");
        for label in ["Notes", "Undo", "Hint", "Erase", "Level", "Pause", "Help"] {
            assert!(screen.contains(label), "{label} button missing");
        }
        // Board cells and number pad are clickable.
        let has = |want: Action| hits.iter().any(|h| h.action == Some(want));
        assert!(has(Action::Board(BoardAction::Select(0, 0))));
        assert!(has(Action::Board(BoardAction::Digit(5))));
        // Every cell is hit-testable exactly once.
        assert_eq!(cell_hits(&hits), 81);
    }

    #[test]
    fn title_screen_lists_all_difficulties() {
        let app = App::new();
        let (lines, hits) = draw(&app, 100, 50);
        let screen = text(&lines);
        for d in Difficulty::ALL {
            assert!(screen.contains(d.name()), "{d:?} missing on title");
            assert!(hits.iter().any(|h| h.action == Some(Action::StartLevel(d))));
        }
    }

    #[test]
    fn small_terminal_shows_size_warning() {
        let app = app_with_game();
        let (lines, _) = draw(&app, 50, 20);
        assert!(text(&lines).contains("too small"));
    }

    #[test]
    fn win_and_pause_overlays_render() {
        let mut app = app_with_game();
        app.overlays.push(Overlay::Won);
        let (lines, _) = draw(&app, 100, 50);
        assert!(text(&lines).contains("Solved!"));

        let mut app = app_with_game();
        app.pause();
        let (lines, _) = draw(&app, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("Paused"));
        // Board hidden while paused: first cell interior is blank.
        // Board origin for width 100: ox=4, row 0 at y=4..6, interior x=5..10.
        let row: Vec<char> = lines[5].chars().collect();
        assert!(row[5..11].iter().all(|&c| c == ' '));
    }

    #[test]
    fn win_popup_action_buttons_are_visible() {
        let mut app = app_with_game();
        app.overlays.push(Overlay::Won);
        let (lines, hits) = draw(&app, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("New"), "win popup New button text missing");
        assert!(
            screen.contains("Levels"),
            "win popup Levels button text missing"
        );
        for want in [Action::NewGame, Action::OpenLevels] {
            // The topmost hit for this action is the popup's button.
            let hit = hits
                .iter()
                .rev()
                .find(|h| h.action == Some(want))
                .unwrap_or_else(|| panic!("win popup hit missing: {want:?}"));
            // A bordered button needs a content row: height >= 3.
            assert!(
                hit.rect.height >= 3,
                "win popup button too short to show text: {hit:?}"
            );
            let (x, y) = (hit.rect.x + 1, hit.rect.y + 1);
            assert_eq!(hit_at(&hits, x, y), Some(want), "button is clickable");
        }
    }

    #[test]
    fn notes_render_as_mini_grid() {
        let mut app = app_with_game();
        let game = app.game.as_mut().unwrap();
        game.toggle_notes_mode();
        // Cell (0,3) is empty in the test puzzle; pencil 1, 5 and 9.
        game.set_selected(0, 3);
        game.enter_digit(1);
        game.enter_digit(5);
        game.enter_digit(9);
        game.toggle_notes_mode();
        let (lines, _) = draw(&app, 100, 50);
        // Cell (0,3) interior starts at x=4+1+3*7=26, rows y=4..6.
        // The 5-char mini-grid row sits left-aligned in the 6-wide cell.
        let top: Vec<char> = lines[4].chars().collect();
        let mid: Vec<char> = lines[5].chars().collect();
        let bot: Vec<char> = lines[6].chars().collect();
        assert_eq!(top[26..32].iter().collect::<String>(), "1     ");
        assert_eq!(mid[26..32].iter().collect::<String>(), "  5   ");
        assert_eq!(bot[26..32].iter().collect::<String>(), "    9 ");
    }

    #[test]
    fn every_overlay_swallows_clicks_outside_its_own_rect() {
        for overlay in [
            Overlay::Help,
            Overlay::Paused,
            Overlay::Won,
            Overlay::Levels { cursor: 0 },
        ] {
            let mut app = app_with_game();
            app.overlays.push(overlay);
            let (_, hits) = draw(&app, MIN_W, MIN_H);
            // A board cell near the left edge, well outside the popup: the
            // click must close the popup, never select the cell underneath.
            assert_eq!(
                hit_at(&hits, 2, MIN_H - 2),
                Some(Action::Close),
                "{overlay:?}"
            );
        }
    }

    #[test]
    fn dealing_blocks_every_click() {
        let mut app = app_with_game();
        app.dealing = Some(Difficulty::Hard);
        let (lines, hits) = draw(&app, 100, 50);
        assert!(text(&lines).contains("Dealing"));
        for (x, y) in [(2, 48), (10, 8), (50, 25)] {
            assert_eq!(hit_at(&hits, x, y), None);
        }
    }

    #[test]
    fn help_popup_lists_controls() {
        let mut app = app_with_game();
        app.overlays.push(Overlay::Help);
        let (lines, _) = draw(&app, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("How to play"));
        assert!(screen.contains("Hint"));
    }

    #[test]
    fn time_formats_with_hours_when_needed() {
        use std::time::Duration;
        assert_eq!(format_time(Duration::from_secs(75)), "01:15");
        assert_eq!(format_time(Duration::from_secs(3600 + 62)), "1:01:02");
    }
}
