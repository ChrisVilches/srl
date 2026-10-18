//! Recording reviews and resets (see `docs/log.md` and `docs/scheduling.md`).

use jiff::Zoned;
use serde::Deserialize;
use serde_json::Value;

use crate::log::{self, CardUsed};
use crate::{
    Atom, Atoms, By, Card, Error, Event, Id, Memories, Reset, Result, Review, State, Store,
};

/// What `srl review --stdin` reads: a card, its rating, and optionally the
/// response and who recorded the review.
#[derive(Debug, Clone)]
pub struct ReviewRequest {
    card: Card,
    rating: u8,
    response: Option<String>,
    by: By,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    card: Value,
    rating: u8,
    response: Option<String>,
    by: Option<By>,
}

impl ReviewRequest {
    /// Reads `{"card": …, "rating": N, "response": "…", "by": "human"}`.
    /// `response` and `by` may be left out; `by` is then `human`.
    pub fn parse(text: &str, atoms: &Atoms) -> Result<Self> {
        let invalid = |reason: String| Error::InvalidReview { reason };
        let raw = serde_json::from_str::<RawRequest>(text)
            .map_err(|error| invalid(format!("it is not a review ({error})")))?;
        let card = Card::from_value(raw.card, atoms)
            .map_err(|reason| invalid(format!("the card: {reason}")))?;
        Self::new(card, raw.rating, raw.response, raw.by.unwrap_or(By::Human))
    }

    /// A review of a card that is already checked. The rating must be 1–4.
    pub fn new(card: Card, rating: u8, response: Option<String>, by: By) -> Result<Self> {
        check_rating(rating).map_err(|reason| Error::InvalidReview { reason })?;
        Ok(Self {
            card,
            rating,
            response,
            by,
        })
    }
}

fn check_rating(rating: u8) -> std::result::Result<(), String> {
    if (1..=4).contains(&rating) {
        Ok(())
    } else {
        Err(format!("the rating is {rating}, but must be 1 to 4"))
    }
}

/// Records a review of a card: one event per atom of the card, all with the
/// same rating. Returns the events; with `dry_run`, they are not written.
pub fn review(
    store: &Store,
    atoms: &Atoms,
    request: &ReviewRequest,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<Event>> {
    let card = &request.card;
    let used = CardUsed::of(card.drill(), card.reference());
    let events = card
        .atoms()
        .iter()
        .map(|id| {
            let atom = known(atoms, id)?;
            Ok(Event::Review(Review {
                t: log::time(now),
                atom: id.clone(),
                rating: request.rating,
                card: Some(used.clone()),
                response: request.response.clone(),
                by: request.by,
                hash: log::hash(atom),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    write(store, events, dry_run)
}

/// Records a review of each atom in `ids` without a card (`srl review ID…
/// --rating N`). Every ID must be an atom of the store.
pub fn review_ids(
    store: &Store,
    atoms: &Atoms,
    ids: &[Id],
    rating: u8,
    by: By,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<Event>> {
    check_rating(rating).map_err(|reason| Error::InvalidReview { reason })?;
    let events = ids
        .iter()
        .map(|id| {
            let atom = known(atoms, id)?;
            Ok(Event::Review(Review {
                t: log::time(now),
                atom: id.clone(),
                rating,
                card: None,
                response: None,
                by,
                hash: log::hash(atom),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    write(store, events, dry_run)
}

/// Makes each atom of `selected` that is not new again new, with a `reset`
/// event, and returns their IDs. New atoms are left alone: there is nothing
/// to reset.
pub fn reset(
    store: &Store,
    selected: &[&Atom],
    memories: &Memories,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<Id>> {
    let events = selected
        .iter()
        .filter(|atom| memories.state(atom.id()) != State::New)
        .map(|atom| {
            Event::Reset(Reset {
                t: log::time(now),
                atom: atom.id().clone(),
            })
        })
        .collect::<Vec<_>>();
    let ids = events.iter().map(|event| event.atom().clone()).collect();
    write(store, events, dry_run)?;
    Ok(ids)
}

fn known<'a>(atoms: &'a Atoms, id: &Id) -> Result<&'a Atom> {
    atoms
        .get(id)
        .ok_or_else(|| Error::UnknownId { id: id.clone() })
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
    use crate::{Config, Log, Lookup};

    fn store() -> (TempDir, Store, Atoms) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        for (name, id) in [("a.md", "aaaaa1"), ("b.md", "aaaaa2")] {
            std::fs::write(
                store.atom_file(name),
                format!("---\nid: {id}\ntitle: {name}\n---\n"),
            )
            .unwrap();
        }
        let atoms = Atoms::load(&store).unwrap();
        (tmp, store, atoms)
    }

    fn now() -> Zoned {
        "2026-10-26T09:12:31.5+09:00[+09:00]".parse().unwrap()
    }

    fn logged(store: &Store) -> Vec<Event> {
        let log = Log::load(store, &Config::default().schedule).unwrap();
        assert!(log.problems.is_empty(), "{:?}", log.problems);
        log.entries.into_iter().map(|entry| entry.event).collect()
    }

    #[test]
    fn a_card_about_two_atoms_gives_two_events() {
        let (_tmp, store, atoms) = store();
        let text = r#"{"card": {"atoms": ["aaaaa1", "aaaaa2"], "drill": "synthesis", "ref": "synthesis:1",
            "prompt": "q", "answer": "a"}, "rating": 2, "by": "agent"}"#;
        let request = ReviewRequest::parse(text, &atoms).unwrap();
        let events = review(&store, &atoms, &request, &now(), false).unwrap();
        assert_eq!(logged(&store), events);
        let Event::Review(first) = &events[0] else {
            panic!("expected a review");
        };
        assert_eq!(first.t, "2026-10-26T09:12:31+09:00");
        assert_eq!(first.rating, 2);
        assert_eq!(first.by, By::Agent);
        assert_eq!(first.response, None);
        assert_eq!(first.hash.len(), 12);
        assert_eq!(events[1].atom().as_str(), "aaaaa2");
    }

    #[test]
    fn the_rating_and_response_are_kept_as_given() {
        let (_tmp, store, atoms) = store();
        let text = r#"{"card": {"atoms": ["aaaaa1"], "drill": "cards", "ref": "cards:0",
            "prompt": "q", "answer": "a"}, "rating": 4, "response": "my answer"}"#;
        let request = ReviewRequest::parse(text, &atoms).unwrap();
        let events = review(&store, &atoms, &request, &now(), true).unwrap();
        assert!(matches!(&events[0], Event::Review(review)
            if review.rating == 4 && review.response.as_deref() == Some("my answer")));
        assert!(logged(&store).is_empty());
    }

    #[test]
    fn bad_reviews_are_refused() {
        let (_tmp, _store, atoms) = store();
        let card = r#"{"atoms": ["aaaaa1"], "drill": "cards", "ref": "cards:0", "prompt": "q", "answer": "a"}"#;
        for text in [
            format!(r#"{{"card": {card}, "rating": 5}}"#),
            format!(r#"{{"card": {card}}}"#),
            format!(r#"{{"card": {card}, "rating": 3, "mood": "good"}}"#),
            format!(r#"{{"card": {card}, "rating": 3, "response": ["a"]}}"#),
            r#"{"card": {"atoms": ["zzzzzz"]}, "rating": 3}"#.to_owned(),
        ] {
            assert!(
                ReviewRequest::parse(&text, &atoms).is_err(),
                "accepted {text}"
            );
        }
    }

    #[test]
    fn reviews_without_a_card_need_known_atoms() {
        let (_tmp, store, atoms) = store();
        let ids = [Id::parse("aaaaa1").unwrap()];
        let events = review_ids(&store, &atoms, &ids, 3, By::Human, &now(), false).unwrap();
        assert!(matches!(&events[0], Event::Review(review) if review.card.is_none()));
        let unknown = [Id::parse("zzzzzz").unwrap()];
        assert!(review_ids(&store, &atoms, &unknown, 3, By::Human, &now(), false).is_err());
        assert!(review_ids(&store, &atoms, &ids, 0, By::Human, &now(), false).is_err());
    }

    #[test]
    fn reset_only_touches_atoms_that_are_not_new() {
        let (_tmp, store, atoms) = store();
        review_ids(
            &store,
            &atoms,
            &[Id::parse("aaaaa1").unwrap()],
            3,
            By::Human,
            &now(),
            false,
        )
        .unwrap();
        let config = Config::default();
        let log = Log::load(&store, &config.schedule).unwrap();
        let memories = Memories::replay(&log, &config, jiff::civil::date(2026, 10, 26)).unwrap();
        let all = atoms.atoms.iter().collect::<Vec<_>>();
        let reset_ids = reset(&store, &all, &memories, &now(), false).unwrap();
        assert_eq!(reset_ids, [Id::parse("aaaaa1").unwrap()]);
        assert!(matches!(logged(&store).last(), Some(Event::Reset(_))));
    }
}
