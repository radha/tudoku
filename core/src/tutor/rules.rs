//! When the tutor may act for the player, and what acting does: the
//! rules any front end shares.

use super::{Advice, NoteMode, consult, examples};
use crate::game::Game;
use crate::logic::Effect;
use crate::sudoku::CELLS;

/// With notes still to fill in, there's more to try alone first. A wrong
/// digit is always worth hearing about at once.
pub fn starts_with_try_first(mode: NoteMode, advice: &Advice) -> bool {
    mode == NoteMode::PairMarks && matches!(advice, Advice::Step(_) | Advice::Stuck)
}

/// Whether the advice is a step whose technique has a worked example.
pub fn has_example(advice: &Advice) -> bool {
    matches!(advice, Advice::Step(step) if examples::example(step.technique).is_some())
}

/// Whether "do it for me" can act: place a single or clear the eliminated
/// notes once the step is `shown`, or place one digit when no strategy
/// applies. Eliminations need full notes to clear.
pub fn can_apply(mode: NoteMode, advice: &Advice, shown: bool) -> bool {
    match advice {
        Advice::Stuck => true,
        Advice::Step(step) => {
            shown
                && match step.effect {
                    Effect::Place { .. } => true,
                    Effect::Eliminate(_) => mode == NoteMode::Full,
                }
        }
        Advice::WrongDigit(_) | Advice::WrongNotes(_) => false,
    }
}

/// Do what the advice says. When no strategy applies, the answer goes in
/// the selected cell if it is empty, else the first empty one. False if
/// there was nothing to do.
pub fn apply(game: &mut Game, mode: NoteMode, advice: &Advice) -> bool {
    match advice {
        Advice::Step(step) => game.apply_step(step, mode),
        Advice::Stuck => {
            let s = game.selected_idx();
            let open = if game.values[s] == 0 {
                Some(s)
            } else {
                (0..CELLS).find(|&i| game.values[i] == 0)
            };
            if let Some(cell) = open {
                game.place_answer(cell);
            }
            true
        }
        Advice::WrongDigit(_) | Advice::WrongNotes(_) => false,
    }
}

/// Whether the board no longer reads the way it did when the advice was given.
pub fn is_stale(game: &Game, mode: NoteMode, advice: &Advice) -> bool {
    consult(game) != (mode, advice.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deal::Difficulty;
    use crate::logic::{Grid, Pattern, Step, Technique};
    use crate::sudoku::{candidates_mask, test_board};

    fn game() -> Game {
        let (p, s) = test_board();
        Game::new(Difficulty::Easy, p, s)
    }

    fn full_notes(g: &mut Game) {
        for i in 0..CELLS {
            if g.values[i] == 0 {
                g.notes[i] = candidates_mask(&g.values, i);
            }
        }
    }

    fn eliminate() -> Advice {
        Advice::Step(Step {
            technique: Technique::Skyscraper,
            effect: Effect::Eliminate(vec![(0, 1 << 1)]),
            pattern: Pattern::default(),
            why: String::new(),
        })
    }

    fn place() -> Advice {
        let (p, _) = test_board();
        Advice::Step(Grid::new(&p).next_step().unwrap())
    }

    #[test]
    fn try_first_only_with_pair_marks_and_a_step_or_stuck() {
        assert!(starts_with_try_first(NoteMode::PairMarks, &Advice::Stuck));
        assert!(starts_with_try_first(NoteMode::PairMarks, &place()));
        assert!(!starts_with_try_first(NoteMode::Full, &Advice::Stuck));
        assert!(!starts_with_try_first(
            NoteMode::PairMarks,
            &Advice::WrongDigit(3)
        ));
    }

    #[test]
    fn can_apply_matrix() {
        for mode in [NoteMode::Full, NoteMode::PairMarks] {
            for shown in [false, true] {
                assert!(can_apply(mode, &Advice::Stuck, shown));
                assert!(!can_apply(mode, &Advice::WrongDigit(0), shown));
                assert!(!can_apply(mode, &Advice::WrongNotes(0), shown));
            }
            assert!(!can_apply(mode, &place(), false));
            assert!(can_apply(mode, &place(), true));
        }
        assert!(!can_apply(NoteMode::Full, &eliminate(), false));
        assert!(can_apply(NoteMode::Full, &eliminate(), true));
        assert!(!can_apply(NoteMode::PairMarks, &eliminate(), true));
    }

    #[test]
    fn has_example_follows_the_technique() {
        assert!(has_example(&eliminate()));
        assert!(!has_example(&Advice::Stuck));
    }

    #[test]
    fn apply_stuck_places_in_selected_empty_cell_else_first_empty() {
        let mut g = game();
        let given = (0..CELLS).find(|&i| g.values[i] != 0).unwrap();
        let first = (0..CELLS).find(|&i| g.values[i] == 0).unwrap();
        g.set_selected(given / 9, given % 9);
        assert!(apply(&mut g, NoteMode::PairMarks, &Advice::Stuck));
        assert_eq!(g.values[first], g.solution[first]);

        let mut g = game();
        let other = (0..CELLS).filter(|&i| g.values[i] == 0).nth(4).unwrap();
        g.set_selected(other / 9, other % 9);
        assert!(apply(&mut g, NoteMode::PairMarks, &Advice::Stuck));
        assert_eq!(g.values[other], g.solution[other]);
        assert_eq!(g.values[first], 0);
    }

    #[test]
    fn apply_does_nothing_for_wrong_digit_or_notes() {
        let mut g = game();
        assert!(!apply(&mut g, NoteMode::Full, &Advice::WrongDigit(2)));
        assert!(!apply(&mut g, NoteMode::Full, &Advice::WrongNotes(2)));
    }

    #[test]
    fn stale_when_board_changes_the_advice() {
        let mut g = game();
        full_notes(&mut g);
        let (mode, advice) = consult(&g);
        assert!(!is_stale(&g, mode, &advice));
        let cell = (0..CELLS).find(|&i| g.values[i] == 0).unwrap();
        g.values[cell] = g.solution[cell] % 9 + 1;
        assert!(is_stale(&g, mode, &advice));
    }
}
