//! Memory states, computed by replaying the log through FSRS (see
//! `docs/scheduling.md`).

use std::collections::HashMap;

use fsrs::{FSRS, MemoryState};
use jiff::Span;
use jiff::civil::Date;

use crate::{Config, Entry, Error, Event, Id, Log, Result};

/// The longest interval, in days. FSRS keeps intervals far below this; the
/// limit only makes the conversion from its `f32` exact.
const MAX_INTERVAL: f32 = 36_500.0;

/// What the log says about one atom, from its last `reset` on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Memory {
    /// Reviews since the last reset.
    pub reviews: usize,
    /// Agains on the atom when it was not new, since the last reset.
    pub lapses: u32,
    /// FSRS's state, or `None` while the atom is new.
    pub state: Option<MemoryState>,
    /// The study day of the first review since the last reset.
    pub first_review: Option<Date>,
    /// The study day of the latest review.
    pub last_review: Option<Date>,
    /// The study day the atom is due again.
    pub due: Option<Date>,
    /// For each card ref used on this atom, where in the log it was last
    /// used. Kept across resets: it only decides which card comes next.
    pub(crate) last_used: HashMap<String, usize>,
}

impl Memory {
    /// Where in the log the card `reference` was last used for this atom.
    pub fn last_used(&self, reference: &str) -> Option<usize> {
        self.last_used.get(reference).copied()
    }

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
    pub atom: Id,
    /// The review's study day.
    pub day: Date,
    pub rating: u8,
    /// Whether the atom had no review since its last reset before this one.
    pub was_new: bool,
}

/// What replaying the log gives, whatever the study day: every atom's
/// memory and every review.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    /// The memory of every atom the log mentions.
    pub memories: HashMap<Id, Memory>,
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
            let memory = history
                .memories
                .entry(entry.event.atom().clone())
                .or_default();
            let (next, reviewed) = replayer.apply(memory, entry, position)?;
            *memory = next;
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

    /// The memory of an atom after `entry`, the event at `position` in the
    /// log, and the review it records, if it is one.
    pub(crate) fn apply(
        &self,
        memory: &Memory,
        entry: &Entry,
        position: usize,
    ) -> Result<(Memory, Option<Reviewed>)> {
        match &entry.event {
            Event::Reset(_) => {
                let memory = Memory {
                    last_used: memory.last_used.clone(),
                    ..Memory::default()
                };
                Ok((memory, None))
            }
            Event::Review(review) => {
                let reviewed = Reviewed {
                    atom: review.atom.clone(),
                    day: entry.day,
                    rating: review.rating,
                    was_new: memory.reviews == 0,
                };
                let mut next = step(&self.fsrs, self.retention, memory, entry.day, review.rating)?;
                if let Some(card) = &review.card {
                    next.last_used.insert(card.reference.clone(), position);
                }
                Ok((next, Some(reviewed)))
            }
        }
    }
}

/// Every atom's memory, on one study day.
#[derive(Debug, Clone)]
pub struct Memories {
    by_atom: HashMap<Id, Memory>,
    today: Date,
    mature_days: u32,
    leech_lapses: u32,
}

impl Memories {
    /// The memories of a replay, seen on study day `today`.
    pub fn new(by_atom: HashMap<Id, Memory>, config: &Config, today: Date) -> Self {
        Self {
            by_atom,
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

    /// The memory of atom `id`; an atom the log never mentions is new.
    pub fn get(&self, id: &Id) -> Memory {
        self.by_atom.get(id).cloned().unwrap_or_default()
    }

    /// The study day the states are computed for.
    pub fn today(&self) -> Date {
        self.today
    }

    /// The state of atom `id` today.
    pub fn state(&self, id: &Id) -> State {
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

    /// Whether atom `id` has at least `leech_lapses` lapses.
    pub fn is_leech(&self, id: &Id) -> bool {
        self.get(id).lapses >= self.leech_lapses
    }

    /// How many atoms had their first review since their last reset today
    /// (see `docs/scheduling.md`, "The new-atom limit").
    pub fn new_today(&self) -> usize {
        self.by_atom
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
        last_used: memory.last_used.clone(),
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

/// The FSRS weights: those of `srl.toml`, or none for FSRS's defaults.
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
    use crate::{By, Entry, Reset, Review};

    fn id() -> Id {
        Id::parse("a7f3k2").unwrap()
    }

    fn review(day: Date, rating: u8, reference: &str) -> Entry {
        Entry {
            event: Event::Review(Review {
                t: format!("{day}T12:00:00+00:00"),
                atom: id(),
                rating,
                card: Some(crate::CardUsed {
                    drill: "cards".to_owned(),
                    reference: reference.to_owned(),
                }),
                response: None,
                by: By::Human,
                hash: String::new(),
            }),
            day,
        }
    }

    fn reset(day: Date) -> Entry {
        Entry {
            event: Event::Reset(Reset {
                t: format!("{day}T12:00:00+00:00"),
                atom: id(),
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
    fn an_atom_without_reviews_is_new() {
        let memories = replay(Vec::new(), date(2026, 10, 26));
        assert_eq!(memories.state(&id()), State::New);
    }

    #[test]
    fn a_reviewed_atom_is_not_due_the_same_day_but_is_later() {
        let day = date(2026, 10, 26);
        let memories = replay(vec![review(day, 1, "cards:0")], day);
        let memory = memories.get(&id());
        assert_eq!(memories.state(&id()), State::Young);
        assert_eq!(memory.due, Some(date(2026, 10, 27)));
        let later = replay(vec![review(day, 1, "cards:0")], date(2026, 10, 27));
        assert_eq!(later.state(&id()), State::Due);
    }

    #[test]
    fn good_reviews_grow_the_interval() {
        let first = date(2026, 1, 1);
        let one = replay(vec![review(first, 3, "cards:0")], first);
        let due = one.get(&id()).due.unwrap();
        let two = replay(
            vec![review(first, 3, "cards:0"), review(due, 3, "cards:0")],
            due,
        );
        let second = two.get(&id());
        assert!((second.due.unwrap() - due).get_days() > (due - first).get_days());
        assert_eq!(second.reviews, 2);
        assert_eq!(second.first_review, Some(first));
    }

    #[test]
    fn again_on_a_reviewed_atom_is_a_lapse() {
        let day = date(2026, 1, 1);
        let entries = (0..8)
            .map(|offset| {
                review(
                    day.checked_add(Span::new().days(offset)).unwrap(),
                    1,
                    "cards:0",
                )
            })
            .collect::<Vec<_>>();
        let memories = replay(entries, date(2026, 2, 1));
        // The first Again was on a new atom, so it is not a lapse.
        assert_eq!(memories.get(&id()).lapses, 7);
        assert!(!memories.is_leech(&id()));
    }

    #[test]
    fn a_reset_makes_the_atom_new_but_remembers_cards() {
        let day = date(2026, 1, 1);
        let memories = replay(vec![review(day, 3, "cloze:0:1"), reset(day)], day);
        assert_eq!(memories.state(&id()), State::New);
        assert_eq!(memories.get(&id()).last_used("cloze:0:1"), Some(0));
        assert_eq!(memories.new_today(), 0);
    }

    #[test]
    fn new_today_counts_first_reviews_on_this_study_day() {
        let day = date(2026, 1, 2);
        let memories = replay(vec![review(date(2026, 1, 1), 3, "cards:0")], day);
        assert_eq!(memories.new_today(), 0);
        let memories = replay(vec![review(day, 3, "cards:0")], day);
        assert_eq!(memories.new_today(), 1);
    }
}
