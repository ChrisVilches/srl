//! Recording reviews, resets, suspensions and moves (see `docs/log.md` and
//! `docs/scheduling.md`).

use jiff::Zoned;
use serde::Deserialize;

use crate::log;
use crate::{
    By, Card, CardEvent, CardId, Error, Event, Item, Memories, Move, Notes, Result, Review, State,
    Store,
};

/// What `srl review --stdin` reads: a card ID, its rating, and optionally
/// the response and who recorded the review.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequest {
    pub card: CardId,
    pub rating: u8,
    #[serde(default)]
    pub response: Option<String>,
    #[serde(default = "human")]
    pub by: By,
}

fn human() -> By {
    By::Human
}

impl ReviewRequest {
    /// Reads `{"card": "k3m9q1-2", "rating": N, "response": "…", "by": "human"}`.
    /// `response` and `by` may be left out; `by` is then `human`.
    pub fn parse(text: &str) -> Result<Self> {
        let request = serde_json::from_str::<Self>(text).map_err(|error| Error::InvalidReview {
            reason: format!("it is not a review ({error})"),
        })?;
        check_rating(request.rating)?;
        Ok(request)
    }
}

fn check_rating(rating: u8) -> Result<()> {
    if (1..=4).contains(&rating) {
        Ok(())
    } else {
        Err(Error::InvalidReview {
            reason: format!("the rating is {rating}, but must be 1 to 4"),
        })
    }
}

/// Records `request`'s rating for its card, which must be a card of the
/// store. Returns the events; with `dry_run`, they are not written.
pub fn review(
    store: &Store,
    notes: &Notes,
    requests: &[ReviewRequest],
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<Event>> {
    let events = requests
        .iter()
        .map(|request| {
            check_rating(request.rating)?;
            let (item, _) = known(notes, &request.card)?;
            Ok(Event::Review(Review {
                t: log::time(now),
                card: request.card.clone(),
                rating: request.rating,
                response: request.response.clone(),
                by: request.by,
                hash: log::hash(item),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    write(store, events, dry_run)
}

/// Makes each of `selected` that is not new again new, with a `reset`
/// event, and returns their IDs.
pub fn reset(
    store: &Store,
    selected: &[(&Item, &Card)],
    memories: &Memories,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<CardId>> {
    let ids = selected
        .iter()
        .map(|(_, card)| card.id())
        .filter(|id| memories.state(id) != State::New)
        .cloned()
        .collect::<Vec<_>>();
    let events = ids
        .iter()
        .map(|id| {
            Event::Reset(CardEvent {
                t: log::time(now),
                card: id.clone(),
            })
        })
        .collect();
    write(store, events, dry_run)?;
    Ok(ids)
}

/// Suspends (`suspend` true) or unsuspends each of `selected` that is
/// active, or suspended, and returns their IDs. Drafts are left alone.
pub fn suspend(
    store: &Store,
    selected: &[(&Item, &Card)],
    memories: &Memories,
    suspend: bool,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<CardId>> {
    let ids = selected
        .iter()
        .filter(|(item, card)| !item.is_draft() && memories.is_suspended(card.id()) != suspend)
        .map(|(_, card)| card.id().clone())
        .collect::<Vec<_>>();
    let events = ids
        .iter()
        .map(|id| {
            let event = CardEvent {
                t: log::time(now),
                card: id.clone(),
            };
            if suspend {
                Event::Suspend(event)
            } else {
                Event::Unsuspend(event)
            }
        })
        .collect();
    write(store, events, dry_run)?;
    Ok(ids)
}

/// Gives the history of the orphan `from` to the card `to`, which must have
/// no events at all (see `docs/scheduling.md`, "Moving history").
pub fn move_history(
    store: &Store,
    notes: &Notes,
    memories: &Memories,
    from: &CardId,
    to: &CardId,
    now: &Zoned,
    dry_run: bool,
) -> Result<Event> {
    let refuse = |reason: &str| Error::CannotMove {
        from: from.to_string(),
        to: to.to_string(),
        reason: reason.to_owned(),
    };
    if notes.card(from).is_some() {
        return Err(refuse(&format!(
            "{from} is still a card of the store; only an orphan's history can move"
        )));
    }
    if memories.get(from).seen.is_none() {
        return Err(refuse(&format!("{from} has no reviews to move")));
    }
    if notes.card(to).is_none() {
        return Err(refuse(&format!("{to} is not a card of the store")));
    }
    if memories.get(to).touched {
        return Err(refuse(&format!(
            "{to} already has events, and history is never merged"
        )));
    }
    let event = Event::Move(Move {
        t: log::time(now),
        from: from.clone(),
        to: to.clone(),
    });
    write(store, vec![event.clone()], dry_run)?;
    Ok(event)
}

fn known<'a>(notes: &'a Notes, id: &CardId) -> Result<(&'a Item, &'a Card)> {
    notes
        .card(id)
        .ok_or_else(|| Error::UnknownCard { id: id.to_string() })
}

fn write(store: &Store, events: Vec<Event>, dry_run: bool) -> Result<Vec<Event>> {
    if !dry_run {
        log::append(store, &events)?;
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::{Config, History, Log, Lookup};

    fn store() -> (TempDir, Store, Notes) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        std::fs::write(
            store.note_file("a.md"),
            "<!-- srl id:aaa1 -->\nQ: q\nA: a\n\n<!-- srl id:aaa2 -->\nQ: {{c1::x}} {{c2::y}}\n\n<!-- srl id:aaa3 draft -->\nQ: d\nA: e\n",
        )
        .unwrap();
        let notes = Notes::load(&store, &Config::default()).unwrap();
        (tmp, store, notes)
    }

    fn now() -> Zoned {
        "2026-10-26T09:12:31.5+09:00[+09:00]".parse().unwrap()
    }

    fn logged(store: &Store) -> Vec<Event> {
        let log = Log::load(store, &Config::default().schedule).unwrap();
        assert!(log.problems.is_empty(), "{:?}", log.problems);
        log.entries.into_iter().map(|entry| entry.event).collect()
    }

    fn memories(store: &Store) -> Memories {
        let log = Log::load(store, &Config::default().schedule).unwrap();
        let history = History::replay(&log, &Config::default()).unwrap();
        Memories::new(
            history.memories,
            &Config::default(),
            jiff::civil::date(2026, 10, 26),
        )
    }

    fn request(card: &str, rating: u8) -> ReviewRequest {
        ReviewRequest {
            card: CardId::parse(card).unwrap(),
            rating,
            response: None,
            by: By::Human,
        }
    }

    #[test]
    fn a_review_is_recorded_for_its_card() {
        let (_tmp, store, notes) = store();
        let request = ReviewRequest::parse(
            r#"{"card": "aaa2-2", "rating": 2, "response": "y", "by": "agent"}"#,
        )
        .unwrap();
        let events = review(&store, &notes, &[request], &now(), false).unwrap();
        assert_eq!(logged(&store), events);
        let Event::Review(first) = &events[0] else {
            panic!("expected a review");
        };
        assert_eq!(first.t, "2026-10-26T09:12:31+09:00");
        assert_eq!(first.card.to_string(), "aaa2-2");
        assert_eq!((first.rating, first.by), (2, By::Agent));
        assert_eq!(first.response.as_deref(), Some("y"));
        assert_eq!(first.hash.len(), 12);
    }

    #[test]
    fn bad_reviews_are_refused() {
        for text in [
            r#"{"card": "aaa1", "rating": 5}"#,
            r#"{"card": "aaa1"}"#,
            r#"{"card": "aaa1", "rating": 3, "mood": "good"}"#,
            r#"{"card": "aaa1", "rating": 3, "response": ["a"]}"#,
            r#"{"card": {"atoms": ["aaa1"]}, "rating": 3}"#,
        ] {
            assert!(ReviewRequest::parse(text).is_err(), "accepted {text}");
        }
        let (_tmp, store, notes) = store();
        for card in ["aaa2", "zzz9", "aaa1-1"] {
            assert!(
                review(&store, &notes, &[request(card, 3)], &now(), false).is_err(),
                "accepted {card}"
            );
        }
        assert!(logged(&store).is_empty());
    }

    #[test]
    fn reset_only_touches_cards_that_are_not_new() {
        let (_tmp, store, notes) = store();
        review(&store, &notes, &[request("aaa1", 3)], &now(), false).unwrap();
        let all = notes.cards().collect::<Vec<_>>();
        let reset_ids = reset(&store, &all, &memories(&store), &now(), false).unwrap();
        assert_eq!(reset_ids, [CardId::parse("aaa1").unwrap()]);
        assert!(matches!(logged(&store).last(), Some(Event::Reset(_))));
    }

    #[test]
    fn suspend_and_unsuspend_skip_drafts_and_cards_already_there() {
        let (_tmp, store, notes) = store();
        let all = notes.cards().collect::<Vec<_>>();
        let suspended = suspend(&store, &all, &memories(&store), true, &now(), false).unwrap();
        assert_eq!(suspended.len(), 3);
        let again = suspend(&store, &all, &memories(&store), true, &now(), false).unwrap();
        assert!(again.is_empty());
        let back = suspend(&store, &all[..1], &memories(&store), false, &now(), false).unwrap();
        assert_eq!(back, [CardId::parse("aaa1").unwrap()]);
        assert!(!memories(&store).is_suspended(&CardId::parse("aaa1").unwrap()));
    }

    #[test]
    fn history_moves_only_from_an_orphan_to_an_untouched_card() {
        let (_tmp, store, notes) = store();
        let id = |text: &str| CardId::parse(text).unwrap();
        log::append(
            &store,
            &[Event::Review(Review {
                t: "2026-10-01T09:00:00+09:00".to_owned(),
                card: id("aaa1-1"),
                rating: 3,
                response: None,
                by: By::Human,
                hash: String::new(),
            })],
        )
        .unwrap();
        review(&store, &notes, &[request("aaa2-1", 3)], &now(), false).unwrap();
        let memories = memories(&store);
        let attempt = |from: &str, to: &str| {
            move_history(&store, &notes, &memories, &id(from), &id(to), &now(), true)
        };
        assert!(attempt("aaa1", "aaa2-2").is_err(), "a card of the store");
        assert!(attempt("zzz1", "aaa2-2").is_err(), "no reviews");
        assert!(attempt("aaa1-1", "zzz1").is_err(), "not a card");
        assert!(attempt("aaa1-1", "aaa2-1").is_err(), "has events");
        assert!(attempt("aaa1-1", "aaa1").is_ok());
    }
}
