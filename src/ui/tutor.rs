//! The tutor's pane, in the side panel under the number pad: first the
//! name of a strategy to look for, then the step itself in r#c# terms,
//! with the keys that go on from there.

use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{ACCENT, GREEN, Hit, INK, MUTED, PANEL_W, RED, popup_block, push_hit};
use crate::app::{Action, Tutor, TutorStage};
use crate::logic::{Effect, Step, digit_list, rc};
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
        Effect::Eliminate(removals) => match step.pattern.digit {
            Some(digit) => {
                lines.push(Line::styled(
                    format!("Not {digit}:"),
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ));
                let cells: Vec<String> = removals.iter().map(|&(cell, _)| rc(cell)).collect();
                para(lines, &cells.join(", "), ink);
            }
            None => {
                for &(cell, mask) in removals {
                    let text = format!("{}: not {}", rc(cell), digit_list(mask));
                    para(lines, &text, ink);
                }
            }
        },
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
}
