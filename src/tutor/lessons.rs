//! What each strategy is, in a few plain steps: the text beside the
//! worked example. Each step is one short sentence in plain words, and
//! the roles name what the chip colours on the example stand for.

use crate::logic::Technique;

/// A strategy's explanation: 3 to 5 short steps, numbered on screen.
pub struct Lesson {
    pub steps: &'static [&'static str],
    /// What the `Role::Key` and `Role::Other` chips on the example mean,
    /// for the legend; `""` for a role the strategy doesn't mark.
    pub roles: (&'static str, &'static str),
}

pub fn lesson(t: Technique) -> Lesson {
    use Technique::*;
    match t {
        FullHouse => Lesson {
            steps: &[
                "Find a row, column or box with just one empty cell.",
                "Only one digit is missing from it.",
                "That digit goes in the empty cell.",
            ],
            roles: ("", ""),
        },
        NakedSingle => Lesson {
            steps: &[
                "Pick an empty cell.",
                "Rule out every digit already in its row, column or box.",
                "If only one digit is left, it goes there.",
            ],
            roles: ("", ""),
        },
        HiddenSingle => Lesson {
            steps: &[
                "Pick a row, column or box, and a digit it still needs.",
                "Rule out each empty cell whose row, column or box already has that digit.",
                "If only one cell is left, the digit goes there.",
            ],
            roles: ("", ""),
        },
        Pointing => Lesson {
            steps: &[
                "Pick a box, and find the cells where a digit fits in it.",
                "If they all sit in one row or column, the digit must go in one of those cells.",
                "So the rest of that row or column can't be the digit.",
            ],
            roles: ("where it fits", ""),
        },
        Claiming => Lesson {
            steps: &[
                "Pick a row or column, and find the cells where a digit fits in it.",
                "If they all sit in one box, the digit must go in one of those cells.",
                "So the rest of that box can't be the digit.",
            ],
            roles: ("where it fits", ""),
        },
        NakedPair => Lesson {
            steps: &[
                "Find two cells in a row, column or box where only the same two digits fit.",
                "Those two digits must fill those two cells, one each.",
                "So no other cell there can be either digit.",
            ],
            roles: ("the pair", ""),
        },
        HiddenPair => Lesson {
            steps: &[
                "In a row, column or box, find two digits that fit in only the same two cells.",
                "Those two cells must hold those two digits, one each.",
                "So they can't hold any other digit.",
            ],
            roles: ("the pair", ""),
        },
        NakedTriple => Lesson {
            steps: &[
                "Find three cells in a row, column or box where only three digits fit between them.",
                "Some of the cells may fit just two of them.",
                "Those three digits must fill those three cells.",
                "So no other cell there can be any of them.",
            ],
            roles: ("the triple", ""),
        },
        HiddenTriple => Lesson {
            steps: &[
                "In a row, column or box, find three digits that fit in only the same three cells.",
                "Some of the digits may fit in just two of them.",
                "Those cells must hold those three digits.",
                "So they can't hold any other digit.",
            ],
            roles: ("the triple", ""),
        },
        NakedQuad => Lesson {
            steps: &[
                "Find four cells in a row, column or box where only four digits fit between them.",
                "Some of the cells may fit just two or three of them.",
                "Those four digits must fill those four cells.",
                "So no other cell there can be any of them.",
            ],
            roles: ("the quad", ""),
        },
        HiddenQuad => Lesson {
            steps: &[
                "In a row, column or box, find four digits that fit in only the same four cells.",
                "Some of the digits may fit in just two or three of them.",
                "Those cells must hold those four digits.",
                "So they can't hold any other digit.",
            ],
            roles: ("the quad", ""),
        },
        XWing => Lesson {
            steps: &[
                "Find two rows where a digit fits in only two cells each.",
                "If those cells line up in the same two columns, they make a rectangle.",
                "The digit takes opposite corners, so each column gets it in one of these rows.",
                "So the rest of those columns can't be the digit.",
                "It works the same with rows and columns swapped.",
            ],
            roles: ("corners", ""),
        },
        Skyscraper => Lesson {
            steps: &[
                "Find two rows or two columns where a digit fits in only two cells each.",
                "One end of each lines up across them: the base. They can't both be the digit.",
                "So at least one of the other ends, the tops, is the digit.",
                "Any cell that sees both tops can't be it.",
            ],
            roles: ("top", "base"),
        },
        TwoStringKite => Lesson {
            steps: &[
                "Find a row and a column where a digit fits in only two cells each.",
                "One end of each sits in the same box: the knot. They can't both be the digit.",
                "So one of the two far ends is the digit.",
                "Any cell that sees both far ends can't be it.",
            ],
            roles: ("end", "knot"),
        },
        Crane => Lesson {
            steps: &[
                "Find a box and a row or column outside it where a digit fits in only two cells each.",
                "One end of each shares a line with the other: the link. They can't both be the digit.",
                "So one of the two far ends is the digit.",
                "Any cell that sees both far ends can't be it.",
            ],
            roles: ("end", "link"),
        },
        EmptyRectangle => Lesson {
            steps: &[
                "In a box, find a digit whose spots all lie in one row and one column of it.",
                "Find a row or column outside the box where it fits in only two cells, one lined up with that box row or column.",
                "If that cell is the digit, the box's digit must be in the box's other line.",
                "If the other end is the digit instead, it sees the cell outside the box where it meets that other line. Either way that cell can't be it.",
            ],
            roles: ("pair", "box"),
        },
        XYWing => Lesson {
            steps: &[
                "Find a cell where only two digits fit: the pivot.",
                "Find two cells it sees where only two digits fit: the pincers.",
                "Each shares a different digit with the pivot, and both share a third.",
                "Whichever digit the pivot is, one pincer is the third digit.",
                "So any cell that sees both pincers can't be it.",
            ],
            roles: ("third digit", "pivot's digits"),
        },
        Swordfish => Lesson {
            steps: &[
                "Find three rows where a digit fits in only two or three cells each.",
                "If those cells all fall in the same three columns, the rows' three digits must take one column each.",
                "So the rest of those columns can't be the digit.",
                "It works the same with rows and columns swapped.",
            ],
            roles: ("where it fits", ""),
        },
        XYZWing => Lesson {
            steps: &[
                "Find a cell where only three digits fit: the pivot.",
                "Find two cells it sees where only two of its digits fit: the pincers.",
                "The pincers share just one digit.",
                "Whatever the pivot is, it or a pincer is that digit.",
                "So any cell that sees all three can't be it.",
            ],
            roles: ("shared digit", "other digits"),
        },
        Jellyfish => Lesson {
            steps: &[
                "Find four rows where a digit fits in only two to four cells each.",
                "If those cells all fall in the same four columns, the rows' four digits must take one column each.",
                "So the rest of those columns can't be the digit.",
                "It works the same with rows and columns swapped.",
            ],
            roles: ("where it fits", ""),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::Role;
    use crate::tutor::examples::example;
    use crate::ui::BOARD_H;
    use crate::ui::tutor::{TEXT_W, wrap};

    #[test]
    fn lesson_text_fits_the_pane() {
        for &t in Technique::ALL {
            let lesson = lesson(t);
            let n = lesson.steps.len();
            assert!((3..=5).contains(&n), "{t:?} has {n} steps");
            let mut rows = 0;
            for (k, step) in lesson.steps.iter().enumerate() {
                let lines = wrap(&format!("{} {step}", k + 1), TEXT_W);
                assert!(
                    lines.iter().all(|l| l.chars().count() <= TEXT_W),
                    "{t:?}: {lines:?}"
                );
                rows += lines.len() + 1; // and a blank line after it
            }
            for role in [lesson.roles.0, lesson.roles.1] {
                // A chip, a space and the role's name.
                assert!(3 + 1 + role.chars().count() <= TEXT_W, "{t:?}: {role}");
            }
            let legend = 3;
            let keys = 3;
            let spacing = 2;
            assert!(
                rows + legend + keys + spacing <= usize::from(BOARD_H - 3),
                "{t:?} runs {rows} rows"
            );
        }
    }

    #[test]
    fn the_legend_names_the_chips_the_example_draws() {
        for &t in Technique::ALL {
            let roles = lesson(t).roles;
            let step = example(t).unwrap_or_else(|| panic!("{t:?}")).step;
            let has = |role| step.pattern.marks.iter().any(|m| m.role == role);
            assert_eq!(!roles.0.is_empty(), has(Role::Key), "{t:?}: key chips");
            assert_eq!(!roles.1.is_empty(), has(Role::Other), "{t:?}: other chips");
        }
    }
}
