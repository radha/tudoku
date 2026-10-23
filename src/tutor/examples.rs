//! Worked examples: one sample board per strategy, with the notes that
//! make the strategy stand out. They are picked ahead of time by
//! `tudoku --find-examples` and kept in `examples.txt`.

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::logic::{Grid, Step, Technique};
use crate::sudoku::{self, Board, CELLS};

/// One line per example: `name|values|candidate masks`, where values are
/// 81 digits (0 for empty) and the masks are 81 three-digit hex numbers,
/// bit `d` for candidate `d`.
const EXAMPLES: &str = include_str!("examples.txt");

/// The search stops looking for a technique once it has seen this many.
const ENOUGH: usize = 25;

/// Strategies an easier one always beats to it, so the solver never names
/// them and their example is a board where they apply. Once no naked pair
/// is left, two cells of a Remote Pair that see each other are their
/// house's only two spots for each digit, so Simple Colouring (or
/// something easier) finds whatever the Remote Pair does.
pub const NEVER_FIRST: &[Technique] = &[Technique::RemotePair];

/// A sample board and the step the strategy finds on it.
pub struct Example {
    pub values: Board,
    pub cands: [u16; CELLS],
    pub step: Step,
}

/// The worked example for `t`, if one has been found.
pub fn example(t: Technique) -> Option<Example> {
    EXAMPLES
        .lines()
        .filter(|line| line.split('|').next().and_then(Technique::from_name) == Some(t))
        .find_map(parse)
}

/// An example from its line, if the line is well formed and the board
/// really shows its strategy. Comment lines name no strategy.
fn parse(line: &str) -> Option<Example> {
    let mut fields = line.split('|');
    let t = Technique::from_name(fields.next()?)?;
    let (values, masks) = (fields.next()?, fields.next()?);
    if fields.next().is_some() || !masks.is_ascii() || masks.len() != 3 * CELLS {
        return None;
    }
    let values: Vec<u8> = values
        .chars()
        .map(|c| c.to_digit(10).map(|d| d as u8))
        .collect::<Option<_>>()?;
    let values: Board = values.try_into().ok()?;
    let cands: Vec<u16> = masks
        .as_bytes()
        .chunks(3)
        .map(|hex| u16::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok())
        .collect::<Option<_>>()?;
    let grid = Grid::with_candidates(&values, &cands.try_into().ok()?);
    let step = grid.find(t)?;
    Some(Example {
        values,
        cands: *grid.cands(),
        step,
    })
}

/// The `examples.txt` line for `t` on `grid`.
fn line(t: Technique, grid: &Grid) -> String {
    let values: String = grid
        .values()
        .iter()
        .map(|&v| char::from(b'0' + v))
        .collect();
    let masks: String = grid.cands().iter().map(|m| format!("{m:03x}")).collect();
    format!("{}|{values}|{masks}", t.name())
}

/// Search dealt puzzles for a worked example of each `wanted` technique,
/// as lines for `examples.txt`, and name the ones not found on stderr.
///
/// Every state of every solve where a technique is the easiest step is a
/// candidate; the best has the fewest empty cells, then the fewest marked
/// candidates, so the pattern stands out on a quiet board. A strategy in
/// [`NEVER_FIRST`] is never the easiest step, so every state where it
/// applies at all is a candidate instead, and the best is the one whose
/// easiest step is hardest: no single or pair is waiting there, so the
/// pattern is needed as nearly as it can be.
pub fn find_examples(wanted: &[Technique], seed: u64, max_puzzles: usize) -> Vec<String> {
    let (beaten, named): (Vec<Technique>, Vec<Technique>) =
        wanted.iter().partition(|t| NEVER_FIRST.contains(t));
    // Only where the tutor itself would name the technique: a pattern an
    // easier one would catch first is often just smaller ones side by
    // side, like a hidden pair and a hidden single passing for a hidden
    // triple.
    let mut found = search(&named, seed, max_puzzles, ENOUGH, |_, next, t| {
        (next.technique == t).then(|| next.clone())
    });
    found.extend(search(
        &beaten,
        seed,
        max_puzzles,
        usize::MAX,
        |grid, _, t| grid.find(t),
    ));
    let mut lines = Vec::new();
    for &t in wanted {
        let at = named.iter().chain(&beaten).position(|&u| u == t);
        match at.and_then(|k| found[k].clone()) {
            Some(line) => lines.push(line),
            None => eprintln!("no example found for {}", t.name()),
        }
    }
    lines
}

/// The best example line for each `wanted` technique over the states of
/// up to `max_puzzles` dealt puzzles, counting a state for `t` when `step`
/// gives a step there (from the grid, the solver's next step and `t`), and
/// stopping once each technique has had `enough`. The best has the
/// hardest next step, then the fewest empty cells, then the fewest marks.
fn search(
    wanted: &[Technique],
    seed: u64,
    max_puzzles: usize,
    enough: usize,
    step: impl Fn(&Grid, &Step, Technique) -> Option<Step>,
) -> Vec<Option<String>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut seen = vec![0; wanted.len()];
    // (rank, line) of the best candidate so far.
    type Ranked = ((usize, usize, usize), String);
    let mut best: Vec<Option<Ranked>> = vec![None; wanted.len()];
    for _ in 0..max_puzzles {
        if seen.iter().all(|&n| n >= enough) {
            break;
        }
        let solution = sudoku::full_solution(&mut rng);
        let puzzle = sudoku::dig(&solution, CELLS, &mut rng);
        let mut grid = Grid::new(&puzzle);
        while let Some(next) = grid.next_step() {
            for (k, &t) in wanted.iter().enumerate() {
                if seen[k] >= enough {
                    continue;
                }
                let Some(found) = step(&grid, &next, t) else {
                    continue;
                };
                seen[k] += 1;
                let easier = Technique::ALL.len() - next.technique as usize;
                let empty = grid.values().iter().filter(|&&v| v == 0).count();
                let rank = (easier, empty, found.pattern.marks.len());
                if best[k].as_ref().is_none_or(|b| rank < b.0) {
                    best[k] = Some((rank, line(t, &grid)));
                }
            }
            grid.apply(&next);
        }
    }
    best.into_iter().map(|b| b.map(|(_, line)| line)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::Grid;

    #[test]
    fn every_technique_has_a_worked_example() {
        for &t in Technique::ALL {
            let e = example(t).unwrap_or_else(|| panic!("{t:?}"));
            assert_eq!(e.step.technique, t);
            let grid = Grid::with_candidates(&e.values, &e.cands);
            let next = grid.next_step().unwrap_or_else(|| panic!("{t:?}"));
            if NEVER_FIRST.contains(&t) {
                // An easier step always comes first there.
                assert!(next.technique < t, "{t:?}");
            } else {
                // And it is the step the tutor itself would name there.
                assert_eq!(next, e.step, "{t:?}");
            }
        }
    }

    #[test]
    fn find_examples_finds_a_common_technique() {
        let lines = find_examples(&[Technique::Pointing], 7, 200);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("pointing|"));
    }

    #[test]
    fn found_examples_read_back_as_the_same_step() {
        let lines = find_examples(&[Technique::Pointing], 7, 200);
        let [line] = &lines[..] else {
            panic!("expected one line, got {lines:?}")
        };
        let fields: Vec<&str> = line.split('|').collect();
        assert_eq!(fields.len(), 3, "{line}");
        assert_eq!(fields[1].len(), CELLS);
        assert_eq!(fields[2].len(), 3 * CELLS);
        let e = parse(line).expect("the line parses");
        assert_eq!(e.step.technique, Technique::Pointing);
        let grid = Grid::with_candidates(&e.values, &e.cands);
        assert_eq!(grid.find(Technique::Pointing), Some(e.step));
    }

    #[test]
    fn malformed_lines_are_skipped() {
        for line in [
            "pointing",
            "pointing|123|456",
            "no such thing|0|0",
            &format!("pointing|{}|{}", "x".repeat(CELLS), "0".repeat(3 * CELLS)),
        ] {
            assert!(parse(line).is_none(), "{line}");
        }
    }
}
