//! The tutor: reads the player's board, checks it, and names the easiest
//! strategy that makes progress.

pub mod examples;
pub mod lessons;

use crate::game::Game;
use crate::logic::{Grid, Step};
use crate::sudoku::{Board, CELLS, candidates_mask};

/// How the player's notes are read, for the whole board at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteMode {
    /// Every empty cell has notes: they are the candidates.
    Full,
    /// Some empty cell has none: the notes are pair marks, not candidates,
    /// so they are set aside and candidates come from the placed digits.
    PairMarks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advice {
    /// This cell holds a wrong digit.
    WrongDigit(usize),
    /// This cell's notes cross out the digit that goes there.
    WrongNotes(usize),
    Step(Step),
    Stuck,
}

/// The board as the tutor reasons about it.
pub fn read(values: &Board, notes: &[u16; CELLS]) -> (NoteMode, Grid) {
    let full = (0..CELLS).all(|i| values[i] != 0 || notes[i] != 0);
    let cands = std::array::from_fn(|i| {
        let open = candidates_mask(values, i);
        if full { notes[i] & open } else { open }
    });
    let mode = if full {
        NoteMode::Full
    } else {
        NoteMode::PairMarks
    };
    (mode, Grid::with_candidates(values, &cands))
}

/// Check the board, then name the easiest next step.
pub fn consult(game: &Game) -> (NoteMode, Advice) {
    let (mode, grid) = read(&game.values, &game.notes);
    let advice = if let Some(i) =
        (0..CELLS).find(|&i| game.values[i] != 0 && game.values[i] != game.solution[i])
    {
        Advice::WrongDigit(i)
    } else if let Some(i) = (mode == NoteMode::Full)
        .then(|| {
            (0..CELLS)
                .find(|&i| game.values[i] == 0 && game.notes[i] & (1 << game.solution[i]) == 0)
        })
        .flatten()
    {
        Advice::WrongNotes(i)
    } else {
        grid.next_step().map_or(Advice::Stuck, Advice::Step)
    };
    (mode, advice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deal::Difficulty;
    use crate::logic::{Effect, Technique};
    use crate::sudoku::{parse_board, solve_one, test_board};

    fn test_game() -> Game {
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

    fn game_from(puzzle: &str) -> Game {
        let p = parse_board(puzzle).unwrap();
        Game::new(Difficulty::Hard, p, solve_one(&p).unwrap())
    }

    #[test]
    fn full_mode_needs_notes_in_every_empty_cell() {
        let mut g = test_game();
        assert_eq!(consult(&g).0, NoteMode::PairMarks);
        full_notes(&mut g);
        assert_eq!(consult(&g).0, NoteMode::Full);
        let c = (0..CELLS).find(|&i| g.values[i] == 0).unwrap();
        g.notes[c] = 0;
        assert_eq!(consult(&g).0, NoteMode::PairMarks);
    }

    #[test]
    fn pair_marks_are_never_read_as_candidates() {
        let mut g = test_game();
        let marked = (0..CELLS)
            .find(|&i| g.values[i] == 0 && g.solution[i] != 1)
            .unwrap();
        g.notes[marked] = 1 << 1;
        let (mode, advice) = consult(&g);
        assert_eq!(mode, NoteMode::PairMarks);
        assert!(!matches!(advice, Advice::WrongNotes(_)));
        if let Advice::Step(s) = advice {
            assert_ne!(
                s.effect,
                Effect::Place {
                    cell: marked,
                    digit: 1
                }
            );
        }
    }

    #[test]
    fn a_wrong_digit_comes_first() {
        let mut g = test_game();
        full_notes(&mut g);
        let mut empties = (0..CELLS).filter(|&i| g.values[i] == 0);
        let (wrong, noted) = (empties.next().unwrap(), empties.next().unwrap());
        g.values[wrong] = g.solution[wrong] % 9 + 1;
        g.notes[wrong] = 0;
        g.notes[noted] &= !(1 << g.solution[noted]);
        assert_eq!(consult(&g).1, Advice::WrongDigit(wrong));
    }

    #[test]
    fn wrong_notes_are_caught_in_full_mode() {
        let mut g = test_game();
        full_notes(&mut g);
        let c = (0..CELLS).filter(|&i| g.values[i] == 0).nth(5).unwrap();
        // Leave the cell another note so the board still reads as full.
        g.notes[c] = 1 << (g.solution[c] % 9 + 1);
        assert_eq!(consult(&g), (NoteMode::Full, Advice::WrongNotes(c)));
    }

    #[test]
    fn the_tutor_moves_on_once_you_make_the_elimination() {
        let mut g = game_from(
            "087000500003000060150600000200500003000089002000073900000430000900005706040007000",
        );
        full_notes(&mut g);
        let step = loop {
            match consult(&g).1 {
                Advice::Step(s) => match s.effect {
                    Effect::Place { .. } => assert!(g.apply_step(&s, NoteMode::Full)),
                    Effect::Eliminate(_) => break s,
                },
                other => panic!("expected a step, got {other:?}"),
            }
        };
        let Effect::Eliminate(removals) = &step.effect else {
            unreachable!()
        };
        for &(cell, mask) in removals {
            g.notes[cell] &= !mask;
        }
        let (mode, next) = consult(&g);
        assert_eq!(mode, NoteMode::Full);
        assert_ne!(next, Advice::Step(step));
    }

    #[test]
    fn easiest_step_first_in_pair_mark_mode() {
        let g = test_game();
        let (mode, advice) = consult(&g);
        assert_eq!(mode, NoteMode::PairMarks);
        let Advice::Step(s) = advice else {
            panic!("expected a step, got {advice:?}")
        };
        assert!(s.technique <= Technique::HiddenSingle);
    }

    #[test]
    fn stuck_when_no_strategy_applies() {
        let p = parse_board(
            "800000000003600000070090200050007000000045700000100030001000068008500010090000400",
        )
        .unwrap();
        let mut grid = Grid::new(&p);
        while let Some(step) = grid.next_step() {
            grid.apply(&step);
        }
        assert!(!grid.is_solved());
        let mut g = Game::new(Difficulty::Master, p, solve_one(&p).unwrap());
        g.values = *grid.values();
        g.notes = *grid.cands();
        assert_eq!(consult(&g), (NoteMode::Full, Advice::Stuck));
    }
}
