//! `srl stats`: atoms per state, reviews today, recent retention (see
//! `docs/cli.md`, "Analysis").

use std::collections::HashSet;

use jiff::Span;
use serde::Serialize;

use crate::{Atom, Memories, Reviewed, State};

/// How many study days, today included, retention looks back.
const RETENTION_DAYS: i64 = 30;

/// What `srl stats` reports about a set of atoms.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    /// How many atoms were looked at.
    pub atoms: usize,
    pub states: StateCounts,
    /// Reviews of these atoms on today's study day.
    pub reviews_today: usize,
    /// Over the last 30 study days, the share of reviews of these atoms
    /// that were not Again, counting only reviews of atoms that were not
    /// new. `None` if there were none.
    pub retention: Option<f64>,
    /// How many reviews `retention` is computed from.
    pub retention_reviews: usize,
}

/// Atoms per state. `leech` overlaps the other four, which add up to all
/// atoms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct StateCounts {
    pub new: usize,
    pub due: usize,
    pub young: usize,
    pub mature: usize,
    pub leech: usize,
}

/// The statistics of `atoms`, from their memories and every review of the
/// log.
pub fn stats(atoms: &[&Atom], memories: &Memories, reviews: &[Reviewed]) -> Stats {
    let states = atoms.iter().fold(StateCounts::default(), |counts, atom| {
        let id = atom.id();
        let counts = StateCounts {
            leech: counts.leech + usize::from(memories.is_leech(id)),
            ..counts
        };
        match memories.state(id) {
            State::New => StateCounts {
                new: counts.new + 1,
                ..counts
            },
            State::Due => StateCounts {
                due: counts.due + 1,
                ..counts
            },
            State::Young => StateCounts {
                young: counts.young + 1,
                ..counts
            },
            State::Mature => StateCounts {
                mature: counts.mature + 1,
                ..counts
            },
        }
    });
    let ids = atoms.iter().map(|atom| atom.id()).collect::<HashSet<_>>();
    let today = memories.today();
    let first_day = today
        .checked_sub(Span::new().days(RETENTION_DAYS - 1))
        .unwrap_or(today);
    let reviews = reviews
        .iter()
        .filter(|review| ids.contains(&review.atom))
        .collect::<Vec<_>>();
    let recalled = reviews
        .iter()
        .filter(|review| !review.was_new && review.day >= first_day && review.day <= today)
        .map(|review| review.rating > 1)
        .collect::<Vec<_>>();
    let retention = (!recalled.is_empty()).then(|| {
        let kept = recalled.iter().filter(|kept| **kept).count();
        ratio(kept, recalled.len())
    });
    Stats {
        atoms: atoms.len(),
        states,
        reviews_today: reviews.iter().filter(|review| review.day == today).count(),
        retention,
        retention_reviews: recalled.len(),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "review counts are far below the 2^52 where f64 loses precision"
)]
fn ratio(part: usize, whole: usize) -> f64 {
    part as f64 / whole as f64
}

#[cfg(test)]
mod tests {
    use jiff::civil::{Date, date};

    use super::*;
    use crate::{By, Config, Entry, Event, History, Id, Log, Review};

    fn atom(id: &str) -> Atom {
        Atom::parse("a.md".to_owned(), format!("---\nid: {id}\ntitle: x\n---\n")).unwrap()
    }

    fn reviewed(id: &str, day: Date, rating: u8) -> Entry {
        Entry {
            event: Event::Review(Review {
                t: format!("{day}T12:00:00+00:00"),
                atom: Id::parse(id).unwrap(),
                rating,
                card: None,
                response: None,
                by: By::Human,
                grader: "self".to_owned(),
                hash: String::new(),
            }),
            day,
        }
    }

    #[test]
    fn states_today_and_retention_are_counted() {
        let today = date(2026, 10, 26);
        let entries = vec![
            // Long ago: outside the 30 days.
            reviewed("aaaaa1", date(2026, 8, 1), 3),
            reviewed("aaaaa1", date(2026, 8, 2), 1),
            // Within 30 days: one Again and one Good on atoms that were not new.
            reviewed("aaaaa1", date(2026, 10, 1), 1),
            reviewed("aaaaa1", today, 3),
            // A first review: not counted for retention.
            reviewed("aaaaa2", today, 1),
        ];
        let log = Log {
            entries,
            problems: Vec::new(),
        };
        let config = Config::default();
        let history = History::replay(&log, &config).unwrap();
        let memories = Memories::new(history.memories, &config, today);
        let atoms = [atom("aaaaa1"), atom("aaaaa2"), atom("aaaaa3")];
        let refs = atoms.iter().collect::<Vec<_>>();
        let stats = stats(&refs, &memories, &history.reviews);
        assert_eq!(stats.atoms, 3);
        assert_eq!(stats.states.new, 1);
        assert_eq!(stats.states.young, 2);
        assert_eq!(stats.reviews_today, 2);
        assert_eq!(stats.retention_reviews, 2);
        assert_eq!(stats.retention, Some(0.5));
    }

    #[test]
    fn no_reviews_means_no_retention() {
        let log = Log {
            entries: Vec::new(),
            problems: Vec::new(),
        };
        let memories = Memories::replay(&log, &Config::default(), date(2026, 10, 26)).unwrap();
        let atom = atom("aaaaa1");
        let stats = stats(&[&atom], &memories, &[]);
        assert_eq!(stats.retention, None);
        assert_eq!(stats.states.new, 1);
    }
}
