//! The techniques library: every strategy the tutor knows, laid out by
//! level in `Technique::ALL`'s order, easiest first, to read like a short
//! course or to look a name up.

#![allow(dead_code)] // used once the library is drawn (Task 5)

use crate::deal::{Difficulty, level_of};
use crate::logic::Technique;

/// What the library shows in place of the title menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Library {
    /// Every technique, by level, with `at` highlighted.
    Map { at: Technique },
    /// The tutor's explanation of `at`, its worked example at `stage`.
    Lesson { at: Technique, stage: u8 },
}

/// `level`'s techniques, easiest first: one column of the map.
pub fn column(level: Difficulty) -> Vec<Technique> {
    Technique::ALL
        .iter()
        .copied()
        .filter(|&t| level_of(t) == level)
        .collect()
}

/// Where the map's arrows go from `at`: `dr` rows within its column, or
/// `dc` columns across, keeping the row where the next column has one and
/// taking its last technique where it's shorter. Nothing wraps.
pub fn map_move(at: Technique, dr: i8, dc: i8) -> Technique {
    let level = level_of(at);
    let row = column(level).iter().position(|&t| t == at).unwrap_or(0);
    let col = step(level.index(), dc, Difficulty::ALL.len());
    let techniques = column(Difficulty::ALL[col]);
    let row = if col == level.index() {
        step(row, dr, techniques.len())
    } else {
        row.min(techniques.len() - 1)
    };
    techniques[row]
}

/// The technique `delta` places from `at` in `Technique::ALL`, crossing
/// levels, stopping at the first and the last.
pub fn turn(at: Technique, delta: i8) -> Technique {
    let i = place(at) - 1;
    Technique::ALL[step(i, delta, Technique::ALL.len())]
}

/// `at`'s place in the whole list, counting from 1.
pub fn place(at: Technique) -> usize {
    Technique::ALL.iter().position(|&t| t == at).unwrap_or(0) + 1
}

/// `i` moved by `delta`, kept within `0..len`.
fn step(i: usize, delta: i8, len: usize) -> usize {
    i.saturating_add_signed(isize::from(delta)).min(len - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Technique::*;

    #[test]
    fn columns_hold_every_technique_once_by_level() {
        let counts: Vec<usize> = Difficulty::ALL.iter().map(|&d| column(d).len()).collect();
        assert_eq!(counts, [3, 6, 8, 12, 7]);
        let all: Vec<Technique> = Difficulty::ALL.iter().flat_map(|&d| column(d)).collect();
        assert_eq!(all, Technique::ALL);
        assert_eq!(column(Difficulty::Hard)[3], Skyscraper);
    }

    #[test]
    fn up_and_down_stay_in_the_column_and_stop_at_its_ends() {
        assert_eq!(map_move(Skyscraper, 1, 0), TwoStringKite);
        assert_eq!(map_move(Skyscraper, -1, 0), XWing);
        assert_eq!(map_move(XYWing, 1, 0), XYWing, "no falling into Master");
        assert_eq!(
            map_move(NakedQuad, -1, 0),
            NakedQuad,
            "no climbing into Medium"
        );
        assert_eq!(map_move(FullHouse, -1, 0), FullHouse);
    }

    #[test]
    fn left_and_right_keep_the_row_or_take_the_last() {
        assert_eq!(map_move(Skyscraper, 0, -1), HiddenPair, "row 4 to row 4");
        assert_eq!(map_move(Skyscraper, 0, 1), FinnedXWing);
        // Master's row 12 to Hard, which has 8.
        assert_eq!(map_move(FinnedJellyfish, 0, -1), XYWing);
        assert_eq!(map_move(XYWing, 0, -1), HiddenTriple, "Medium has 6");
        assert_eq!(
            map_move(HiddenSingle, 0, -1),
            HiddenSingle,
            "no wrap at Easy"
        );
        assert_eq!(map_move(DeathBlossom, 0, 1), DeathBlossom, "no wrap at Zen");
    }

    #[test]
    fn turning_runs_through_all_in_order_and_stops_at_the_ends() {
        assert_eq!(turn(EmptyRectangle, 1), XYWing);
        assert_eq!(turn(XYWing, 1), Swordfish, "crosses into Master");
        assert_eq!(turn(Swordfish, -1), XYWing);
        assert_eq!(turn(FullHouse, -1), FullHouse);
        assert_eq!(turn(DeathBlossom, 1), DeathBlossom);
    }

    #[test]
    fn places_count_from_one() {
        assert_eq!(place(FullHouse), 1);
        assert_eq!(place(Skyscraper), 13);
        assert_eq!(place(DeathBlossom), 36);
    }
}
