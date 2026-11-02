//! Memory states, computed by replaying the log through FSRS (see
//! `docs/scheduling.md`).

use std::collections::HashMap;

use fsrs::{FSRS, MemoryState};
use jiff::Span;
use jiff::civil::Date;

use crate::{CardId, Config, Entry, Error, Event, Log, Result};

/// The longest interval, in days. FSRS keeps intervals far below this; the
/// limit only makes the conversion from its `f32` exact.
const MAX_INTERVAL: f32 = 36_500.0;

/// When a card was last reviewed: where in the log, and on which study
/// day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    pub position: usize,
    pub day: Date,
}

/// What the log says about one card.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Memory {
    /// Reviews since the last reset.
    pub reviews: usize,
    /// Agains on the card when it was not new, since the last reset.
    pub lapses: u32,
    /// FSRS's state, or `None` while the card is new.
    pub state: Option<MemoryState>,
    /// The study day of the first review since the last reset.
    pub first_review: Option<Date>,
    /// The study day of the latest review since the last reset.
    pub last_review: Option<Date>,
    /// The study day the card is due again.
    pub due: Option<Date>,
    /// Whether the card is suspended.
    pub suspended: bool,
    /// Its last review, whether before or after a reset, for the sibling
    /// rule (see `docs/scheduling.md`, "Siblings").
    pub seen: Option<Seen>,
    /// Whether any event is about this card.
    pub touched: bool,
}

impl Memory {
    pub fn stability(&self) -> Option<f32> {
        self.state.map(|state| state.stability)
    }
}

/// A scheduling state (see `docs/scheduling.md`, "States").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    New,
    Due,
    Young,
    Mature,
}

/// One review of the log, as replay saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reviewed {
    pub card: CardId,
    /// The review's study day.
    pub day: Date,
    pub rating: u8,
    /// Whether the card had no review since its last reset before this one.
    pub was_new: bool,
}

/// What replaying the log gives, whatever the study day: every card's
/// memory and every review.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    /// The memory of every card the log mentions.
    pub memories: HashMap<CardId, Memory>,
    /// Every review of the log, in log order.
    pub reviews: Vec<Reviewed>,
}

impl History {
    /// Replays the whole log, in log order, with the FSRS weights and
    /// desired retention of `config`.
    pub fn replay(log: &Log, config: &Config) -> Result<Self> {
        let replayer = Replayer::new(config)?;
        let mut history = Self::default();
        for (position, entry) in log.entries.iter().enumerate() {
            let (_, reviewed) = replayer.apply(&mut history.memories, entry, position)?;
            history.reviews.extend(reviewed);
        }
        Ok(history)
    }
}

/// Replays events one at a time, with the FSRS weights and desired
/// retention of the configuration. Replaying the whole log and adding new
/// events to a cached replay both go through `apply`, so they agree.
pub(crate) struct Replayer {
    fsrs: FSRS,
    retention: f32,
}

impl Replayer {
    pub(crate) fn new(config: &Config) -> Result<Self> {
        let schedule = &config.schedule;
        let fsrs =
            FSRS::new(&weights(schedule.weights.as_deref())).map_err(|error| Error::Fsrs {
                reason: format!("the weights are not valid ({error})"),
            })?;
        Ok(Self {
            fsrs,
            retention: to_f32(schedule.desired_retention),
        })
    }

    /// Applies `entry`, the event at `position` in the log, to `memories`.
    /// Returns the cards whose memory changed, and the review it records,
    /// if it is one.
    pub(crate) fn apply(
        &self,
        memories: &mut HashMap<CardId, Memory>,
        entry: &Entry,
        position: usize,
    ) -> Result<(Vec<CardId>, Option<Reviewed>)> {
        let memory = |memories: &HashMap<CardId, Memory>, card: &CardId| {
            memories.get(card).cloned().unwrap_or_default()
        };
        match &entry.event {
            Event::Review(review) => {
                let old = memory(memories, &review.card);
                let reviewed = Reviewed {
                    card: review.card.clone(),
                    day: entry.day,
                    rating: review.rating,
                    was_new: old.reviews == 0,
                };
                let next = Memory {
                    seen: Some(Seen {
                        position,
                        day: entry.day,
                    }),
                    ..step(&self.fsrs, self.retention, &old, entry.day, review.rating)?
                };
                memories.insert(review.card.clone(), next);
                Ok((vec![review.card.clone()], Some(reviewed)))
            }
            Event::Reset(event) => {
                let old = memory(memories, &event.card);
                let next = Memory {
                    suspended: old.suspended,
                    seen: old.seen,
                    touched: true,
                    ..Memory::default()
                };
                memories.insert(event.card.clone(), next);
                Ok((vec![event.card.clone()], None))
            }
            Event::Suspend(event) | Event::Unsuspend(event) => {
                let old = memory(memories, &event.card);
                let next = Memory {
                    suspended: matches!(entry.event, Event::Suspend(_)),
                    touched: true,
                    ..old
                };
                memories.insert(event.card.clone(), next);
                Ok((vec![event.card.clone()], None))
            }
            Event::Move(moved) => {
                let history = memory(memories, &moved.from);
                let next = Memory {
                    touched: true,
                    ..history
                };
                memories.insert(moved.to.clone(), next);
                memories.insert(moved.from.clone(), Memory::default());
                Ok((vec![moved.from.clone(), moved.to.clone()], None))
            }
        }
    }
}

/// Every card's memory, on one study day.
#[derive(Debug, Clone)]
pub struct Memories {
    by_card: HashMap<CardId, Memory>,
    today: Date,
    mature_days: u32,
    leech_lapses: u32,
}

impl Memories {
    /// The memories of a replay, seen on study day `today`.
    pub fn new(by_card: HashMap<CardId, Memory>, config: &Config, today: Date) -> Self {
        Self {
            by_card,
            today,
            mature_days: config.schedule.mature_days,
            leech_lapses: config.schedule.leech_lapses,
        }
    }

    /// Replays the whole log (see [`History::replay`]) and keeps the
    /// memories.
    pub fn replay(log: &Log, config: &Config, today: Date) -> Result<Self> {
        Ok(Self::new(
            History::replay(log, config)?.memories,
            config,
            today,
        ))
    }

    /// The memory of card `id`; a card the log never mentions is new.
    pub fn get(&self, id: &CardId) -> Memory {
        self.by_card.get(id).cloned().unwrap_or_default()
    }

    /// Every card the log mentions, with its memory.
    pub fn all(&self) -> impl Iterator<Item = (&CardId, &Memory)> {
        self.by_card.iter()
    }

    /// The study day the states are computed for.
    pub fn today(&self) -> Date {
        self.today
    }

    /// The state of card `id` today.
    pub fn state(&self, id: &CardId) -> State {
        let memory = self.get(id);
        match (memory.due, memory.stability()) {
            (Some(due), _) if due <= self.today => State::Due,
            (Some(_), Some(stability)) if stability >= to_f32(f64::from(self.mature_days)) => {
                State::Mature
            }
            (Some(_), _) => State::Young,
            (None, _) => State::New,
        }
    }

    /// Whether card `id` has at least `leech_lapses` lapses.
    pub fn is_leech(&self, id: &CardId) -> bool {
        self.get(id).lapses >= self.leech_lapses
    }

    /// Whether card `id` is suspended.
    pub fn is_suspended(&self, id: &CardId) -> bool {
        self.get(id).suspended
    }

    /// Whether card `id` was reviewed on today's study day.
    pub fn reviewed_today(&self, id: &CardId) -> bool {
        self.get(id).seen.is_some_and(|seen| seen.day == self.today)
    }

    /// How many cards had their first review since their last reset today
    /// (see `docs/scheduling.md`, "The new-card limit").
    pub fn new_today(&self) -> usize {
        self.by_card
            .values()
            .filter(|memory| memory.first_review == Some(self.today))
            .count()
    }
}

/// The memory after one more review with `rating` on study day `day`.
fn step(fsrs: &FSRS, retention: f32, memory: &Memory, day: Date, rating: u8) -> Result<Memory> {
    // Events are replayed in log order; a review dated before the previous
    // one counts as on the same day.
    let elapsed = memory
        .last_review
        .map_or(0, |last| (day - last).get_days().max(0));
    let elapsed = u32::try_from(elapsed).expect("a non-negative day count should fit in a u32");
    let next = fsrs
        .next_states(memory.state, retention, elapsed)
        .map_err(|error| Error::Fsrs {
            reason: error.to_string(),
        })?;
    let item = match rating {
        1 => next.again,
        2 => next.hard,
        3 => next.good,
        _ => next.easy,
    };
    let interval = interval_days(item.interval);
    let due = day
        .checked_add(Span::new().days(interval))
        .map_err(|error| Error::Fsrs {
            reason: format!("the due day is out of range ({error})"),
        })?;
    let lapse = u32::from(rating == 1 && memory.reviews > 0);
    Ok(Memory {
        reviews: memory.reviews + 1,
        lapses: memory.lapses + lapse,
        state: Some(item.memory),
        first_review: memory.first_review.or(Some(day)),
        last_review: Some(day),
        due: Some(due),
        touched: true,
        ..memory.clone()
    })
}

/// An FSRS interval as whole days: rounded, and at least one day (see
/// `docs/scheduling.md`, "Days, not minutes").
fn interval_days(interval: f32) -> i64 {
    let days = interval.round().clamp(1.0, MAX_INTERVAL);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is a whole number between 1 and MAX_INTERVAL"
    )]
    let days = days as i64;
    days
}

/// The FSRS weights: those of `config.toml`, or none for FSRS's defaults.
fn weights(configured: Option<&[f64]>) -> Vec<f32> {
    configured
        .unwrap_or_default()
        .iter()
        .map(|weight| to_f32(*weight))
        .collect()
}

/// FSRS works in `f32`; configuration values are read as `f64`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "configuration values are small numbers that f32 holds well enough"
)]
fn to_f32(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::{By, CardEvent, Entry, Move, Review};

    fn id() -> CardId {
        CardId::parse("a7f3k2").unwrap()
    }

    fn at(day: Date) -> String {
        format!("{day}T12:00:00+00:00")
    }

    fn review(day: Date, rating: u8) -> Entry {
        Entry {
            event: Event::Review(Review {
                t: at(day),
                card: id(),
                rating,
                response: None,
                by: By::Human,
                hash: String::new(),
            }),
            day,
        }
    }

    fn card_event(day: Date, make: fn(CardEvent) -> Event) -> Entry {
        Entry {
            event: make(CardEvent {
                t: at(day),
                card: id(),
            }),
            day,
        }
    }

    fn replay(entries: Vec<Entry>, today: Date) -> Memories {
        let log = Log {
            entries,
            problems: Vec::new(),
        };
        Memories::replay(&log, &Config::default(), today).unwrap()
    }

    #[test]
    fn a_card_without_reviews_is_new() {
        let memories = replay(Vec::new(), date(2026, 10, 26));
        assert_eq!(memories.state(&id()), State::New);
        assert!(!memories.get(&id()).touched);
    }

    #[test]
    fn a_reviewed_card_is_not_due_the_same_day_but_is_later() {
        let day = date(2026, 10, 26);
        let memories = replay(vec![review(day, 1)], day);
        let memory = memories.get(&id());
        assert_eq!(memories.state(&id()), State::Young);
        assert_eq!(memory.due, Some(date(2026, 10, 27)));
        assert!(memories.reviewed_today(&id()));
        let later = replay(vec![review(day, 1)], date(2026, 10, 27));
        assert_eq!(later.state(&id()), State::Due);
        assert!(!later.reviewed_today(&id()));
    }

    #[test]
    fn good_reviews_grow_the_interval() {
        let first = date(2026, 1, 1);
        let one = replay(vec![review(first, 3)], first);
        let due = one.get(&id()).due.unwrap();
        let two = replay(vec![review(first, 3), review(due, 3)], due);
        let second = two.get(&id());
        assert!((second.due.unwrap() - due).get_days() > (due - first).get_days());
        assert_eq!(second.reviews, 2);
        assert_eq!(second.first_review, Some(first));
    }

    #[test]
    fn again_on_a_reviewed_card_is_a_lapse() {
        let day = date(2026, 1, 1);
        let entries = (0..8)
            .map(|offset| review(day.checked_add(Span::new().days(offset)).unwrap(), 1))
            .collect::<Vec<_>>();
        let memories = replay(entries, date(2026, 2, 1));
        // The first Again was on a new card, so it is not a lapse.
        assert_eq!(memories.get(&id()).lapses, 7);
        assert!(!memories.is_leech(&id()));
    }

    #[test]
    fn a_reset_makes_the_card_new_but_keeps_its_last_review_and_status() {
        let day = date(2026, 1, 1);
        let memories = replay(
            vec![
                review(day, 3),
                card_event(day, Event::Suspend),
                card_event(day, Event::Reset),
            ],
            day,
        );
        assert_eq!(memories.state(&id()), State::New);
        assert_eq!(memories.get(&id()).seen.map(|seen| seen.position), Some(0));
        assert!(memories.is_suspended(&id()));
        assert_eq!(memories.new_today(), 0);
    }

    #[test]
    fn the_last_suspend_or_unsuspend_decides() {
        let day = date(2026, 1, 1);
        let memories = replay(
            vec![
                card_event(day, Event::Suspend),
                card_event(day, Event::Unsuspend),
            ],
            day,
        );
        assert!(!memories.is_suspended(&id()));
        assert!(memories.get(&id()).touched);
    }

    #[test]
    fn a_move_gives_the_history_to_another_card() {
        let day = date(2026, 1, 1);
        let to = CardId::parse("a7f3k2.1").unwrap();
        let moved = Entry {
            event: Event::Move(Move {
                t: at(day),
                from: id(),
                to: to.clone(),
            }),
            day,
        };
        let memories = replay(vec![review(day, 3), moved], day);
        assert_eq!(memories.get(&to).reviews, 1);
        assert!(memories.get(&to).seen.is_some());
        assert_eq!(memories.get(&id()), Memory::default());
    }

    #[test]
    fn new_today_counts_first_reviews_on_this_study_day() {
        let day = date(2026, 1, 2);
        let memories = replay(vec![review(date(2026, 1, 1), 3)], day);
        assert_eq!(memories.new_today(), 0);
        let memories = replay(vec![review(day, 3)], day);
        assert_eq!(memories.new_today(), 1);
    }
}
