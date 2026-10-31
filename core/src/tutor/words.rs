//! The tutor's wording, free of any screen's styling.

use crate::logic::{Effect, Role, Step, digit_list, rc};
use crate::tutor::lessons::Lesson;

pub const PAIR_MARKS: &str = "Going by placed digits — your notes aren't filled in yet.";
pub const TRY_FIRST: &str = "Try first.";
pub const TRY_FIRST_DETAIL: &str = "Note every digit that still fits in each empty cell, then look again: \
     most strategies need the full set of candidates.";
pub const TRY_FIRST_COST: &str = "Naming one costs the puzzle its best time.";
pub const STUCK: &str = "Nothing in my book applies here.";

pub fn wrong_digit(cell: usize) -> String {
    format!("{} is wrong: clear it first.", rc(cell))
}

pub fn wrong_notes(cell: usize) -> String {
    format!(
        "Your notes in {} cross out something that still fits.",
        rc(cell)
    )
}

/// What a step does, by cell name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectText {
    /// "r3c4 is 5".
    Placed { cell: String, digit: u8 },
    /// The same one digit out of every cell: "Not 1:" and the cells.
    NotDigit { digit: u8, cells: Vec<String> },
    /// One line per cell: "r1c1: not 2 and 7".
    PerCell(Vec<String>),
}

pub fn effect_text(step: &Step) -> EffectText {
    match &step.effect {
        Effect::Place { cell, digit } => EffectText::Placed {
            cell: rc(*cell),
            digit: *digit,
        },
        Effect::Eliminate(removals) => {
            // The same one digit out of every cell reads as a list of
            // cells, whatever digits the pattern itself is built on.
            let first = removals.first().map_or(0, |&(_, mask)| mask);
            if first.is_power_of_two() && removals.iter().all(|&(_, mask)| mask == first) {
                EffectText::NotDigit {
                    digit: first.trailing_zeros() as u8,
                    cells: removals.iter().map(|&(cell, _)| rc(cell)).collect(),
                }
            } else {
                EffectText::PerCell(
                    removals
                        .iter()
                        .map(|&(cell, mask)| format!("{}: not {}", rc(cell), digit_list(mask)))
                        .collect(),
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendKind {
    Key,
    Other,
    RulesOut,
    Placed,
    Removed,
}

/// One line of the legend for an example's colours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendEntry {
    pub kind: LegendKind,
    /// Bit d is digit d.
    pub digits: u16,
    pub label: String,
}

/// What the example's colours mean: each role the strategy marks, the
/// placed digits it reasons from, then what the step removes or places.
pub fn legend(lesson: &Lesson, step: &Step) -> Vec<LegendEntry> {
    let mut entries = Vec::new();
    for (role, kind, name) in [
        (Role::Key, LegendKind::Key, lesson.roles.0),
        (Role::Other, LegendKind::Other, lesson.roles.1),
    ] {
        let digits = step
            .pattern
            .marks
            .iter()
            .filter(|m| m.role == role)
            .fold(0u16, |mask, m| mask | 1 << m.digit);
        if digits != 0 && !name.is_empty() {
            entries.push(LegendEntry {
                kind,
                digits,
                label: name.to_string(),
            });
        }
    }
    if let (Some(d), false) = (step.pattern.digit, step.pattern.placed.is_empty()) {
        entries.push(LegendEntry {
            kind: LegendKind::RulesOut,
            digits: 1 << d,
            label: "rules cells out".to_string(),
        });
    }
    match &step.effect {
        Effect::Place { digit, .. } => entries.push(LegendEntry {
            kind: LegendKind::Placed,
            digits: 1 << digit,
            label: "placed".to_string(),
        }),
        Effect::Eliminate(removals) => entries.push(LegendEntry {
            kind: LegendKind::Removed,
            digits: removals.iter().fold(0, |all, &(_, mask)| all | mask),
            label: "removed".to_string(),
        }),
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{Pattern, Technique};
    use crate::sudoku::idx;
    use crate::tutor::examples::example;
    use crate::tutor::lessons::lesson;

    fn step(effect: Effect) -> Step {
        Step {
            technique: Technique::Skyscraper,
            effect,
            pattern: Pattern::default(),
            why: String::new(),
        }
    }

    #[test]
    fn wrong_digit_and_notes_read_as_today() {
        assert_eq!(wrong_digit(idx(3, 4)), "r4c5 is wrong: clear it first.");
        assert_eq!(
            wrong_notes(idx(3, 4)),
            "Your notes in r4c5 cross out something that still fits."
        );
    }

    #[test]
    fn effect_text_one_digit_out_of_many_cells_lists_cells() {
        let s = step(Effect::Eliminate(vec![
            (idx(4, 2), 1 << 1),
            (idx(5, 3), 1 << 1),
        ]));
        assert_eq!(
            effect_text(&s),
            EffectText::NotDigit {
                digit: 1,
                cells: vec!["r5c3".into(), "r6c4".into()]
            }
        );
    }

    #[test]
    fn effect_text_mixed_masks_go_cell_by_cell() {
        let s = step(Effect::Eliminate(vec![(idx(0, 0), 1 << 2 | 1 << 7)]));
        assert_eq!(
            effect_text(&s),
            EffectText::PerCell(vec!["r1c1: not 2 and 7".into()])
        );
    }

    #[test]
    fn effect_text_placement() {
        let s = step(Effect::Place {
            cell: idx(2, 3),
            digit: 5,
        });
        assert_eq!(
            effect_text(&s),
            EffectText::Placed {
                cell: "r3c4".into(),
                digit: 5
            }
        );
    }

    #[test]
    fn legend_matches_skyscraper_example() {
        let ex = example(Technique::Skyscraper).unwrap();
        let l = lesson(Technique::Skyscraper);
        let entries = legend(&l, &ex.step);
        assert!(
            entries
                .iter()
                .any(|e| e.kind == LegendKind::Key && e.label == l.roles.0)
        );
        let Effect::Eliminate(removals) = &ex.step.effect else {
            panic!("skyscraper eliminates");
        };
        let mask = removals.iter().fold(0, |all, &(_, m)| all | m);
        assert!(
            entries
                .iter()
                .any(|e| e.kind == LegendKind::Removed && e.digits == mask)
        );
        assert!(entries.iter().all(|e| e.kind != LegendKind::Placed));
    }
}
