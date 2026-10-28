//! Choosing the next card (see `docs/scheduling.md`, "Choosing the next
//! card" and "Siblings").

use std::collections::HashMap;

use jiff::civil::Date;

use crate::{
    Card, CardId, Config, Item, ItemId, Memories, Notes, Result, Selector, State, Status, Store,
    random,
};

/// The next card among those that match `selector`, made by its kind to
/// be shown on the study day `day`, with its item, if there is one. One
/// card is picked, with `seed` choosing how many to skip; if it can't be
/// made, that is the error, and no other card is tried (see `docs/cli.md`,
/// "Studying").
pub fn next<'a>(
    store: &Store,
    notes: &'a Notes,
    memories: &Memories,
    selector: &Selector,
    config: &Config,
    day: Date,
    seed: u64,
) -> Result<Option<(&'a Item, Card)>> {
    let Some((item, card)) = pick(notes, memories, selector.clone(), config, seed) else {
        return Ok(None);
    };
    Ok(Some((item, notes.make_card(store, item, card, day)?)))
}

/// The next card among those that match `selector` (plus
/// `status:active`), with its item, if there is one.
///
/// Due cards come first, by due day and then card ID; then new cards, in
/// the order they are written, if fewer than `new_per_day` cards had their
/// first review today. Of those, the ones that are their group's pick
/// under the sibling rule are the candidates, and the card is one of the
/// first `max_skip + 1`, chosen uniformly from `seed`.
fn pick<'a>(
    notes: &'a Notes,
    memories: &Memories,
    selector: Selector,
    config: &Config,
    seed: u64,
) -> Option<(&'a Item, &'a CardId)> {
    let selector = selector.and_status(Status::Active);
    let limit = usize::try_from(config.schedule.new_per_day).expect("a u32 should fit in a usize");
    let new_allowed = memories.new_today() < limit;
    let eligible = selector
        .select(notes.cards(), memories)
        .into_iter()
        .filter(|(_, card)| match memories.state(card) {
            State::Due => true,
            State::New => new_allowed,
            State::Young | State::Mature => false,
        })
        .collect::<Vec<_>>();
    let picks = picks(&eligible, memories);
    let is_pick =
        |item: &Item, card: &CardId| picks.get(&(item.id(), item.group(card))) == Some(&card);
    let mut due = eligible
        .iter()
        .filter(|(_, card)| memories.state(card) == State::Due)
        .collect::<Vec<_>>();
    due.sort_by_key(|(_, card)| (memories.get(card).due, (*card).clone()));
    let new = eligible
        .iter()
        .filter(|(_, card)| memories.state(card) == State::New);
    let window = usize::try_from(config.schedule.max_skip)
        .expect("a u32 should fit in a usize")
        .saturating_add(1);
    let candidates = due
        .into_iter()
        .chain(new)
        .filter(|(item, card)| is_pick(item, card))
        .take(window)
        .collect::<Vec<_>>();
    let number = random::numbers(seed).next().expect("the numbers never end");
    let len = u64::try_from(candidates.len()).expect("a usize should fit in a u64");
    // No remainder when there are no candidates, and so no card.
    let skip = usize::try_from(number.checked_rem(len)?).expect("it is less than a usize");
    candidates.get(skip).map(|&&pick| pick)
}

/// The card each group of siblings offers today under the sibling rule,
/// by item and group: none if any card of the group was reviewed today,
/// else its eligible card reviewed longest ago, never reviewed first, ties
/// to the lowest card ID.
fn picks<'a>(
    eligible: &[(&'a Item, &'a CardId)],
    memories: &Memories,
) -> HashMap<(&'a ItemId, usize), &'a CardId> {
    let mut picks: HashMap<(&ItemId, usize), &CardId> = HashMap::new();
    for (item, card) in eligible {
        let group = item.group(card);
        let reviewed_today = item
            .siblings(group)
            .iter()
            .any(|sibling| memories.reviewed_today(sibling));
        if reviewed_today {
            continue;
        }
        let key = |id: &CardId| (memories.get(id).seen.map(|seen| seen.position), id.clone());
        picks
            .entry((item.id(), group))
            .and_modify(|pick| {
                if key(card) < key(pick) {
                    *pick = card;
                }
            })
            .or_insert(card);
    }
    picks
}

#[cfg(test)]
mod tests {
    use jiff::civil::{Date, date};
    use tempfile::TempDir;

    use super::*;
    use crate::{By, Entry, Event, Index, Log, Lookup, Review, Store};

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
        let notes = Notes::load(&store, &Config::default(), &mut Index::open(&store)).unwrap();
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
        entries: &[Entry],
        config: &Config,
        selector: &str,
    ) -> Option<String> {
        picked(notes, entries, config, selector, 0)
    }

    /// The card picked with `seed`.
    fn picked(
        notes: &Notes,
        entries: &[Entry],
        config: &Config,
        selector: &str,
        seed: u64,
    ) -> Option<String> {
        let log = Log {
            entries: entries.to_vec(),
            problems: Vec::new(),
        };
        let memories = Memories::replay(&log, config, DAY).unwrap();
        let selector = Selector::parse(selector, &Config::default()).unwrap();
        pick(notes, &memories, selector, config, seed).map(|(_, card)| card.to_string())
    }

    fn skipping(max_skip: u32) -> Config {
        Config {
            schedule: crate::config::Schedule {
                max_skip,
                ..crate::config::Schedule::default()
            },
            ..Config::default()
        }
    }

    /// The cards picked with seeds 0 to 99, without repeats, sorted.
    fn picks_over_seeds(
        notes: &Notes,
        entries: &[Entry],
        config: &Config,
        selector: &str,
    ) -> Vec<String> {
        let mut cards = (0..100)
            .filter_map(|seed| picked(notes, entries, config, selector, seed))
            .collect::<Vec<_>>();
        cards.sort();
        cards.dedup();
        cards
    }

    #[test]
    fn max_skip_picks_among_the_first_cards() {
        let (_tmp, notes) = store();
        // In order: aaa2 (due), then aaa1-1 (new, its group's pick); aaa1-2
        // and aaa1-10 are its siblings, never candidates.
        let entries = vec![reviewed("aaa2", date(2026, 10, 1), 3)];
        assert_eq!(
            picks_over_seeds(&notes, &entries, &skipping(0), ""),
            ["aaa2"]
        );
        assert_eq!(
            picks_over_seeds(&notes, &entries, &skipping(1), ""),
            ["aaa1-1", "aaa2"]
        );
        // Fewer candidates than the window: the range shrinks to them.
        assert_eq!(
            picks_over_seeds(&notes, &entries, &skipping(5), ""),
            ["aaa1-1", "aaa2"]
        );
    }

    #[test]
    fn max_skip_with_nothing_left_picks_nothing() {
        let (_tmp, notes) = store();
        assert_eq!(
            picks_over_seeds(&notes, &[], &skipping(3), "id:aaa3"),
            Vec::<String>::new()
        );
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
            next_card(&notes, &[], &Config::default(), "").as_deref(),
            Some("aaa1-1")
        );
        assert_eq!(next_card(&notes, &[], &Config::default(), "id:aaa3"), None);
    }

    #[test]
    fn due_cards_come_before_new_ones() {
        let (_tmp, notes) = store();
        let entries = vec![reviewed("aaa2", date(2026, 10, 1), 3)];
        assert_eq!(
            next_card(&notes, &entries, &Config::default(), "").as_deref(),
            Some("aaa2")
        );
    }

    #[test]
    fn a_sibling_reviewed_today_holds_back_the_others() {
        let (_tmp, notes) = store();
        let entries = vec![reviewed("aaa1-1", DAY, 3)];
        assert_eq!(
            next_card(&notes, &entries, &Config::default(), "").as_deref(),
            Some("aaa2")
        );
        let entries = vec![reviewed("aaa1-1", DAY, 3)];
        assert_eq!(
            next_card(&notes, &entries, &Config::default(), "id:aaa1"),
            None
        );
    }

    #[test]
    fn new_siblings_come_one_a_day_in_id_order() {
        let (_tmp, notes) = store();
        let entries = vec![reviewed("aaa1-1", date(2026, 10, 25), 3)];
        assert_eq!(
            next_card(&notes, &entries, &Config::default(), "id:aaa1").as_deref(),
            Some("aaa1-2")
        );
        let entries = vec![
            reviewed("aaa1-1", date(2026, 10, 24), 3),
            reviewed("aaa1-2", date(2026, 10, 25), 3),
        ];
        assert_eq!(
            next_card(&notes, &entries, &Config::default(), "id:aaa1").as_deref(),
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
            next_card(&notes, &entries, &Config::default(), "id:aaa1").as_deref(),
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
            next_card(&notes, &entries, &Config::default(), "id:aaa1").as_deref(),
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
        assert_eq!(next_card(&notes, &entries, &config, ""), None);
    }
}
