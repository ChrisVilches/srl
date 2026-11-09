//! `srl stats`: cards per place in the schedule, leeches, reviews today
//! with the new cards and Again ratings among them, recent retention (see
//! `docs/cli.md`, "Analysis").

use std::collections::HashSet;

use jiff::Span;
use serde::Serialize;

use crate::{CardId, Item, Memories, Place, Reviewed};

/// How many study days, today included, retention looks back.
const RETENTION_DAYS: i64 = 30;

/// What `srl stats` reports about a set of cards.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    /// How many cards were looked at.
    pub cards: usize,
    pub schedule: ScheduleCounts,
    /// How many of the cards are leeches, wherever they are in the schedule.
    pub leech: usize,
    /// Reviews of these cards on today's study day.
    pub reviews_today: usize,
    /// Of `reviews_today`, the first reviews of cards that were new.
    pub new_today: usize,
    /// Of `reviews_today`, those rated Again.
    pub again_today: usize,
    /// Over the last 30 study days, the share of reviews of these cards
    /// that were not Again, counting only reviews of cards that were not
    /// new. `None` if there were none.
    pub retention: Option<f64>,
    /// How many reviews `retention` is computed from.
    pub retention_reviews: usize,
}

/// Cards per place in the schedule, which add up to all cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct ScheduleCounts {
    pub new: usize,
    pub due: usize,
    pub young: usize,
    pub mature: usize,
}

/// The statistics of `cards`, from their memories and every review of the
/// log.
pub fn stats(cards: &[(&Item, &CardId)], memories: &Memories, reviews: &[Reviewed]) -> Stats {
    let schedule =
        cards.iter().fold(
            ScheduleCounts::default(),
            |counts, (_, id)| match memories.place(id) {
                Place::New => ScheduleCounts {
                    new: counts.new + 1,
                    ..counts
                },
                Place::Due => ScheduleCounts {
                    due: counts.due + 1,
                    ..counts
                },
                Place::Young => ScheduleCounts {
                    young: counts.young + 1,
                    ..counts
                },
                Place::Mature => ScheduleCounts {
                    mature: counts.mature + 1,
                    ..counts
                },
            },
        );
    let leech = cards.iter().filter(|(_, id)| memories.is_leech(id)).count();
    let ids = cards
        .iter()
        .map(|(_, card)| *card)
        .collect::<HashSet<&CardId>>();
    let today = memories.today();
    let first_day = today
        .checked_sub(Span::new().days(RETENTION_DAYS - 1))
        .unwrap_or(today);
    let reviews = reviews
        .iter()
        .filter(|review| ids.contains(&review.card))
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
    let today_reviews = reviews
        .iter()
        .filter(|review| review.day == today)
        .collect::<Vec<_>>();
    Stats {
        cards: cards.len(),
        schedule,
        leech,
        reviews_today: today_reviews.len(),
        new_today: today_reviews.iter().filter(|review| review.was_new).count(),
        again_today: today_reviews
            .iter()
            .filter(|review| review.rating == 1)
            .count(),
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
    use crate::app::catalog::read_items;
    use crate::{By, Config, Entry, Event, History, Log, Review};

    fn reviewed(id: &str, day: Date, rating: u8) -> Entry {
        Entry {
            event: Event::Review(Review {
                t: format!("{day}T12:00:00+00:00"),
                card: CardId::parse(id).unwrap(),
                rating,
                response: None,
                by: By::Human,
                hash: String::new(),
            }),
            day,
        }
    }

    fn items() -> Vec<Item> {
        read_items(
            "a.jsonl",
            "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"aaa2\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"aaa3\", \"Q\": \"q\", \"A\": \"a\"}\n",
            &crate::kind::Programs::default(),
        )
        .0
    }

    #[test]
    fn schedule_today_and_retention_are_counted() {
        let today = date(2026, 10, 26);
        let entries = vec![
            // Long ago: outside the 30 days.
            reviewed("aaa1", date(2026, 8, 1), 3),
            reviewed("aaa1", date(2026, 8, 2), 1),
            // Within 30 days: one Again and one Good on cards that were not new.
            reviewed("aaa1", date(2026, 10, 1), 1),
            reviewed("aaa1", today, 3),
            // A first review: not counted for retention.
            reviewed("aaa2", today, 1),
        ];
        let log = Log {
            entries,
            problems: Vec::new(),
        };
        let config = Config::default();
        let history = History::replay(&log, &config).unwrap();
        let memories = Memories::new(history.memories, &config, today);
        let items = items();
        let cards = items
            .iter()
            .map(|item| (item, &item.cards()[0]))
            .collect::<Vec<_>>();
        let stats = stats(&cards, &memories, &history.reviews);
        assert_eq!(stats.cards, 3);
        assert_eq!(stats.schedule.new, 1);
        assert_eq!(stats.schedule.young, 2);
        assert_eq!(stats.reviews_today, 2);
        assert_eq!(stats.new_today, 1);
        assert_eq!(stats.again_today, 1);
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
        let items = items();
        let stats = stats(&[(&items[0], &items[0].cards()[0])], &memories, &[]);
        assert_eq!(stats.retention, None);
        assert_eq!(stats.schedule.new, 1);
    }
}
