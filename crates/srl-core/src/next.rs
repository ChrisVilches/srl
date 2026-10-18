//! Choosing the next card (see `docs/scheduling.md`, "Choosing the next
//! card").

use std::time::Duration;

use jiff::civil::Date;

use crate::{Atom, Atoms, Card, Config, Drills, Error, Memories, Result, Selector, State, Status};

/// What `srl next` needs besides the atoms and their memories.
#[derive(Debug, Clone, Copy)]
pub struct NextOptions<'a> {
    /// Only cards from this drill (`--drill NAME`).
    pub drill: Option<&'a str>,
    pub day: Date,
    pub timeout: Duration,
}

/// The next card for the atoms that match `selector` (plus `status:active`),
/// if there is one, and the problems met on the way (failing drills,
/// invalid cards).
///
/// Due atoms come first, by due day and then ID; then new atoms, by path, if
/// fewer than `new_per_day` atoms had their first review today. The first
/// of those atoms that has any card gives the card its rotation picks.
pub fn next(
    atoms: &Atoms,
    memories: &Memories,
    drills: &Drills,
    selector: Selector,
    config: &Config,
    options: NextOptions<'_>,
) -> Result<(Option<Card>, Vec<Error>)> {
    if let Some(name) = options.drill
        && drills.get(name).is_none()
    {
        return Err(Error::UnknownDrill {
            name: name.to_owned(),
        });
    }
    let selector = selector.and_status(Status::Active);
    let matching = atoms.select(&selector, Some(memories));
    let mut due = matching
        .iter()
        .copied()
        .filter(|atom| memories.state(atom.id()) == State::Due)
        .collect::<Vec<_>>();
    due.sort_by_key(|atom| (memories.get(atom.id()).due, atom.id().clone()));
    let limit = usize::try_from(config.schedule.new_per_day).expect("a u32 should fit in a usize");
    let new = matching
        .iter()
        .copied()
        .filter(|atom| memories.state(atom.id()) == State::New)
        .filter(|_| memories.new_today() < limit);
    let mut problems = Vec::new();
    for atom in due.into_iter().chain(new) {
        let made = drills.cards_of(atom, atoms, options.day, options.timeout, options.drill);
        let cards = made
            .into_iter()
            .flat_map(|made| {
                problems.extend(made.problems);
                made.cards
            })
            .collect::<Vec<_>>();
        if let Some(card) = least_recently_used(atom, cards, memories) {
            return Ok((Some(card), problems));
        }
    }
    Ok((None, problems))
}

/// The card of `atom` whose ref was used least recently, cards never used
/// first; ties by drill name, then by ref.
fn least_recently_used(atom: &Atom, cards: Vec<Card>, memories: &Memories) -> Option<Card> {
    let memory = memories.get(atom.id());
    cards.into_iter().min_by_key(|card| {
        (
            memory.last_used(card.reference()),
            card.drill().to_owned(),
            card.reference().to_owned(),
        )
    })
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use tempfile::TempDir;

    use super::*;
    use crate::{By, CardUsed, Entry, Event, Id, Log, Lookup, Review, Search, Store};

    const DAY: Date = Date::constant(2026, 10, 26);

    struct Fixture {
        _tmp: TempDir,
        atoms: Atoms,
        drills: Drills,
    }

    fn fixture(files: &[(&str, &str)]) -> Fixture {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        for (path, text) in files {
            std::fs::write(store.atom_file(path), text).unwrap();
        }
        let atoms = Atoms::load(&store).unwrap();
        let drills = Drills::find(&store, &Search::default()).0;
        Fixture {
            _tmp: tmp,
            atoms,
            drills,
        }
    }

    fn reviewed(id: &str, day: Date, rating: u8, reference: &str) -> Entry {
        Entry {
            event: Event::Review(Review {
                t: format!("{day}T12:00:00+00:00"),
                atom: Id::parse(id).unwrap(),
                rating,
                card: Some(CardUsed {
                    drill: reference.split(':').next().unwrap().to_owned(),
                    reference: reference.to_owned(),
                }),
                response: None,
                by: By::Human,
                hash: String::new(),
            }),
            day,
        }
    }

    fn next_card(
        fixture: &Fixture,
        entries: Vec<Entry>,
        config: &Config,
        selector: &str,
        drill: Option<&str>,
    ) -> Option<Card> {
        let log = Log {
            entries,
            problems: Vec::new(),
        };
        let memories = Memories::replay(&log, config, DAY).unwrap();
        let options = NextOptions {
            drill,
            day: DAY,
            timeout: Duration::from_secs(10),
        };
        let selector = Selector::parse(selector).unwrap();
        let (card, problems) = next(
            &fixture.atoms,
            &memories,
            &fixture.drills,
            selector,
            config,
            options,
        )
        .unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        card
    }

    fn atoms() -> Fixture {
        fixture(&[
            (
                "a.md",
                "---\nid: aaaaa1\ntitle: a\ncards: [{q: x, a: y}, {q: z, a: w}]\ncloze: ['{{x}}']\n---\n",
            ),
            (
                "b.md",
                "---\nid: aaaaa2\ntitle: b\ncards: [{q: b, a: c}]\n---\n",
            ),
            (
                "c.md",
                "---\nid: aaaaa3\ntitle: c\nstatus: draft\ncards: [{q: c, a: d}]\n---\n",
            ),
            ("d.md", "---\nid: aaaaa4\ntitle: d\n---\nNotes only.\n"),
        ])
    }

    #[test]
    fn new_atoms_come_by_path_and_drafts_never() {
        let fixture = atoms();
        let card = next_card(&fixture, Vec::new(), &Config::default(), "", None).unwrap();
        assert_eq!(card.atoms()[0].as_str(), "aaaaa1");
        assert_eq!(card.reference(), "cards:0");
        assert!(next_card(&fixture, Vec::new(), &Config::default(), "id:aaaaa3", None).is_none());
    }

    #[test]
    fn an_atom_without_facets_is_skipped() {
        let fixture = atoms();
        assert!(next_card(&fixture, Vec::new(), &Config::default(), "id:aaaaa4", None).is_none());
    }

    #[test]
    fn due_atoms_come_before_new_ones() {
        let fixture = atoms();
        let entries = vec![reviewed("aaaaa2", date(2026, 10, 1), 3, "cards:0")];
        let card = next_card(&fixture, entries, &Config::default(), "", None).unwrap();
        assert_eq!(card.atoms()[0].as_str(), "aaaaa2");
    }

    #[test]
    fn an_atom_reviewed_today_is_not_next_again() {
        let fixture = atoms();
        let entries = vec![reviewed("aaaaa1", DAY, 3, "cards:0")];
        let card = next_card(&fixture, entries, &Config::default(), "", None).unwrap();
        assert_eq!(card.atoms()[0].as_str(), "aaaaa2");
    }

    #[test]
    fn the_least_recently_used_card_comes_next() {
        let fixture = atoms();
        let entries = vec![
            reviewed("aaaaa1", date(2026, 9, 1), 3, "cards:0"),
            reviewed("aaaaa1", date(2026, 9, 10), 3, "cloze:0:1"),
            reviewed("aaaaa1", date(2026, 9, 20), 3, "cards:1"),
            reviewed("aaaaa1", date(2026, 10, 1), 1, "cards:0"),
        ];
        let card = next_card(&fixture, entries, &Config::default(), "id:aaaaa1", None).unwrap();
        assert_eq!(card.reference(), "cloze:0:1");
    }

    #[test]
    fn the_new_atom_limit_stops_new_atoms() {
        let fixture = atoms();
        let config = Config {
            schedule: crate::config::Schedule {
                new_per_day: 1,
                ..crate::config::Schedule::default()
            },
            ..Config::default()
        };
        let entries = vec![reviewed("aaaaa1", DAY, 3, "cards:0")];
        assert!(next_card(&fixture, entries, &config, "", None).is_none());
    }

    #[test]
    fn drill_restricts_the_pool_and_skips_atoms_without_its_cards() {
        let fixture = atoms();
        let card = next_card(&fixture, Vec::new(), &Config::default(), "", Some("cloze")).unwrap();
        assert_eq!(card.reference(), "cloze:0:1");
        assert!(
            next_card(
                &fixture,
                Vec::new(),
                &Config::default(),
                "id:aaaaa2",
                Some("cloze")
            )
            .is_none()
        );
    }
}
