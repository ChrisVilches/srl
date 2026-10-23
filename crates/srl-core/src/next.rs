//! Choosing the next card (see `docs/scheduling.md`, "Choosing the next
//! card" and "Siblings").

use std::collections::HashMap;

use crate::{Card, CardId, Config, Item, ItemId, Memories, Notes, Selector, State, Status};

/// The next card among those that match `selector` (plus
/// `status:active`), if there is one.
///
/// Due cards come first, by due day and then card ID; then new cards, in
/// the order they are written, if fewer than `new_per_day` cards had their
/// first review today. The first of those that is its item's pick under the
/// sibling rule is the one.
pub fn next<'a>(
    notes: &'a Notes,
    memories: &Memories,
    selector: Selector,
    config: &Config,
) -> Option<&'a Card> {
    let selector = selector.and_status(Status::Active);
    let limit = usize::try_from(config.schedule.new_per_day).expect("a u32 should fit in a usize");
    let new_allowed = memories.new_today() < limit;
    let eligible = selector
        .select(notes.cards(), memories)
        .into_iter()
        .filter(|(_, card)| match memories.state(card.id()) {
            State::Due => true,
            State::New => new_allowed,
            State::Young | State::Mature => false,
        })
        .collect::<Vec<_>>();
    let picks = picks(&eligible, memories);
    let is_pick = |item: &ItemId, card: &Card| picks.get(item) == Some(&card.id());
    let mut due = eligible
        .iter()
        .filter(|(_, card)| memories.state(card.id()) == State::Due)
        .collect::<Vec<_>>();
    due.sort_by_key(|(_, card)| (memories.get(card.id()).due, card.id().clone()));
    let new = eligible
        .iter()
        .filter(|(_, card)| memories.state(card.id()) == State::New);
    due.into_iter()
        .chain(new)
        .find(|(item, card)| is_pick(item.id(), card))
        .map(|(_, card)| *card)
}

/// The card each item offers today under the sibling rule: none if any of
/// its cards was reviewed today, else its eligible card reviewed longest
/// ago, never reviewed first, ties to the lowest card ID.
fn picks<'a>(
    eligible: &[(&'a Item, &'a Card)],
    memories: &Memories,
) -> HashMap<&'a ItemId, &'a CardId> {
    let mut picks: HashMap<&ItemId, &CardId> = HashMap::new();
    for (item, card) in eligible {
        let reviewed_today = item
            .cards()
            .iter()
            .any(|sibling| memories.reviewed_today(sibling.id()));
        if reviewed_today {
            continue;
        }
        let key = |id: &CardId| (memories.get(id).seen.map(|seen| seen.position), id.clone());
        picks
            .entry(item.id())
            .and_modify(|pick| {
                if key(card.id()) < key(pick) {
                    *pick = card.id();
                }
            })
            .or_insert(card.id());
    }
    picks
}

#[cfg(test)]
mod tests {
    use jiff::civil::{Date, date};
    use tempfile::TempDir;

    use super::*;
    use crate::{By, Entry, Event, Log, Lookup, Review, Store};

    const DAY: Date = Date::constant(2026, 10, 26);

    fn notes(files: &[(&str, &str)]) -> (TempDir, Notes) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        for (path, text) in files {
            std::fs::write(store.note_file(path), text).unwrap();
        }
        let notes = Notes::load(&store, &Config::default()).unwrap();
        assert!(notes.problems().is_empty(), "{:?}", notes.problems());
        (tmp, notes)
    }

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

    fn next_card(
        notes: &Notes,
        entries: Vec<Entry>,
        config: &Config,
        selector: &str,
    ) -> Option<String> {
        let log = Log {
            entries,
            problems: Vec::new(),
        };
        let memories = Memories::replay(&log, config, DAY).unwrap();
        let selector = Selector::parse(selector).unwrap();
        next(notes, &memories, selector, config).map(|card| card.id().to_string())
    }

    fn store() -> (TempDir, Notes) {
        notes(&[
            (
                "a.md",
                "<!-- srl id:aaa1 -->\nQ: {{c1::x}} {{c2::y}} {{c10::z}}\n\n<!-- srl id:aaa2 -->\nQ: b\nA: c\n",
            ),
            ("b.md", "<!-- srl id:aaa3 draft -->\nQ: c\nA: d\n"),
        ])
    }

    #[test]
    fn new_cards_come_in_written_order_and_drafts_never() {
        let (_tmp, notes) = store();
        assert_eq!(
            next_card(&notes, Vec::new(), &Config::default(), "").as_deref(),
            Some("aaa1-1")
        );
        assert_eq!(
            next_card(&notes, Vec::new(), &Config::default(), "id:aaa3"),
            None
        );
    }

    #[test]
    fn due_cards_come_before_new_ones() {
        let (_tmp, notes) = store();
        let entries = vec![reviewed("aaa2", date(2026, 10, 1), 3)];
        assert_eq!(
            next_card(&notes, entries, &Config::default(), "").as_deref(),
            Some("aaa2")
        );
    }

    #[test]
    fn a_sibling_reviewed_today_holds_back_the_others() {
        let (_tmp, notes) = store();
        let entries = vec![reviewed("aaa1-1", DAY, 3)];
        assert_eq!(
            next_card(&notes, entries, &Config::default(), "").as_deref(),
            Some("aaa2")
        );
        let entries = vec![reviewed("aaa1-1", DAY, 3)];
        assert_eq!(
            next_card(&notes, entries, &Config::default(), "id:aaa1"),
            None
        );
    }

    #[test]
    fn new_siblings_come_one_a_day_in_id_order() {
        let (_tmp, notes) = store();
        let entries = vec![reviewed("aaa1-1", date(2026, 10, 25), 3)];
        assert_eq!(
            next_card(&notes, entries, &Config::default(), "id:aaa1").as_deref(),
            Some("aaa1-2")
        );
        let entries = vec![
            reviewed("aaa1-1", date(2026, 10, 24), 3),
            reviewed("aaa1-2", date(2026, 10, 25), 3),
        ];
        assert_eq!(
            next_card(&notes, entries, &Config::default(), "id:aaa1").as_deref(),
            Some("aaa1-10")
        );
    }

    #[test]
    fn the_sibling_reviewed_longest_ago_goes_first_so_none_starves() {
        let (_tmp, notes) = store();
        // All three are due; -2 was reviewed longest ago, then -10, then -1.
        let entries = vec![
            reviewed("aaa1-2", date(2026, 9, 1), 1),
            reviewed("aaa1-10", date(2026, 9, 2), 1),
            reviewed("aaa1-1", date(2026, 9, 3), 1),
        ];
        assert_eq!(
            next_card(&notes, entries, &Config::default(), "id:aaa1").as_deref(),
            Some("aaa1-2")
        );
    }

    #[test]
    fn a_new_sibling_goes_before_a_due_one() {
        let (_tmp, notes) = store();
        let entries = vec![
            reviewed("aaa1-1", date(2026, 9, 1), 1),
            reviewed("aaa1-2", date(2026, 9, 2), 1),
        ];
        assert_eq!(
            next_card(&notes, entries, &Config::default(), "id:aaa1").as_deref(),
            Some("aaa1-10")
        );
    }

    #[test]
    fn the_new_card_limit_stops_new_cards() {
        let (_tmp, notes) = store();
        let config = Config {
            schedule: crate::config::Schedule {
                new_per_day: 1,
                ..crate::config::Schedule::default()
            },
            ..Config::default()
        };
        let entries = vec![reviewed("aaa2", DAY, 3)];
        assert_eq!(next_card(&notes, entries, &config, ""), None);
    }
}
