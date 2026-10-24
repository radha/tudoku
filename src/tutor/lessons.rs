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
        WWing => Lesson {
            steps: &[
                "Find two cells that don't see each other, with only the same two digits: the ends.",
                "Pick one digit, the bridge. Find a house where it fits in just two cells, one seeing each end.",
                "If an end is the bridge, the house cell it sees isn't, so the other is, and the far end isn't.",
                "So one end is the other digit: if neither is the bridge, both are.",
                "Any cell that sees both ends can't be the other digit.",
            ],
            roles: ("removed digit", "bridge"),
        },
        SimpleColouring => Lesson {
            steps: &[
                "Pairs: houses where a digit fits only two cells. Pairs sharing a cell make a chain of 3+ cells.",
                "Each pair holds it once, so colour the chain A, B, A, B: one colour is all the digit, the other none.",
                "Trap: a cell outside the chain seeing an A and a B sees the digit either way. So it isn't the digit.",
                "Wrap: two cells of one colour seeing each other can't both be it. That colour is false: clear it.",
            ],
            roles: ("colour A", "colour B"),
        },
        UniqueRectangle => Lesson {
            steps: &[
                "Four rectangle corners in two boxes fit the same two digits. Held only by them, they could swap: two solutions. A puzzle has one.",
                "Floors fit only them; roofs more. Type 1: three floors, so the roof isn't either digit. Type 2: each roof has one extra digit, the same, so one roof is it; cells seeing both can't be it.",
                "Type 4: a floor digit fits only in the roofs, in a house they share. So one is it, and neither is the other digit.",
            ],
            roles: ("roof", "floor"),
        },
        BugPlusOne => Lesson {
            steps: &[
                "Look for a board where every empty cell fits two digits, except one that fits three.",
                "Check every unplaced digit fits twice in each row, column and box, except one of that cell's digits: three times in its row, column and box.",
                "Without that digit in the cell, all would fit twice: any solution could flip every cell to its other digit. A puzzle has one, so the cell is that digit.",
            ],
            roles: ("three-way cell", ""),
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
        FinnedXWing => Lesson {
            steps: &[
                "Pick two rows (or columns) where a digit fits, and two columns (or rows) that each hold some of its cells.",
                "The cells outside those columns (or rows) are fins, all in one box.",
                "If no fin is the digit, it's a plain fish; if one is, the fin box has it.",
                "Either way, a fin-box cell in those columns (or rows) but not in the first rows (or columns) can't be it. (Sashimi: a first line has under two cells there.)",
            ],
            roles: ("fish cells", "fins"),
        },
        FinnedSwordfish => Lesson {
            steps: &[
                "Pick three rows (or columns) where a digit fits, and three columns (or rows) that each hold some of its cells.",
                "The cells outside those columns (or rows) are fins, all in one box.",
                "If no fin is the digit, it's a plain fish; if one is, the fin box has it.",
                "Either way, a fin-box cell in those columns (or rows) but not in the first rows (or columns) can't be it. (Sashimi: a first line has under two cells there.)",
            ],
            roles: ("fish cells", "fins"),
        },
        WXYZWing => Lesson {
            steps: &[
                "Take a pivot and three cells it sees, each fitting two or more digits.",
                "Together they fit exactly four digits.",
                "One digit is loose and fits in two or more of them. For every other digit, its cells among them all see each other.",
                "So each other digit fills at most one: three can't fill four cells. One cell is the loose digit.",
                "Any other cell that sees all the loose digit's cells can't be it.",
            ],
            roles: ("loose digit", "locked digits"),
        },
        FinnedJellyfish => Lesson {
            steps: &[
                "Pick four rows (or columns) where a digit fits, and four columns (or rows) that each hold some of its cells.",
                "The cells outside those columns (or rows) are fins, all in one box.",
                "If no fin is the digit, it's a plain fish; if one is, the fin box has it.",
                "Either way, a fin-box cell in those columns (or rows) but not in the first rows (or columns) can't be it. (Sashimi: a first line has under two cells there.)",
            ],
            roles: ("fish cells", "fins"),
        },
        RemotePair => Lesson {
            steps: &[
                "Find four or more cells where only the same two digits fit, joined into one group by seeing each other.",
                "Two of them that see each other can't match: one is each digit. So colour the group A and B, with cells that see each other in different colours.",
                "Then all the As are one digit and all the Bs the other.",
                "A cell outside the group that sees an A and a B sees both digits, so it can't be either.",
            ],
            roles: ("colour A", "colour B"),
        },
        XChain => Lesson {
            steps: &[
                "Strong link: a house where a digit fits only two cells. If one isn't it, the other is.",
                "Weak link: two cells that see each other. If one is it, the other isn't.",
                "Chain them strong, weak, strong ... ending strong, no cell twice.",
                "If either end isn't the digit, each link forces the next, and the far end is. So one end is.",
                "Any other cell that sees both ends can't be the digit.",
            ],
            roles: ("ends", "chain"),
        },
        XYChain => Lesson {
            steps: &[
                "Find cells that fit only two digits, each seeing the next in a path.",
                "If the first isn't its start digit, it's its other. The next cell must also fit that one, and can't be it, so it's its other. And so on.",
                "So the last cell is its end digit: one end is true.",
                "Same digit at both ends: a cell seeing both can't be it. Else, if the end cells see each other, neither can be the other's digit, or both would be false.",
            ],
            roles: ("ends", "chain"),
        },
        Aic => Lesson {
            steps: &[
                "Strong link: a house's only two spots for a digit, or a cell's only two digits. One is true.",
                "Weak link: a digit in two cells that see each other, or two digits in a cell. At most one is true.",
                "Chain them strong, weak ... strong. If one end is false, the other is forced true.",
                "Same digit at both ends: a cell seeing both can't be it. Else, if the end cells see each other, neither can be the other's digit, or both would be false.",
            ],
            roles: ("ends", "chain"),
        },
        AlsXz => Lesson {
            steps: &[
                "ALS: 1-4 cells in one house fitting one more digit than cells. No digit repeats, so if one is ruled out, each other one fills a cell.",
                "Find two ALSs, no shared cell, and a link digit X both fit: every X in one sees every X in the other. They can't both hold X: at least one lacks it, so holds all its other digits.",
                "So a digit Z both fit (not X) is in one of them. A cell outside both seeing all their Zs can't be Z.",
            ],
            roles: ("removed digit", "link digit"),
        },
        AlsXyWing => Lesson {
            steps: &[
                "ALS: 1-4 cells in one house fitting one more digit than cells. No digit repeats, so if one is ruled out, each other one fills a cell.",
                "Find ALSs A, B, C, no shared cells, and links X in A and C, Y (not X) in B and C: every X in A sees every X in C, every Y in B every Y in C.",
                "C misses one digit at most, so holds X or Y: A lacks X or B lacks Y, and holds its others. So a Z in A and B (not X or Y) is in one. Cells outside seeing all their Zs can't be Z.",
            ],
            roles: ("removed digit", "link digit"),
        },
        SueDeCoq => Lesson {
            steps: &[
                "Where a box meets a line (row or column), take 2-3 empty cells fitting 2 digits more than cells.",
                "Add a line cell outside the box and a box cell outside the line, each fitting only two digits, both from those, none shared.",
                "Any two share a house or no digit, so all differ; with as many digits, each fills one. So the line cell's digits stay in the line, the box cell's in the box, the rest in both: other cells of each house lose them.",
            ],
            roles: ("intersection", "helpers"),
        },
        DeathBlossom => Lesson {
            steps: &[
                "Stem: a cell fitting 2 or 3 digits. Each stem digit has a petal: an ALS (1-3 cells in one house fitting one more digit than cells) holding it, whose cells fitting it all see the stem. No shared cells.",
                "The stem is one digit, so that digit's petal lacks it, and (no digit repeating) holds all its others.",
                "So a digit Z every petal fits, and the stem doesn't, is in a petal. A cell outside seeing all petal Zs can't be Z.",
            ],
            roles: ("removed digit", "stem & petals"),
        },
    }
}

/// A strategy in one line, for the library map's gist line beside its
/// name. Like the lessons: general terms (no cells, no digits), in the
/// words that strategy's lesson and legend use.
#[allow(dead_code)] // drawn on the library map (Task 5)
pub fn gist(t: Technique) -> &'static str {
    use Technique::*;
    match t {
        FullHouse => "the one empty cell left in a row, column or box",
        NakedSingle => "a cell where only one digit fits",
        HiddenSingle => "a digit that fits only one cell in a row, column or box",
        Pointing => "a box's spots for a digit all in one row or column",
        Claiming => "a row or column's spots for a digit all in one box",
        NakedPair => "two cells where only the same two digits fit",
        HiddenPair => "two digits that fit only the same two cells",
        NakedTriple => "three cells where only three digits fit",
        HiddenTriple => "three digits that fit only the same three cells",
        NakedQuad => "four cells where only four digits fit",
        HiddenQuad => "four digits that fit only the same four cells",
        XWing => "a digit's two spots in two rows make a rectangle",
        Skyscraper => "two rows (or columns), two spots each, with a shared base",
        TwoStringKite => "a row and a column, two spots each, tied at a knot",
        Crane => "a box and a line, two spots each, joined by a link",
        EmptyRectangle => "a box's spots in one row and column, and a pair outside",
        XYWing => "a two-digit pivot and two pincers sharing a third digit",
        Swordfish => "an X-Wing on three rows and three columns",
        XYZWing => "an XY-Wing whose pivot fits three digits",
        WWing => "two matching two-digit ends joined by a bridge",
        FinnedXWing => "an X-Wing with fins in one box",
        RemotePair => "cells fitting the same two digits, coloured A and B",
        SimpleColouring => "one digit's pairs chained and coloured A and B",
        UniqueRectangle => "four corners that could swap would mean two solutions",
        BugPlusOne => "every empty cell fits two digits but one fits three",
        Jellyfish => "an X-Wing on four rows and four columns",
        FinnedSwordfish => "a Swordfish with fins in one box",
        WXYZWing => "a pivot and three cells fitting four digits, one loose",
        FinnedJellyfish => "a Jellyfish with fins in one box",
        XChain => "one digit's strong and weak links, chained",
        XYChain => "a path of cells that each fit only two digits",
        Aic => "strong and weak links across cells and digits",
        AlsXz => "two ALSs joined by a link digit",
        AlsXyWing => "three ALSs joined by two link digits",
        SueDeCoq => "where a box meets a line, digits split between them",
        DeathBlossom => "a stem cell with a petal for each of its digits",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::Role;
    use crate::tutor::examples::example;
    use crate::ui::BOARD_H;
    use crate::ui::tutor::{TEXT_W, wrap};

    /// The longest gist: it shares the map's gist line with the name.
    const GIST_MAX: usize = 60;

    #[test]
    fn every_technique_has_a_short_general_gist() {
        for &t in Technique::ALL {
            let g = gist(t);
            assert!(!g.is_empty(), "{t:?}");
            assert!(g.chars().count() <= GIST_MAX, "{t:?}: {g}");
            assert!(
                !g.contains(|c: char| c.is_ascii_digit()),
                "{t:?} names a digit: {g}"
            );
            assert_eq!(g, g.trim(), "{t:?}");
            assert!(
                !g.ends_with('.'),
                "{t:?}: a gist is a phrase, not a sentence"
            );
        }
    }

    #[test]
    fn every_technique_has_a_worked_example() {
        for &t in Technique::ALL {
            assert!(example(t).is_some(), "{t:?}");
        }
    }

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
