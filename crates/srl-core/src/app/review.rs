//! Recording reviews, resets and suspensions (see `docs/log.md` and
//! `docs/scheduling.md`).

use jiff::Zoned;

use crate::log;
use crate::{
    CardEvent, CardId, Catalog, Error, Event, Item, Memories, Place, Result, Review, Store, Suspend,
};

fn check_rating(rating: u8) -> Result<()> {
    if (1..=4).contains(&rating) {
        Ok(())
    } else {
        Err(Error::InvalidReview {
            reason: format!("the rating is {rating}, but must be 1 to 4"),
        })
    }
}

/// Records `rating` for `card`, which must be a card of the store. Returns
/// the event; with `dry_run`, it is not written.
pub fn review(
    store: &Store,
    catalog: &Catalog,
    card: &CardId,
    rating: u8,
    now: &Zoned,
    dry_run: bool,
) -> Result<Event> {
    check_rating(rating)?;
    let (item, _) = catalog.card(card).ok_or_else(|| Error::UnknownCard {
        id: card.to_string(),
    })?;
    let event = Event::Review(Review {
        t: log::time(now),
        card: card.clone(),
        rating,
        hash: log::hash(item),
    });
    write(store, vec![event.clone()], dry_run)?;
    Ok(event)
}

/// Makes each of `selected` that is not new again new, with a `reset`
/// event, and returns their IDs.
pub fn reset(
    store: &Store,
    selected: &[(&Item, &CardId)],
    memories: &Memories,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<CardId>> {
    let ids = selected
        .iter()
        .map(|(_, card)| *card)
        .filter(|id| memories.place(id) != Place::New)
        .cloned()
        .collect::<Vec<_>>();
    write(store, card_events(&ids, now, Event::Reset), dry_run)?;
    Ok(ids)
}

/// Suspends each of `selected` that is active, with `reason` if given, and
/// returns their IDs. With a reason, cards already suspended are suspended
/// again, so that it replaces theirs. Drafts are left alone.
pub fn suspend(
    store: &Store,
    selected: &[(&Item, &CardId)],
    memories: &Memories,
    reason: Option<&str>,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<CardId>> {
    let ids = selected
        .iter()
        .filter(|(item, card)| {
            !item.is_draft() && (reason.is_some() || !memories.is_suspended(card))
        })
        .map(|(_, card)| (*card).clone())
        .collect::<Vec<_>>();
    let events = ids
        .iter()
        .map(|id| {
            Event::Suspend(Suspend {
                t: log::time(now),
                card: id.clone(),
                reason: reason.map(str::to_owned),
            })
        })
        .collect();
    write(store, events, dry_run)?;
    Ok(ids)
}

/// Unsuspends each of `selected` that is suspended, and returns their IDs.
/// Drafts are left alone.
pub fn unsuspend(
    store: &Store,
    selected: &[(&Item, &CardId)],
    memories: &Memories,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<CardId>> {
    let ids = selected
        .iter()
        .filter(|(item, card)| !item.is_draft() && memories.is_suspended(card))
        .map(|(_, card)| (*card).clone())
        .collect::<Vec<_>>();
    write(store, card_events(&ids, now, Event::Unsuspend), dry_run)?;
    Ok(ids)
}

/// One event made by `kind` for each card of `ids`, at `now`.
fn card_events(ids: &[CardId], now: &Zoned, kind: fn(CardEvent) -> Event) -> Vec<Event> {
    ids.iter()
        .map(|id| {
            kind(CardEvent {
                t: log::time(now),
                card: id.clone(),
            })
        })
        .collect()
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
    use crate::{Config, History, Index, Log, Lookup};

    fn store() -> (TempDir, Store, Catalog) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        std::fs::write(
            store.file("a.jsonl"),
            "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"aaa2\", \"Q\": \"{{c1::x}} {{c2::y}}\"}\n{\"id\": \"aaa3\", \"draft\": true, \"Q\": \"d\", \"A\": \"e\"}\n",
        )
        .unwrap();
        let catalog = Catalog::load(&store, &Config::default(), &mut Index::open(&store)).unwrap();
        (tmp, store, catalog)
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

    fn id(text: &str) -> CardId {
        CardId::parse(text).unwrap()
    }

    #[test]
    fn a_review_is_recorded_for_its_card() {
        let (_tmp, store, catalog) = store();
        let event = review(&store, &catalog, &id("aaa2.2"), 2, &now(), false).unwrap();
        assert_eq!(logged(&store), std::slice::from_ref(&event));
        let Event::Review(recorded) = &event else {
            panic!("expected a review");
        };
        assert_eq!(recorded.t, "2026-10-26T09:12:31+09:00");
        assert_eq!(recorded.card.to_string(), "aaa2.2");
        assert_eq!(recorded.rating, 2);
        assert_eq!(recorded.hash.len(), 12);
    }

    #[test]
    fn bad_reviews_are_refused() {
        let (_tmp, store, catalog) = store();
        for (card, rating) in [
            ("aaa1", 0),
            ("aaa1", 5),
            ("aaa2", 3),
            ("zzz9", 3),
            ("aaa1.1", 3),
        ] {
            assert!(
                review(&store, &catalog, &id(card), rating, &now(), false).is_err(),
                "accepted {card} with {rating}"
            );
        }
        assert_eq!(logged(&store), [] as [Event; 0]);
    }

    #[test]
    fn reset_only_touches_cards_that_are_not_new() {
        let (_tmp, store, catalog) = store();
        review(&store, &catalog, &id("aaa1"), 3, &now(), false).unwrap();
        let all = catalog.cards().collect::<Vec<_>>();
        let reset_ids = reset(&store, &all, &memories(&store), &now(), false).unwrap();
        assert_eq!(reset_ids, [CardId::parse("aaa1").unwrap()]);
        assert!(matches!(logged(&store).last(), Some(Event::Reset(_))));
    }

    #[test]
    fn suspend_and_unsuspend_skip_drafts_and_cards_already_there() {
        let (_tmp, store, catalog) = store();
        let all = catalog.cards().collect::<Vec<_>>();
        let suspended = suspend(&store, &all, &memories(&store), None, &now(), false).unwrap();
        assert_eq!(suspended.len(), 3);
        let again = suspend(&store, &all, &memories(&store), None, &now(), false).unwrap();
        assert_eq!(again, [] as [CardId; 0]);
        let back = unsuspend(&store, &all[..1], &memories(&store), &now(), false).unwrap();
        assert_eq!(back, [CardId::parse("aaa1").unwrap()]);
        assert!(!memories(&store).is_suspended(&CardId::parse("aaa1").unwrap()));
        let again = unsuspend(&store, &all[..1], &memories(&store), &now(), false).unwrap();
        assert_eq!(again, [] as [CardId; 0]);
    }

    #[test]
    fn a_reason_replaces_the_reason_of_cards_already_suspended() {
        let (_tmp, store, catalog) = store();
        let all = catalog.cards().collect::<Vec<_>>();
        let id = CardId::parse("aaa1").unwrap();
        suspend(&store, &all[..1], &memories(&store), None, &now(), false).unwrap();
        assert_eq!(memories(&store).suspend_reason(&id), None);
        let again = suspend(&store, &all, &memories(&store), Some("ugly"), &now(), false).unwrap();
        assert_eq!(again.len(), 3);
        assert_eq!(
            memories(&store).suspend_reason(&id).as_deref(),
            Some("ugly")
        );
        unsuspend(&store, &all[..1], &memories(&store), &now(), false).unwrap();
        assert_eq!(memories(&store).suspend_reason(&id), None);
    }
}
