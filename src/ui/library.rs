//! The techniques library: a map of every strategy by level, and each
//! strategy's lesson, which is the tutor's explain screen with the
//! library's own header and keys.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::tutor::{LIBRARY_KEYS, explain_header, heading};
use super::{
    ACCENT, BG, BRIGHT, GREEN, Hit, INK, LINE, MUTED, SEL_BG, board_origin, draw_explain,
    layout_top, popup, push_hit,
};
use crate::app::Action;
use crate::deal::{Difficulty, level_of};
use crate::library::{Library, column, place};
use crate::logic::Technique;
use crate::tutor::examples::example;
use crate::tutor::lessons::gist;

/// The map's popup: the indent, five columns but the last one's trailing
/// gap, and a frame with a column of padding. At 103 it just fits `MIN_W`.
const MAP_W: u16 = 1 + 5 * COL_W - 2 + 4;
const MAP_H: u16 = 22;
/// One level's column: the longest name (18), and room between columns.
const COL_W: u16 = 20;
/// Rows of the map's inside above the first technique: the hint, a blank
/// line, the level headings and their rules.
const NAMES_Y: u16 = 4;
/// Everything inside the map is indented a column, like the level menu's
/// rows; the last column's trailing gap is what gives way.
const INDENT: &str = " ";

/// Draw the library: the map, or a lesson. A map `covered` by a popup
/// keeps only its header: the rest would just peek out around it.
pub(super) fn draw_library(
    frame: &mut Frame,
    area: Rect,
    library: Library,
    covered: bool,
    hits: &mut Vec<Hit>,
) {
    match library {
        Library::Lesson { at, stage } => match example(at) {
            Some(ex) => {
                let rest = format!(
                    "  {} · {} of {}",
                    level_of(at).name(),
                    place(at),
                    Technique::ALL.len()
                );
                let top = badges(Span::styled(rest, Style::default().fg(MUTED)));
                let header = (top, explain_header(at, stage).1);
                draw_explain(frame, area, header, &ex, stage, &LIBRARY_KEYS, hits);
            }
            None => draw_map(frame, area, at, covered, hits),
        },
        Library::Map { at } => draw_map(frame, area, at, covered, hits),
    }
}

/// The header's first line: the TUDOKU and LIBRARY badges, then `rest`.
fn badges(rest: Span<'static>) -> Line<'static> {
    let badge = |fill| {
        Style::default()
            .fg(BG)
            .bg(fill)
            .add_modifier(Modifier::BOLD)
    };
    Line::from(vec![
        Span::styled(" TUDOKU ", badge(ACCENT)),
        Span::raw("  "),
        Span::styled(" LIBRARY ", badge(GREEN)),
        rest,
    ])
}

/// Every technique in a column per level, `at` highlighted, and its gist.
/// Each name is a click target that opens its lesson.
fn draw_map(frame: &mut Frame, area: Rect, at: Technique, covered: bool, hits: &mut Vec<Hit>) {
    // The header sits where the lesson screen's does, so it stays put
    // going between the two.
    let (ox, _) = board_origin(area);
    let rest = Span::styled(
        "  every strategy the tutor knows, easiest first",
        Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
    );
    let header = Rect::new(ox, layout_top(area), area.right() - ox, 1);
    frame.render_widget(Paragraph::new(badges(rest)), header);
    if covered {
        return;
    }

    let (r, inner) = popup(frame, area, MAP_W, MAP_H, "Techniques library");
    push_hit(hits, r, None);
    let muted = Style::default().fg(MUTED);
    let col_w = usize::from(COL_W);
    let columns = Difficulty::ALL.map(column);
    let longest = columns.iter().map(Vec::len).max().unwrap_or(0);

    let mut lines = vec![
        Line::styled(" arrows/hjkl to move, Enter to open, or click", muted),
        Line::from(""),
    ];
    let mut headings = vec![Span::raw(INDENT)];
    let mut rules = vec![Span::raw(INDENT)];
    for (level, techniques) in Difficulty::ALL.iter().zip(&columns) {
        let name = level.name().to_uppercase();
        let count = format!(" {}", techniques.len());
        let pad = col_w.saturating_sub(name.len() + count.len());
        headings.push(Span::styled(
            name,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
        headings.push(Span::styled(format!("{count}{}", " ".repeat(pad)), muted));
        rules.push(Span::styled(
            "─".repeat(col_w - 2),
            Style::default().fg(LINE),
        ));
        rules.push(Span::raw("  "));
    }
    lines.push(Line::from(headings));
    lines.push(Line::from(rules));
    for row in 0..longest {
        let mut spans = vec![Span::raw(INDENT)];
        for (k, techniques) in columns.iter().enumerate() {
            let Some(&t) = techniques.get(row) else {
                spans.push(Span::raw(" ".repeat(col_w)));
                continue;
            };
            let style = if t == at {
                Style::default()
                    .fg(BRIGHT)
                    .bg(SEL_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(INK)
            };
            spans.push(Span::styled(
                format!("{:<w$}", heading(t), w = col_w - 1),
                style,
            ));
            spans.push(Span::raw(" "));
            let cell = Rect::new(
                inner.x + INDENT.len() as u16 + k as u16 * COL_W,
                inner.y + NAMES_Y + row as u16,
                COL_W - 1,
                1,
            );
            push_hit(hits, cell, Some(Action::LibraryOpen(t)));
        }
        lines.push(Line::from(spans));
    }
    // The gist line and the keys sit at the foot, under the longest column.
    while lines.len() < usize::from(inner.height) - 4 {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw(INDENT),
        Span::styled(
            heading(at),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {}", gist(at)), Style::default().fg(INK)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::styled(" Enter open   Esc back to the menu", muted));
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use crate::app::{Action, App, Overlay};
    use crate::deal::Difficulty;
    use crate::library::Library;
    use crate::logic::Technique;
    use crate::store::Store;
    use crate::tutor::lessons::gist;
    use crate::ui::tests::{W, draw, text};
    use crate::ui::tutor::heading;
    use crate::ui::{MIN_H, MIN_W, hit_at};

    fn reading(library: Library) -> App {
        let mut app = App::new(Store::none());
        app.library = Some(library);
        app
    }

    #[test]
    fn the_map_shows_every_technique_by_level_at_the_minimum_size() {
        let app = reading(Library::Map {
            at: Technique::Skyscraper,
        });
        let (lines, hits) = draw(&app, MIN_W, MIN_H);
        let screen = text(&lines);
        for (d, n) in Difficulty::ALL.iter().zip([3, 6, 12, 11, 7]) {
            let heading = format!("{} {n}", d.name().to_uppercase());
            assert!(screen.contains(&heading), "{screen}");
        }
        for &t in Technique::ALL {
            assert!(screen.contains(&heading(t)), "{t:?} missing");
            let opens = hits
                .iter()
                .any(|h| h.action == Some(Action::LibraryOpen(t)));
            assert!(opens, "{t:?}");
        }
        assert!(
            screen.contains("Unique Rectangle 1  Finned Swordfish"),
            "the longest names keep a gap: {screen}"
        );
        let line = format!("Skyscraper  {}", gist(Technique::Skyscraper));
        assert!(screen.contains(&line), "{screen}");
        assert!(screen.contains("Esc back to the menu"), "{screen}");
        assert!(screen.contains("LIBRARY"));
        assert!(!screen.contains("New game"), "the title menu is gone");
    }

    #[test]
    fn help_over_the_map_hides_the_map() {
        // Like the title menu: a popup under Help would only peek out.
        let mut app = reading(Library::Map {
            at: Technique::Skyscraper,
        });
        app.overlays.push(Overlay::Help);
        let (lines, hits) = draw(&app, MIN_W, MIN_H);
        let screen = text(&lines);
        assert!(screen.contains("How to play"), "{screen}");
        assert!(!screen.contains("Techniques library"), "{screen}");
        assert!(!screen.contains("Skyscraper"), "{screen}");
        let opens = |h: &&crate::ui::Hit| matches!(h.action, Some(Action::LibraryOpen(_)));
        assert!(!hits.iter().any(|h| opens(&h)));
    }

    #[test]
    fn every_gist_fits_the_gist_line_at_the_minimum_size() {
        for &t in Technique::ALL {
            let (lines, _) = draw(&reading(Library::Map { at: t }), MIN_W, MIN_H);
            let want = format!("{}  {}", heading(t), gist(t));
            assert!(text(&lines).contains(&want), "{t:?}: {want}");
        }
    }

    #[test]
    fn clicks_open_only_the_technique_under_them() {
        let app = reading(Library::Map {
            at: Technique::FullHouse,
        });
        let (lines, hits) = draw(&app, W, 44);
        // Screen column of `needle` in `line` (the border before it is multi-byte).
        let col =
            |line: &str, needle: &str| line[..line.find(needle).unwrap()].chars().count() as u16;
        let y = lines.iter().position(|l| l.contains("Skyscraper")).unwrap();
        let x = col(&lines[y], "Skyscraper");
        let sky = Some(Action::LibraryOpen(Technique::Skyscraper));
        assert_eq!(hit_at(&hits, x, y as u16), sky);
        // Under Easy's three, beside Skyscraper: an empty slot.
        let easy_x = col(&lines[y - 3], "Full house");
        assert_eq!(hit_at(&hits, easy_x, y as u16), None);
        // The popup's border and the screen outside it.
        let top = lines
            .iter()
            .position(|l| l.contains("╭ Techniques library"))
            .unwrap();
        assert_eq!(hit_at(&hits, easy_x, top as u16), None);
        assert_eq!(hit_at(&hits, 0, 0), None);
    }

    #[test]
    fn a_lesson_is_the_tutors_screen_with_library_keys() {
        let t = Technique::Skyscraper;
        let app = reading(Library::Lesson { at: t, stage: 1 });
        let (lines, hits) = draw(&app, MIN_W, MIN_H);
        let screen = text(&lines);
        assert!(
            lines[0].contains(" LIBRARY   Hard · 13 of 39"),
            "{}",
            lines[0]
        );
        assert!(
            lines[1].contains("Skyscraper  ·  step 2 of 3"),
            "{}",
            lines[1]
        );
        assert!(screen.contains("╭ Skyscraper ─"));
        assert!(
            !screen.contains("EXAMPLE") && !screen.contains("show me"),
            "{screen}"
        );
        for want in [
            Action::ExampleStep(1),
            Action::LibraryTurn(1),
            Action::LibraryBack,
        ] {
            assert!(hits.iter().any(|h| h.action == Some(want)), "{want:?}");
        }
        // The ↑ of "↑ ↓" turns back, like ← steps back.
        let y = lines
            .iter()
            .position(|l| l.contains("↑ ↓  other techniques"))
            .unwrap();
        let x = lines[y].chars().position(|c| c == '↑').unwrap() as u16;
        let y = y as u16;
        assert_eq!(hit_at(&hits, x, y), Some(Action::LibraryTurn(-1)));
        assert_eq!(hit_at(&hits, x + 2, y), Some(Action::LibraryTurn(1)));
    }

    #[test]
    fn a_lesson_draws_the_same_board_and_pane_as_the_tutor() {
        // Same example and stage through the tutor and through the library:
        // every row must match, except the header's badge line and the key lines.
        let t = Technique::XYWing;
        let library = draw(&reading(Library::Lesson { at: t, stage: 2 }), W, 50).0;
        let tutor = draw(&crate::ui::tutor::tests::app_explaining(t, 2), W, 50).0;
        let skip = [
            "TUDOKU",
            "show me",
            "back to solving",
            "other techniques",
            "back to the list",
        ];
        for (k, (a, b)) in library.iter().zip(&tutor).enumerate() {
            if !skip.iter().any(|w| a.contains(w) || b.contains(w)) {
                assert_eq!(a, b, "row {k} differs");
            }
        }
    }
}
