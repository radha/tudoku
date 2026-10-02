//! Rendering: board, number bar, buttons, popups. Pure view code —
//! `render` draws everything and returns the clickable areas for the
//! event loop to hit-test mouse clicks against.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::game::Game;
use crate::sudoku::{Difficulty, box_of, idx};

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

#[derive(Debug, Clone, Copy)]
pub enum ClickAction {
    Cell(usize, usize),
    Digit(u8),
    Notes,
    Undo,
    Hint,
    Erase,
    New,
    Level,
    Pause,
    Help,
    LevelChoice(usize),
    MenuChoice(usize),
    Close,
    WinNew,
    WinLevels,
    /// Swallows a click on the modal backdrop so it can't reach whatever is
    /// drawn underneath an open popup.
    Blocked,
}

#[derive(Debug, Clone, Copy)]
pub struct Hit {
    pub rect: Rect,
    pub action: ClickAction,
}

impl Hit {
    fn new(rect: Rect, action: ClickAction) -> Self {
        Self { rect, action }
    }
}

pub fn hit_at(hits: &[Hit], x: u16, y: u16) -> Option<ClickAction> {
    hits.iter()
        .rev()
        .find(|h| {
            x >= h.rect.x
                && x < h.rect.x + h.rect.width
                && y >= h.rect.y
                && y < h.rect.y + h.rect.height
        })
        .map(|h| h.action)
}

// ------------------------------------------------------------ state in

pub struct RenderState<'a> {
    pub game: Option<&'a Game>,
    pub show_help: bool,
    pub show_levels: bool,
    pub level_cursor: usize,
    pub menu_cursor: usize,
    pub generating: bool,
}

// ------------------------------------------------------------ helpers

pub fn format_time(d: std::time::Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn centered_rect(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn cell_screen(ox: u16, oy: u16, row: usize, col: usize) -> (u16, u16) {
    let x = ox + 1 + col as u16 * (CELL_W + 1);
    let y = oy + 1 + row as u16 * CELL_H + (row >= 3) as u16 + (row >= 6) as u16;
    (x, y)
}

fn is_hline(y: u16, oy: u16) -> bool {
    // One horizontal guide per 3-row box band: top, between bands, bottom.
    let band = 3 * CELL_H + 1;
    y.saturating_sub(oy).is_multiple_of(band)
}

// ------------------------------------------------------------ board

#[allow(clippy::too_many_lines)]
fn draw_board(frame: &mut Frame, game: &Game, ox: u16, oy: u16, hits: &mut Vec<Hit>) {
    let buf = frame.buffer_mut();
    let sel = game.selected_idx();
    let sel_val = game.values[sel];
    let (sel_r, sel_c) = game.selected;

    // Column numbers above the board.
    for c in 0..9 {
        let (x, _) = cell_screen(ox, oy, 0, c);
        let s = (c + 1).to_string();
        let span_x = x + CELL_W / 2;
        if oy > 0 && span_x < buf.area.width {
            buf[(span_x, oy - 1)]
                .set_char(s.chars().next().unwrap())
                .set_fg(DIM);
        }
    }

    // Cell interiors.
    for r in 0..9 {
        // Row number to the left, vertically centered on the cell.
        let (_, y) = cell_screen(ox, oy, r, 0);
        if ox > 0 {
            buf[(ox - 1, y + CELL_H / 2)]
                .set_char(char::from(b'1' + r as u8))
                .set_fg(DIM);
        }
        for c in 0..9 {
            let i = idx(r, c);
            let (x, y) = cell_screen(ox, oy, r, c);
            let is_sel = r == sel_r && c == sel_c;
            let peer =
                !is_sel && (r == sel_r || c == sel_c || box_of(r, c) == box_of(sel_r, sel_c));
            let same_val =
                !is_sel && sel_val != 0 && game.values[i] == sel_val && game.values[i] != 0;
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
            // Paint interior.
            for dy in 0..CELL_H {
                for k in 0..CELL_W {
                    buf[(x + k, y + dy)].set_bg(bg);
                }
            }
            hits.push(Hit::new(
                Rect {
                    x,
                    y,
                    width: CELL_W,
                    height: CELL_H,
                },
                ClickAction::Cell(r, c),
            ));
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
                let ch = char::from(b'0' + v);
                buf[(x + CELL_W / 2, ym)].set_char(ch).set_style(style);
                // Conflict marker: duplicates show even without color.
                if err {
                    buf[(x, ym)]
                        .set_char('!')
                        .set_style(Style::default().fg(RED).bg(bg).add_modifier(Modifier::BOLD));
                }
            } else {
                // Pencil marks as a 3x3 mini-grid filling the cell.
                let mut any = false;
                for nr in 0..3u16 {
                    let mut row = String::with_capacity(CELL_W as usize);
                    for nc in 0..3u8 {
                        if nc > 0 {
                            row.push(' ');
                        }
                        let d = nr as u8 * 3 + nc + 1;
                        if game.notes[i] & (1 << d) != 0 {
                            row.push(char::from(b'0' + d));
                            any = true;
                        } else {
                            row.push(' ');
                        }
                    }
                    for (k, ch) in row.chars().enumerate() {
                        buf[(x + k as u16, y + nr)]
                            .set_char(ch)
                            .set_style(Style::default().fg(DIM).bg(bg).add_modifier(Modifier::DIM));
                    }
                }
                if !any && is_sel {
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
        let hline = is_hline(y, oy);
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
    accent: bool,
    action: ClickAction,
}

fn draw_button(frame: &mut Frame, rect: Rect, btn: &Button, hits: &mut Vec<Hit>) {
    let border = if btn.active {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else if btn.accent {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(GRID_THIN)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border);
    frame.render_widget(block, rect);
    hits.push(Hit::new(rect, btn.action));
    if rect.width < 3 || rect.height < 2 {
        return;
    }
    let inner = Rect {
        x: rect.x + 1,
        y: rect.y + 1,
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    };
    let line = Line::from(vec![
        Span::styled(
            btn.title.to_string(),
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
        Span::styled(
            format!("{} offline", if done == total { "✓" } else { "○" }),
            Style::default().fg(DIM),
        ),
    ]);
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
        Span::styled(
            format!("{bar} {done}/{total}   "),
            Style::default().fg(if done == total { GREEN } else { DIM }),
        ),
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
        .style(Style::default().bg(Color::Rgb(15, 23, 42)))
}

fn draw_levels_popup(
    frame: &mut Frame,
    area: Rect,
    cursor: usize,
    for_menu: bool,
    hits: &mut Vec<Hit>,
) {
    let w = 46u16;
    let h = 16u16;
    let r = centered_rect(area, w, h);
    frame.render_widget(Clear, r);
    frame.render_widget(popup_block("New game — pick difficulty"), r);
    hits.push(Hit::new(r, ClickAction::Close));
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
                .bg(if sel { SEL_BG } else { Color::Reset })
                .add_modifier(Modifier::BOLD),
        )]));
        // Register a click row for each option.
        let row_y = r.y + 3 + i as u16;
        if row_y < r.y + r.height - 1 {
            hits.push(Hit::new(
                Rect {
                    x: r.x + 1,
                    y: row_y,
                    width: r.width.saturating_sub(2),
                    height: 1,
                },
                if for_menu {
                    ClickAction::MenuChoice(i)
                } else {
                    ClickAction::LevelChoice(i)
                },
            ));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" Enter start   ", Style::default().fg(DIM)),
        Span::styled("Esc cancel ", Style::default().fg(DIM)),
    ]));
    let inner = Rect {
        x: r.x + 2,
        y: r.y + 1,
        width: r.width.saturating_sub(4),
        height: r.height.saturating_sub(2),
    };
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
}

fn draw_help_popup(frame: &mut Frame, area: Rect, hits: &mut Vec<Hit>) {
    let r = centered_rect(area, 56, 21);
    frame.render_widget(Clear, r);
    frame.render_widget(popup_block("How to play  (?)"), r);
    hits.push(Hit::new(r, ClickAction::Close));
    let rows = vec![
        ("Move", "arrows / hjkl, or click a cell"),
        ("Fill", "1-9  (click a cell, then a number)"),
        ("Notes", "n toggles pencil marks, then 1-9"),
        ("Erase", "0 / Backspace / Delete / e"),
        ("Undo", "u  or Ctrl+Z"),
        ("Hint", "H (capital) — reveals & locks a cell"),
        ("New", "N (capital) — fresh puzzle, same level"),
        ("Level", "d — change difficulty"),
        ("Pause", "p — hides the board, stops timer"),
        ("Quit", "q    •    close popups: Esc"),
        ("Mouse", "everything is clickable below"),
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
    let inner = Rect {
        x: r.x + 2,
        y: r.y + 1,
        width: r.width.saturating_sub(4),
        height: r.height.saturating_sub(2),
    };
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_win_popup(frame: &mut Frame, area: Rect, game: &Game, hits: &mut Vec<Hit>) {
    let r = centered_rect(area, 48, 13);
    frame.render_widget(Clear, r);
    frame.render_widget(popup_block("Solved!"), r);
    let inner = Rect {
        x: r.x + 2,
        y: r.y + 1,
        width: r.width.saturating_sub(4),
        height: r.height.saturating_sub(2),
    };
    let lines = vec![
        Line::from(Span::styled(
            format!("{} puzzle cleared", game.difficulty.name()),
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("Time {}   ", format_time(game.elapsed())),
                Style::default().fg(INK),
            ),
            Span::styled(
                format!("Mistakes {}   ", game.mistakes),
                Style::default().fg(INK),
            ),
            Span::styled(
                format!("Hints {}", game.hints_used),
                Style::default().fg(INK),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "N new puzzle • d change level • q quit",
            Style::default().fg(DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
    // Clickable footer buttons.
    let bw = 12u16;
    let by = r.y + r.height - 4;
    let bx = r.x + (r.width.saturating_sub(bw * 2 + 2)) / 2;
    let r1 = Rect {
        x: bx,
        y: by,
        width: bw,
        height: 3,
    };
    let r2 = Rect {
        x: bx + bw + 2,
        y: by,
        width: bw,
        height: 3,
    };
    frame.render_widget(
        Paragraph::new(Line::styled(
            " ↻ New ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        ),
        r1,
    );
    frame.render_widget(
        Paragraph::new(Line::styled(" ◈ Levels ", Style::default().fg(INK)))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            ),
        r2,
    );
    hits.push(Hit::new(r1, ClickAction::WinNew));
    hits.push(Hit::new(r2, ClickAction::WinLevels));
}

// ------------------------------------------------------------ main render

/// Draw the whole screen; return click targets for the event loop.
pub fn render(frame: &mut Frame, st: &RenderState) -> Vec<Hit> {
    let mut hits = Vec::new();
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(INK)),
        area,
    );

    if area.width < MIN_W || area.height < MIN_H {
        let r = centered_rect(area, 44, 7);
        frame.render_widget(Clear, r);
        frame.render_widget(popup_block("Terminal too small"), r);
        let inner = Rect {
            x: r.x + 2,
            y: r.y + 1,
            width: r.width.saturating_sub(4),
            height: r.height.saturating_sub(2),
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::raw(format!("need at least {}x{}", MIN_W, MIN_H))),
                Line::from(Span::raw(format!("now {}x{}", area.width, area.height))),
                Line::from(Span::raw("enlarge the window, then keep playing")),
            ])
            .alignment(Alignment::Center),
            inner,
        );
        return hits;
    }

    let Some(game) = st.game else {
        // Title screen: difficulty menu, big and welcoming.
        let title = vec![
            Line::from(Span::styled(
                "TUDOKU",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "offline sudoku for your terminal",
                Style::default().fg(DIM),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "pick a difficulty to deal a fresh puzzle",
                Style::default().fg(INK),
            )),
            Line::from(""),
        ];
        let tr = Rect {
            x: area.x,
            y: area.y + (area.height.saturating_sub(15)) / 2 - 6,
            width: area.width,
            height: 5,
        };
        frame.render_widget(Paragraph::new(title).alignment(Alignment::Center), tr);
        draw_levels_popup(frame, area, st.menu_cursor, true, &mut hits);
        if st.show_help {
            hits.push(Hit::new(area, ClickAction::Blocked));
            draw_help_popup(frame, area, &mut hits);
        }
        return hits;
    };

    // Header (2 lines).
    let (top, bottom) = header_lines(game);
    frame.render_widget(
        Paragraph::new(vec![top, bottom]),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 2,
        },
    );
    // Divider.
    {
        let buf = frame.buffer_mut();
        for x in area.x..area.x + area.width {
            buf[(x, area.y + 2)].set_char('─').set_fg(GRID_THIN);
        }
    }

    // Board on the left, number pad + action buttons in a side panel on the
    // right — centered together as one block. This pins the required
    // terminal height to the board's own height (a fixed 31 rows) instead of
    // stacking the panel underneath it, since terminals are almost always
    // much wider than they are tall.
    let board_y = area.y + 3;
    let ox = area.x + area.width.saturating_sub(BLOCK_W) / 2;
    draw_board(frame, game, ox, board_y, &mut hits);

    let panel_x = ox + BOARD_W + PANEL_GAP;
    let panel_y = board_y + (BOARD_H - PANEL_H) / 2;

    // Number pad: 3x3 grid of digit buttons.
    let remaining = game.remaining();
    for d in 1..=9u8 {
        let col = u16::from(d - 1) % 3;
        let row = u16::from(d - 1) / 3;
        let r = Rect {
            x: panel_x + col * (DIGIT_W + DIGIT_GAP),
            y: panel_y + row * CELL_H,
            width: DIGIT_W,
            height: CELL_H,
        };
        let left = remaining[d as usize];
        let done = left == 0;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if done { GRID_THIN } else { ACCENT }));
        frame.render_widget(block, r);
        hits.push(Hit::new(r, ClickAction::Digit(d)));
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
        frame.render_widget(
            Paragraph::new(label).alignment(Alignment::Center),
            Rect {
                x: r.x,
                y: r.y + 1,
                width: r.width,
                height: 1,
            },
        );
    }

    // Action buttons: two columns of four, below the number pad.
    let btn_y0 = panel_y + DIGIT_GRID_H + PANEL_GROUP_GAP;
    let specs = [
        Button {
            title: "Notes",
            hint: if game.notes_mode { "on" } else { "n" },
            active: game.notes_mode,
            accent: false,
            action: ClickAction::Notes,
        },
        Button {
            title: "Undo",
            hint: "u",
            active: false,
            accent: false,
            action: ClickAction::Undo,
        },
        Button {
            title: "Hint",
            hint: "H",
            active: false,
            accent: false,
            action: ClickAction::Hint,
        },
        Button {
            title: "Erase",
            hint: "e",
            active: false,
            accent: false,
            action: ClickAction::Erase,
        },
        Button {
            title: "New",
            hint: "N",
            active: false,
            accent: false,
            action: ClickAction::New,
        },
        Button {
            title: "Level",
            hint: "d",
            active: false,
            accent: false,
            action: ClickAction::Level,
        },
        Button {
            title: "Pause",
            hint: "p",
            active: false,
            accent: false,
            action: ClickAction::Pause,
        },
        Button {
            title: "Help",
            hint: "?",
            active: false,
            accent: false,
            action: ClickAction::Help,
        },
    ];
    for (k, btn) in specs.iter().enumerate() {
        let col = k as u16 % 2;
        let row = k as u16 / 2;
        let r = Rect {
            x: panel_x + col * (BTN_W + BTN_GAP),
            y: btn_y0 + row * CELL_H,
            width: BTN_W,
            height: CELL_H,
        };
        draw_button(frame, r, btn, &mut hits);
    }

    // Footer hints, directly under the board.
    let footer_y = board_y + BOARD_H;
    if footer_y < area.y + area.height {
        let footer = Line::from(vec![
            Span::styled(" arrows/hjkl ", Style::default().fg(DIM)),
            Span::styled("1-9 fill ", Style::default().fg(DIM)),
            Span::styled("n notes ", Style::default().fg(DIM)),
            Span::styled("u undo ", Style::default().fg(DIM)),
            Span::styled("H hint ", Style::default().fg(DIM)),
            Span::styled("e erase ", Style::default().fg(DIM)),
            Span::styled("d level ", Style::default().fg(DIM)),
            Span::styled("? help ", Style::default().fg(DIM)),
            Span::styled("q quit", Style::default().fg(DIM)),
        ]);
        frame.render_widget(
            Paragraph::new(footer).alignment(Alignment::Center),
            Rect {
                x: area.x,
                y: footer_y,
                width: area.width,
                height: 1,
            },
        );
    }

    if st.generating {
        let r = centered_rect(area, 30, 5);
        frame.render_widget(Clear, r);
        frame.render_widget(popup_block("Dealing"), r);
        frame.render_widget(
            Paragraph::new(Line::styled(
                "shuffling a fresh puzzle…",
                Style::default().fg(INK),
            ))
            .alignment(Alignment::Center),
            Rect {
                x: r.x + 1,
                y: r.y + 2,
                width: r.width.saturating_sub(2),
                height: 1,
            },
        );
    }
    if game.completed {
        hits.push(Hit::new(area, ClickAction::Blocked));
        draw_win_popup(frame, area, game, &mut hits);
        if st.show_levels {
            hits.push(Hit::new(area, ClickAction::Blocked));
            draw_levels_popup(frame, area, st.level_cursor, false, &mut hits);
        }
    } else if st.show_levels {
        hits.push(Hit::new(area, ClickAction::Blocked));
        draw_levels_popup(frame, area, st.level_cursor, false, &mut hits);
    } else if game.paused {
        hits.push(Hit::new(area, ClickAction::Blocked));
        let r = centered_rect(area, 34, 7);
        frame.render_widget(Clear, r);
        frame.render_widget(popup_block("Paused"), r);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "board hidden, timer stopped",
                    Style::default().fg(INK),
                )),
                Line::from(Span::styled(
                    "press p or click to resume",
                    Style::default().fg(DIM),
                )),
            ])
            .alignment(Alignment::Center),
            Rect {
                x: r.x + 2,
                y: r.y + 2,
                width: r.width.saturating_sub(4),
                height: 3,
            },
        );
        hits.push(Hit::new(r, ClickAction::Pause));
    } else if st.show_help {
        hits.push(Hit::new(area, ClickAction::Blocked));
        draw_help_popup(frame, area, &mut hits);
    }

    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn test_game() -> Game {
        let (p, s) = crate::sudoku::test_board();
        Game::new(Difficulty::Easy, p, s)
    }

    fn draw(state: &RenderState, w: u16, h: u16) -> (Vec<String>, Vec<Hit>) {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut hits = Vec::new();
        terminal
            .draw(|f| {
                hits = render(f, state);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let area = buf.area;
        let lines: Vec<String> = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect();
        (lines, hits)
    }

    fn text(lines: &[String]) -> String {
        lines.join("\n")
    }

    #[test]
    fn cells_render_square_on_typical_fonts() {
        // Terminal glyph cells run ~2x taller than wide, so CELL_W should be
        // ~2x CELL_H for a visually square grid. Tolerate ±15% for fonts
        // whose glyph aspect strays from exactly 2:1.
        let ratio = CELL_W as f32 / CELL_H as f32;
        assert!(
            (ratio - 2.0).abs() <= 0.3,
            "cell aspect {CELL_W}x{CELL_H} is not square on ~2:1 fonts"
        );
    }

    #[test]
    fn minimum_terminal_still_fits_board_and_controls() {
        let game = test_game();
        let st = RenderState {
            game: Some(&game),
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, hits) = draw(&st, MIN_W, MIN_H);
        let screen = text(&lines);
        assert!(
            !screen.contains("too small"),
            "board should fit at {MIN_W}x{MIN_H}"
        );
        let cells = hits
            .iter()
            .filter(|h| matches!(h.action, ClickAction::Cell(_, _)))
            .count();
        assert_eq!(cells, 81);
        for label in ["Notes", "Undo", "Hint", "Erase"] {
            assert!(
                screen.contains(label),
                "{label} button missing at minimum size"
            );
        }
    }

    #[test]
    fn full_game_screen_has_board_and_controls() {
        let game = test_game();
        let st = RenderState {
            game: Some(&game),
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, hits) = draw(&st, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("TUDOKU"), "title missing");
        assert!(screen.contains("Easy"), "difficulty missing");
        for label in ["Notes", "Undo", "Hint", "Erase", "Level", "Pause", "Help"] {
            assert!(screen.contains(label), "{label} button missing");
        }
        // Board cells and number bar are clickable.
        assert!(
            hits.iter()
                .any(|h| matches!(h.action, ClickAction::Cell(0, 0)))
        );
        assert!(
            hits.iter()
                .any(|h| matches!(h.action, ClickAction::Digit(5)))
        );
        // Every cell is hit-testable exactly once.
        let cells = hits
            .iter()
            .filter(|h| matches!(h.action, ClickAction::Cell(_, _)))
            .count();
        assert_eq!(cells, 81);
    }

    #[test]
    fn title_screen_lists_all_difficulties() {
        let st = RenderState {
            game: None,
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 1,
            generating: false,
        };
        let (lines, _) = draw(&st, 100, 50);
        let screen = text(&lines);
        for name in ["Easy", "Medium", "Hard", "Expert", "Zen"] {
            assert!(screen.contains(name), "{name} missing on title");
        }
    }

    #[test]
    fn small_terminal_shows_size_warning() {
        let game = test_game();
        let st = RenderState {
            game: Some(&game),
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, _) = draw(&st, 50, 20);
        assert!(text(&lines).contains("too small"));
    }

    #[test]
    fn win_and_pause_overlays_render() {
        let mut game = test_game();
        game.values = game.solution;
        game.completed = true;
        let st = RenderState {
            game: Some(&game),
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, _) = draw(&st, 100, 50);
        assert!(text(&lines).contains("Solved!"));

        let mut game = test_game();
        game.set_paused(true);
        let st = RenderState {
            game: Some(&game),
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, _) = draw(&st, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("Paused"));
        // Board hidden while paused: first cell interior is blank.
        // Board origin for width 100: ox=4, row 0 at y=4..6, interior x=5..10.
        let row: Vec<char> = lines[5].chars().collect();
        assert!(row[5..11].iter().all(|&c| c == ' '));
    }

    #[test]
    fn win_popup_action_buttons_are_visible() {
        let mut game = test_game();
        game.values = game.solution;
        game.completed = true;
        let st = RenderState {
            game: Some(&game),
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, hits) = draw(&st, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("New"), "win popup New button text missing");
        assert!(
            screen.contains("Levels"),
            "win popup Levels button text missing"
        );
        for want in [ClickAction::WinNew, ClickAction::WinLevels] {
            let hit = hits
                .iter()
                .find(|h| std::mem::discriminant(&h.action) == std::mem::discriminant(&want))
                .unwrap_or_else(|| panic!("win popup hit missing: {want:?}"));
            // A bordered button needs a content row: height >= 3.
            assert!(
                hit.rect.height >= 3,
                "win popup button too short to show text: {hit:?}"
            );
        }
    }

    #[test]
    fn notes_render_as_mini_grid() {
        let mut game = test_game();
        game.toggle_notes_mode();
        // Cell (0,3) is empty in the test puzzle; pencil 1, 5 and 9.
        game.set_selected(0, 3);
        game.enter_digit(1);
        game.enter_digit(5);
        game.enter_digit(9);
        game.toggle_notes_mode();
        let st = RenderState {
            game: Some(&game),
            show_help: false,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, _) = draw(&st, 100, 50);
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
    fn open_popup_blocks_clicks_outside_its_own_rect() {
        let game = test_game();
        let st = RenderState {
            game: Some(&game),
            show_help: true,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (_, hits) = draw(&st, MIN_W, MIN_H);
        // A board cell near the left edge, well outside the centered Help
        // popup: must resolve to Blocked, not to the cell underneath.
        let action = hit_at(&hits, 2, MIN_H - 2).expect("some hit under the backdrop");
        assert!(
            matches!(action, ClickAction::Blocked),
            "click outside an open popup must be swallowed, got {action:?}"
        );
    }

    #[test]
    fn help_popup_lists_controls() {
        let game = test_game();
        let st = RenderState {
            game: Some(&game),
            show_help: true,
            show_levels: false,
            level_cursor: 0,
            menu_cursor: 0,
            generating: false,
        };
        let (lines, _) = draw(&st, 100, 50);
        let screen = text(&lines);
        assert!(screen.contains("How to play"));
        assert!(screen.contains("Hint"));
    }
}
