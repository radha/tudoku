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

use std::time::Duration;

use crate::app::{Action, App, BoardAction, MenuRow, Overlay};
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

// Every cell has its own frame, so the grid pitch is (CELL_W+1) x (CELL_H+1).
// Terminal glyphs run ~2x taller than wide, so an 8x4 pitch looks square.
// 7 wide also centers the pencil-mark mini-grid (" 1 2 3 ") on the value.
pub const CELL_W: u16 = 7;
pub const CELL_H: u16 = 3;
pub const BOARD_W: u16 = 9 * (CELL_W + 1) + 1; // 73
pub const BOARD_H: u16 = 9 * (CELL_H + 1) + 1; // 37

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
const BLOCK_W: u16 = BOARD_W + PANEL_GAP + PANEL_W; // 101

pub const MIN_W: u16 = BLOCK_W + 2; // 103: room for the row labels
pub const MIN_H: u16 = 3 + BOARD_H; // 40: header (2 lines) + divider + board

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

/// Top-left of the board for a screen `area`: the board and side panel
/// are centered together, under the header and divider.
fn board_origin(area: Rect) -> (u16, u16) {
    (area.x + area.width.saturating_sub(BLOCK_W) / 2, area.y + 3)
}

/// Top-left of a cell's interior.
fn cell_screen(ox: u16, oy: u16, row: usize, col: usize) -> (u16, u16) {
    let x = ox + 1 + col as u16 * (CELL_W + 1);
    let y = oy + 1 + row as u16 * (CELL_H + 1);
    (x, y)
}

/// The grid line glyph at board offset `(gx, gy)`, and whether it is part
/// of a heavy box line; `None` inside a cell. Light lines frame cells,
/// heavy ones frame boxes and the board, and every crossing gets the
/// junction glyph that joins exactly the strokes meeting there.
fn grid_glyph(gx: u16, gy: u16) -> Option<(char, bool)> {
    let on_v = gx.is_multiple_of(CELL_W + 1);
    let on_h = gy.is_multiple_of(CELL_H + 1);
    let (c, r) = (gx / (CELL_W + 1), gy / (CELL_H + 1));
    let heavy_v = on_v && c.is_multiple_of(3);
    let heavy_h = on_h && r.is_multiple_of(3);
    let glyph = match (on_v, on_h) {
        (false, false) => return None,
        (true, false) => {
            if heavy_v {
                '┃'
            } else {
                '│'
            }
        }
        (false, true) => {
            if heavy_h {
                '━'
            } else {
                '─'
            }
        }
        (true, true) => match (c, r) {
            (0, 0) => '┏',
            (9, 0) => '┓',
            (0, 9) => '┗',
            (9, 9) => '┛',
            (_, 0) if heavy_v => '┳',
            (_, 0) => '┯',
            (_, 9) if heavy_v => '┻',
            (_, 9) => '┷',
            (0, _) if heavy_h => '┣',
            (0, _) => '┠',
            (9, _) if heavy_h => '┫',
            (9, _) => '┨',
            _ => match (heavy_v, heavy_h) {
                (true, true) => '╋',
                (true, false) => '╂',
                (false, true) => '┿',
                (false, false) => '┼',
            },
        },
    };
    Some((glyph, heavy_v || heavy_h))
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
                // Pencil marks as a 3x3 mini-grid, centered on the value spot.
                let note_style = Style::default().fg(DIM).bg(bg).add_modifier(Modifier::DIM);
                for d in 1..=9u8 {
                    if game.notes[i] & (1 << d) != 0 {
                        let k = u16::from(d - 1);
                        buf[(x + 1 + 2 * (k % 3), y + k / 3)]
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
        for gx in 0..BOARD_W {
            if let Some((glyph, heavy)) = grid_glyph(gx, gy) {
                buf[(ox + gx, oy + gy)].set_char(glyph).set_fg(if heavy {
                    GRID_BOX
                } else {
                    GRID_THIN
                });
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

/// "best 08:12", or a dash before the first hint-free solve.
fn best_label(best: Option<Duration>) -> String {
    best.map_or_else(
        || "best —".to_string(),
        |t| format!("best {}", format_time(t)),
    )
}

fn header_lines(game: &Game, best: Option<Duration>) -> (Line<'static>, Line<'static>) {
    let diff = game.difficulty;
    let (done, total) = game.progress();
    let bar_w = 14;
    let filled = done * bar_w / total.max(1);
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
            format!("{}   ", diff.blurb()),
            Style::default().fg(DIM).add_modifier(Modifier::DIM),
        ),
        Span::styled(best_label(best), Style::default().fg(AMBER)),
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

/// Text width of one level-menu row.
const MENU_TEXT_W: usize = 58;

/// A menu of levels (and, on the title screen, "Continue"), with best
/// times. Returns the popup's frame; each row registers its own click.
fn draw_levels_menu(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    rows: &[MenuRow],
    cursor: usize,
    footer: &str,
    hits: &mut Vec<Hit>,
) -> Rect {
    let title = if rows.contains(&MenuRow::Continue) {
        "Welcome back"
    } else {
        "New game — pick difficulty"
    };
    let h = rows.len() as u16 + 6;
    let (r, inner) = popup(frame, area, MENU_TEXT_W as u16 + 4, h, title);
    let mut lines: Vec<Line> = vec![
        Line::styled(
            " arrows/hjkl + Enter, keys 1-5, or click",
            Style::default().fg(DIM).add_modifier(Modifier::DIM),
        ),
        Line::from(""),
    ];
    for (i, &row) in rows.iter().enumerate() {
        let sel = i == cursor;
        let marker = if sel { "▶" } else { " " };
        let (text, action) = match row {
            MenuRow::Continue => {
                let Some(g) = &app.resume else { continue };
                let (done, total) = g.progress();
                let text = format!(
                    " {marker} c. Continue  {} · {} · {done}/{total} done",
                    g.difficulty.name(),
                    format_time(g.elapsed()),
                );
                (text, Action::Continue)
            }
            MenuRow::Level(d) => {
                let best = best_label(app.stats.level(d).best());
                let text = format!(
                    " {marker} {}. {:<6}  {:<31}  {best:>10}",
                    d.index() + 1,
                    d.name(),
                    d.blurb(),
                );
                (text, Action::StartLevel(d))
            }
        };
        lines.push(Line::styled(
            format!("{text:<MENU_TEXT_W$}"),
            Style::default()
                .fg(if sel { Color::White } else { INK })
                .bg(if sel { SEL_BG } else { POPUP_BG })
                .add_modifier(Modifier::BOLD),
        ));
        let hit = Rect::new(r.x + 1, inner.y + 2 + i as u16, r.width - 2, 1);
        push_hit(hits, hit, Some(action));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(footer.to_string(), Style::default().fg(DIM)));
    frame.render_widget(Paragraph::new(lines), inner);
    r
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let rows = [
        ("Move", "arrows / hjkl, or click a cell"),
        ("Fill", "1-9  (click a cell, then a number)"),
        ("Notes", "n toggles pencil marks, then 1-9"),
        ("Erase", "0 / x / e / Backspace / Delete"),
        ("Undo", "u  or Ctrl+Z"),
        ("Hint", "H (capital) — fills the next logical cell"),
        ("New", "N (capital) — fresh puzzle, same level"),
        ("Level", "d — change difficulty"),
        ("Pause", "p — hides the board, stops timer"),
        ("Saves", "automatic — Continue from the title screen"),
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
    let (_, inner) = popup(frame, area, 56, lines.len() as u16 + 2, "How to play  (?)");
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
fn draw_won(frame: &mut Frame, area: Rect, game: &Game, app: &App) -> (Rect, Rect, Rect) {
    let (r, inner) = popup(frame, area, 48, 13, "Solved!");
    let best = app.stats.level(game.difficulty).best();
    let record = if app.new_best {
        Line::styled(
            "★ New best time! ★",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        )
    } else if game.hints_used > 0 {
        Line::styled(
            "best times count hint-free solves only",
            Style::default().fg(DIM),
        )
    } else {
        Line::styled(best_label(best), Style::default().fg(DIM))
    };
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
        record,
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
            let levels = Difficulty::ALL.map(MenuRow::Level);
            let footer = " Enter start   Esc cancel";
            let r = draw_levels_menu(frame, area, app, &levels, cursor, footer, &mut rows);
            push_hit(hits, r, None);
            hits.extend(rows);
        }
        Overlay::Won => {
            let Some(game) = &app.game else { return };
            let (r, new_btn, levels_btn) = draw_won(frame, area, game, app);
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

fn draw_title(frame: &mut Frame, area: Rect, app: &App, hits: &mut Vec<Hit>) {
    let rows = app.menu_rows();
    let prompt = if app.resume.is_some() {
        "pick up where you left off, or deal a fresh puzzle"
    } else {
        "pick a difficulty to deal a fresh puzzle"
    };
    let title = vec![
        Line::styled(
            "TUDOKU",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Line::styled("offline sudoku for your terminal", Style::default().fg(DIM)),
        Line::from(""),
        Line::styled(prompt, Style::default().fg(INK)),
    ];
    let footer = " Enter start   ? help   q quit";
    let menu = draw_levels_menu(frame, area, app, &rows, app.menu_cursor, footer, hits);
    let tr = Rect::new(area.x, menu.y.saturating_sub(6), area.width, 5);
    frame.render_widget(Paragraph::new(title).alignment(Alignment::Center), tr);
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

fn draw_game(frame: &mut Frame, area: Rect, app: &App, game: &Game, hits: &mut Vec<Hit>) {
    // Board on the left, number pad + action buttons in a side panel on the
    // right, centered together as one block. This pins the required
    // terminal height to the board's own height instead of stacking the
    // panel underneath it, since terminals are almost always much wider
    // than they are tall. The header lines up with the board's left edge.
    let (ox, oy) = board_origin(area);
    let header = Rect::new(ox, area.y, area.right() - ox, 2);
    let best = app.stats.level(game.difficulty).best();
    let (top, bottom) = header_lines(game, best);
    frame.render_widget(Paragraph::new(vec![top, bottom]), header);
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(BLOCK_W))).style(Style::default().fg(GRID_THIN)),
        Rect::new(ox, area.y + 2, BLOCK_W, 1),
    );

    draw_board(frame, game, ox, oy, hits);
    let panel_x = ox + BOARD_W + PANEL_GAP;
    let panel_y = oy + (BOARD_H - PANEL_H) / 2;
    draw_panel(frame, game, panel_x, panel_y, hits);

    // Footer hints, directly under the board.
    let footer_y = oy + BOARD_H;
    if footer_y < area.bottom() {
        let footer =
            "arrows/hjkl  1-9 fill  n notes  u undo  H hint  e erase  d level  ? help  q quit";
        frame.render_widget(
            Paragraph::new(Line::styled(footer, Style::default().fg(DIM))),
            Rect::new(ox, footer_y, area.right() - ox, 1),
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
        None => draw_title(frame, area, app, &mut hits),
        Some(game) => draw_game(frame, area, app, game, &mut hits),
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
    use crate::store::Store;
    use crate::sudoku::test_board;
    use ratatui::{Terminal, backend::TestBackend};

    fn app_with_game() -> App {
        let mut app = App::new(Store::none());
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

    /// Screen width for tests: a little roomier than the minimum.
    const W: u16 = 110;

    fn cell_hits(hits: &[Hit]) -> usize {
        hits.iter()
            .filter(|h| matches!(h.action, Some(Action::Board(BoardAction::Select(..)))))
            .count()
    }

    /// Text row `dy` of a cell's interior, as drawn `W` columns wide.
    fn cell_row(lines: &[String], row: usize, col: usize, dy: u16) -> String {
        let (ox, oy) = board_origin(Rect::new(0, 0, W, 50));
        let (x, y) = cell_screen(ox, oy, row, col);
        lines[usize::from(y + dy)]
            .chars()
            .skip(usize::from(x))
            .take(usize::from(CELL_W))
            .collect()
    }

    /// The glyph drawn at board offset `(gx, gy)`, `W` columns wide.
    fn board_char(lines: &[String], gx: u16, gy: u16) -> char {
        let (ox, oy) = board_origin(Rect::new(0, 0, W, 50));
        let line = &lines[usize::from(oy + gy)];
        line.chars().nth(usize::from(ox + gx)).unwrap()
    }

    #[test]
    fn cells_render_square_on_typical_fonts() {
        // Terminal glyph cells run ~2x taller than wide, so the grid pitch
        // (a cell plus its line) should be ~2x as wide as it is tall.
        // Tolerate ±15% for fonts whose glyph aspect strays from 2:1.
        let ratio = f32::from(CELL_W + 1) / f32::from(CELL_H + 1);
        assert!(
            (ratio - 2.0).abs() <= 0.3,
            "cell pitch {}x{} is not square on ~2:1 fonts",
            CELL_W + 1,
            CELL_H + 1
        );
    }

    #[test]
    fn grid_lines_use_matching_junctions() {
        let (lines, _) = draw(&app_with_game(), W, 50);
        let pitch_x = CELL_W + 1;
        let pitch_y = CELL_H + 1;
        let at = |cx: u16, cy: u16| board_char(&lines, cx * pitch_x, cy * pitch_y);
        // Corners and edge junctions: T-pieces, never crosses poking out.
        assert_eq!(at(0, 0), '┏');
        assert_eq!(at(9, 9), '┛');
        assert_eq!(at(1, 0), '┯', "cell line meets the top edge");
        assert_eq!(at(3, 0), '┳', "box line meets the top edge");
        assert_eq!(at(0, 1), '┠');
        assert_eq!(at(9, 3), '┫');
        // Inside: every mix of light cell lines and heavy box lines.
        assert_eq!(at(1, 1), '┼');
        assert_eq!(at(3, 1), '╂');
        assert_eq!(at(1, 3), '┿');
        assert_eq!(at(3, 3), '╋');
        // Every row has its own line, even inside a box.
        assert_eq!(board_char(&lines, 1, pitch_y), '─');
        assert_eq!(board_char(&lines, 1, 3 * pitch_y), '━');
        assert_eq!(board_char(&lines, pitch_x, 1), '│');
        assert_eq!(board_char(&lines, 3 * pitch_x, 1), '┃');
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
        let (lines, hits) = draw(&app, W, 50);
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
        let app = App::new(Store::none());
        let (lines, hits) = draw(&app, W, 50);
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
        let (lines, _) = draw(&app, W, 50);
        assert!(text(&lines).contains("Solved!"));

        let mut app = app_with_game();
        app.pause();
        let (lines, _) = draw(&app, W, 50);
        let screen = text(&lines);
        assert!(screen.contains("Paused"));
        // Board hidden while paused: the given 5 at (0,0) isn't drawn.
        assert_eq!(cell_row(&lines, 0, 0, 1), " ".repeat(usize::from(CELL_W)));
        let (lines, _) = draw(&app_with_game(), W, 50);
        assert_eq!(cell_row(&lines, 0, 0, 1), "   5   ", "and is when unpaused");
    }

    #[test]
    fn win_popup_action_buttons_are_visible() {
        let mut app = app_with_game();
        app.overlays.push(Overlay::Won);
        let (lines, hits) = draw(&app, W, 50);
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
        let (lines, _) = draw(&app, W, 50);
        // The mini-grid is centered, so its middle column (2, 5, 8) lines
        // up with where a placed digit goes.
        assert_eq!(cell_row(&lines, 0, 3, 0), " 1     ");
        assert_eq!(cell_row(&lines, 0, 3, 1), "   5   ");
        assert_eq!(cell_row(&lines, 0, 3, 2), "     9 ");
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
        let (lines, hits) = draw(&app, W, 50);
        assert!(text(&lines).contains("Dealing"));
        for (x, y) in [(2, 48), (10, 8), (50, 25)] {
            assert_eq!(hit_at(&hits, x, y), None);
        }
    }

    #[test]
    fn help_popup_lists_controls() {
        let mut app = app_with_game();
        app.overlays.push(Overlay::Help);
        let (lines, _) = draw(&app, W, 50);
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
