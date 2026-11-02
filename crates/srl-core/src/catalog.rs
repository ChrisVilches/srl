//! The catalog: every item of a store, read from its sources and checked by
//! their kinds, with the problems found (see `docs/plans/sources.md`, "The
//! core holds every item in memory").
//!
//! Everything that crosses sources happens here: item IDs unique in the
//! store, `see` naming a collection of the store, and the context of an
//! item asked of the source that owns the collection shown.

use std::collections::{HashMap, HashSet};

use jiff::civil::Date;

use crate::card::{Card, CardId};
use crate::context::{Context, ContextMode};
use crate::kind::{Answer, Asked, Cache, Kind, Programs, ToMake, check};
use crate::note::object::Object;
use crate::note::{Field, Item, ItemId, Origin};
use crate::source::{ContextRequest, Sources};
use crate::store::nfc;
use crate::{Config, Error, Index, Result, Store};

/// Every item of a store, and the problems in its sources.
#[derive(Debug)]
pub struct Notes {
    sources: Sources,
    items: Vec<Item>,
    problems: Vec<Error>,
    collections: Vec<Located>,
    unidentified: Vec<Origin>,
    programs: Programs,
}

/// A collection of the store: its address, and its source and name there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    pub(crate) address: String,
    pub(crate) source: usize,
    pub(crate) collection: String,
}

impl Notes {
    /// Reads every source of the store. Problems in sources are collected
    /// in [`Notes::problems`], in the order of the collections and lines
    /// they are about; only failing to read a source is an error here. The
    /// items of every source are checked together, so that each kind
    /// program is asked once about all of its items, and only about the
    /// items it has no answer for in `index` (see `docs/cards.md`, "How srl
    /// calls them"). Problems with the cache are reported by
    /// [`Index::problems`].
    pub fn load(store: &Store, config: &Config, index: &mut Index) -> Result<Self> {
        Self::read(store, config, &mut Cache::Use(index))
    }

    /// Like [`Notes::load`], asking kind programs about every item, and
    /// keeping their new answers in `index` (`srl check`).
    pub fn load_asking(store: &Store, config: &Config, index: &mut Index) -> Result<Self> {
        Self::read(store, config, &mut Cache::Refresh(index))
    }

    fn read(store: &Store, config: &Config, cache: &mut Cache<'_>) -> Result<Self> {
        let sources = Sources::new(store, config);
        let programs = Programs::new(store, config);
        let mut collections = Vec::new();
        let mut written = Vec::new();
        let mut problems = Vec::new();
        let mut unidentified = Vec::new();
        for (index, mounted) in sources.all().iter().enumerate() {
            collections.extend(mounted.source.collections()?.into_iter().map(|collection| {
                Located {
                    address: mounted.address(&collection.name),
                    source: index,
                    collection: collection.name,
                }
            }));
            let read = mounted.source.read()?;
            problems.extend(read.problems.into_iter().map(|problem| Error::Note {
                path: mounted.address(&problem.collection),
                line: problem.line,
                reason: problem.reason,
            }));
            for item in read.items {
                let origin = Origin {
                    source: index,
                    address: mounted.address(&item.collection),
                    collection: item.collection,
                    handle: item.handle,
                    line: item.line,
                    read: item.item,
                };
                match written_item(origin, item.written, &programs) {
                    Ok(item) => written.push(item),
                    Err(Unread::Problem(problem)) => problems.push(problem),
                    Err(Unread::Unidentified(origin)) => {
                        problems.push(Error::MissingId {
                            path: origin.address.clone(),
                            line: origin.line,
                        });
                        unidentified.push(*origin);
                    }
                }
            }
        }
        let (items, checked) = check_items(written, &programs, cache);
        problems.extend(checked);
        let (items, duplicates) = unique(items);
        problems.extend(duplicates);
        let known = collections
            .iter()
            .map(|located| nfc(&located.address))
            .collect::<HashSet<_>>();
        problems.extend(items.iter().filter_map(|item| {
            let see = item.see.as_deref()?;
            (!known.contains(&nfc(see))).then(|| Error::Note {
                path: item.origin.address.clone(),
                line: item.origin.line,
                reason: format!(
                    "`see:{see}` doesn't name a note of the store; give a path relative to the \
                     store's folder, such as japanese/grammar/wo.md"
                ),
            })
        }));
        in_place_order(&mut problems);
        Ok(Self {
            sources,
            items,
            problems,
            collections,
            unidentified,
            programs,
        })
    }

    /// The kind programs the items were read with.
    pub(crate) fn programs(&self) -> &Programs {
        &self.programs
    }

    /// The sources the items were read from.
    pub(crate) fn sources(&self) -> &Sources {
        &self.sources
    }

    /// The items read without an ID, which `srl id` gives one, in order.
    pub(crate) fn unidentified(&self) -> &[Origin] {
        &self.unidentified
    }

    /// The collection whose address is `address`, compared in NFC (see
    /// `docs/store.md`, "File names").
    pub(crate) fn located(&self, address: &str) -> Option<&Located> {
        let address = nfc(address);
        self.collections
            .iter()
            .find(|located| nfc(&located.address) == address)
    }

    /// The context of `item` in `mode` (see `docs/notes.md`, "Context"):
    /// [`Notes::contexts`] for one item.
    pub fn context(&self, item: &Item, mode: ContextMode) -> Result<Option<Context>> {
        let mut contexts = self.contexts(&[item], mode)?;
        Ok(contexts.pop().flatten())
    }

    /// The context of each of `items` in `mode`, in order: what the
    /// collection named by `see:` shows to the items that name it, or else
    /// what the item's own collection shows to its own items, each asked
    /// of the source that owns the collection, with the address of the
    /// collection it comes from. `None` in mode `none`, without asking any
    /// source, and when there is nothing to show. Each source is asked
    /// once, about all of its collections.
    pub fn contexts(&self, items: &[&Item], mode: ContextMode) -> Result<Vec<Option<Context>>> {
        if mode == ContextMode::None {
            return Ok(vec![None; items.len()]);
        }
        let wanted = items
            .iter()
            .map(|item| match item.see() {
                Some(see) => {
                    let located = self
                        .located(see)
                        .expect("`see` should have been checked to name a collection");
                    (located.source, located.collection.as_str(), see, false)
                }
                None => (
                    item.origin.source,
                    item.origin.collection.as_str(),
                    item.address(),
                    true,
                ),
            })
            .collect::<Vec<_>>();
        let mut contexts = vec![None; items.len()];
        for (index, mounted) in self.sources.all().iter().enumerate() {
            let asked = wanted
                .iter()
                .enumerate()
                .filter(|(_, (source, ..))| *source == index)
                .collect::<Vec<_>>();
            if asked.is_empty() {
                continue;
            }
            let requests = asked
                .iter()
                .map(|(_, (_, collection, _, own))| ContextRequest {
                    collection,
                    mode,
                    own: *own,
                })
                .collect::<Vec<_>>();
            let texts = mounted.source.context(&requests)?;
            for ((at, (_, _, address, _)), text) in asked.into_iter().zip(texts) {
                let slot = contexts
                    .get_mut(at)
                    .expect("a context should be asked for one of the items");
                *slot = text.map(|text| Context {
                    text,
                    address: (*address).to_owned(),
                });
            }
        }
        Ok(contexts)
    }

    /// The card `card` of `item`, made by its kind as it is about to be
    /// shown on the study day `day`: [`Notes::make_cards`] for one card.
    pub fn make_card(&self, item: &Item, card: &CardId, day: Date) -> Result<Card> {
        let mut made = self.make_cards(&[(item, card)], day)?;
        Ok(made.pop().expect("one card should have been made"))
    }

    /// The cards `cards`, each with its item, made by their kinds for the
    /// study day `day`, in order; each kind program makes all of its cards
    /// in one call (see `docs/cards.md`, "Kind programs"). Fails at the
    /// first card, in order, that can't be made: only a kind program can
    /// fail here, asking its item's source for the context, or the program
    /// itself.
    pub fn make_cards(&self, cards: &[(&Item, &CardId)], day: Date) -> Result<Vec<Card>> {
        // Only kind programs get the context, asked once for each item.
        let mut programs_items = Vec::new();
        for (item, _) in cards {
            if matches!(item.kind, Kind::Program(_))
                && !programs_items
                    .iter()
                    .any(|known: &&Item| known.id == item.id)
            {
                programs_items.push(*item);
            }
        }
        let contexts = programs_items
            .iter()
            .map(|item| &item.id)
            .zip(self.contexts(&programs_items, ContextMode::Prose)?)
            .collect::<HashMap<_, _>>();
        let wanted = cards
            .iter()
            .map(|(item, card)| ToMake {
                card,
                kind: &item.kind,
                fields: &item.fields,
                context: contexts.get(&item.id).and_then(Option::as_ref),
            })
            .collect::<Vec<_>>();
        crate::kind::make(&wanted, &self.programs, day)
            .into_iter()
            .zip(cards)
            .map(|(made, (_, card))| {
                made.map_err(|reason| Error::CardFailed {
                    card: card.to_string(),
                    reason,
                })
            })
            .collect()
    }

    /// The items, in the order they are written: by source, collection,
    /// then their order in it.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The problems in the sources. Every command that reads items stops
    /// if there are any (see `docs/notes.md`, "Errors").
    pub fn problems(&self) -> &[Error] {
        &self.problems
    }

    /// The address of every collection, in order.
    pub fn addresses(&self) -> impl Iterator<Item = &str> {
        self.collections
            .iter()
            .map(|located| located.address.as_str())
    }

    /// The item with this ID.
    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id() == id)
    }

    /// The card with this ID, and its item.
    pub fn card(&self, id: &CardId) -> Option<(&Item, &CardId)> {
        let item = self.item(id.item())?;
        item.cards
            .iter()
            .find(|card| *card == id)
            .map(|card| (item, card))
    }

    /// Every card, with its item, in the order they are written.
    pub fn cards(&self) -> impl Iterator<Item = (&Item, &CardId)> {
        self.items
            .iter()
            .flat_map(|item| item.cards.iter().map(move |card| (item, card)))
    }

    /// The items an ID given to a command stands for: the item of a card
    /// ID, or the item of an item ID (see `docs/cli.md`, "Conventions").
    pub fn item_of(&self, text: &str) -> Result<&Item> {
        let unknown = || Error::UnknownId {
            id: text.to_owned(),
        };
        let id = CardId::parse(text)?;
        let item = self.item(id.item()).ok_or_else(unknown)?;
        if id.suffix().is_some() && !item.cards.contains(&id) {
            return Err(unknown());
        }
        Ok(item)
    }
}

/// The address of every collection of the store, in order, without reading
/// any item (`srl collections`).
pub fn collections(store: &Store, config: &Config) -> Result<Vec<String>> {
    let mut addresses = Vec::new();
    for mounted in Sources::new(store, config).all() {
        addresses.extend(
            mounted
                .source
                .collections()?
                .into_iter()
                .map(|collection| mounted.address(&collection.name)),
        );
    }
    Ok(addresses)
}

/// The items of the note at `path` whose text is `text`, checked by their
/// kinds, and its problems in the order of their lines.
#[cfg(test)]
pub(crate) fn read_items(path: &str, text: &str, programs: &Programs) -> (Vec<Item>, Vec<Error>) {
    let parsed = crate::note::syntax::of(path).parse(text);
    let mut problems = parsed
        .errors
        .into_iter()
        .map(|(line, reason)| Error::Note {
            path: path.to_owned(),
            line,
            reason,
        })
        .collect::<Vec<_>>();
    let mut written = Vec::new();
    for (position, item) in parsed.items.into_iter().enumerate() {
        let origin = Origin {
            source: 0,
            collection: path.to_owned(),
            address: path.to_owned(),
            handle: position.to_string(),
            line: item.line,
            read: Object {
                entries: item.entries,
                fields: item.fields,
            },
        };
        match written_item(origin, item.text, programs) {
            Ok(item) => written.push(item),
            Err(Unread::Problem(problem)) => problems.push(problem),
            Err(Unread::Unidentified(origin)) => problems.push(Error::MissingId {
                path: origin.address,
                line: origin.line,
            }),
        }
    }
    let (items, checked) = check_items(written, programs, &mut Cache::None);
    problems.extend(checked);
    in_place_order(&mut problems);
    (items, problems)
}

/// Asks the kinds of `written` about them, and gives the items they
/// accepted, with their cards, and the problems: why an item is invalid,
/// and the kind programs that failed as a whole.
fn check_items(
    written: Vec<Item>,
    programs: &Programs,
    cache: &mut Cache<'_>,
) -> (Vec<Item>, Vec<Error>) {
    let asked = written
        .iter()
        .map(|item| Asked {
            kind: &item.kind,
            id: &item.id,
            fields: &item.fields,
        })
        .collect::<Vec<_>>();
    let (answers, mut problems) = check(&asked, programs, cache);
    let mut items = Vec::new();
    for (mut item, answer) in written.into_iter().zip(answers) {
        match answer {
            Some(Answer::Cards(groups)) => {
                item.cards = groups.concat();
                item.cards.sort();
                item.groups = groups;
                items.push(item);
            }
            Some(Answer::Invalid(reasons)) => {
                problems.extend(reasons.into_iter().map(|reason| Error::Note {
                    path: item.origin.address.clone(),
                    line: item.origin.line,
                    reason,
                }));
            }
            // Its program failed as a whole, which is a problem already.
            None => {}
        }
    }
    (items, problems)
}

/// Sorts `problems` by the collection and line they are about, collections
/// in the order they are read, keeping the order of problems about the
/// same line; problems about no collection come last.
fn in_place_order(problems: &mut [Error]) {
    problems.sort_by_cached_key(|problem| match problem.place() {
        Some((path, line)) => (false, nfc(path), path.to_owned(), line),
        None => (true, String::new(), String::new(), 0),
    });
}

/// Checks new items, as if they were read from a source, all at once, so
/// that each kind program is asked once about all of them. A problem's
/// line is the position of its item in `new`, counting from 1; a kind
/// program that failed as a whole is a problem without a line.
pub(crate) fn check_new(new: &[Object], programs: &Programs) -> Vec<Error> {
    let mut problems = Vec::new();
    let mut items = Vec::new();
    for (index, object) in new.iter().enumerate() {
        let origin = Origin {
            source: 0,
            collection: String::new(),
            address: String::new(),
            handle: String::new(),
            line: index + 1,
            read: object.clone(),
        };
        match written_item(origin, String::new(), programs) {
            Ok(item) => items.push(item),
            Err(Unread::Problem(problem)) => problems.push(problem),
            Err(Unread::Unidentified(origin)) => problems.push(Error::MissingId {
                path: origin.address,
                line: origin.line,
            }),
        }
    }
    problems.extend(check_items(items, programs, &mut Cache::None).1);
    in_place_order(&mut problems);
    problems
}

/// Why an item read from a source is not an item yet.
enum Unread {
    /// A problem with its entries or fields.
    Problem(Error),
    /// It has no ID, which `srl id` gives it.
    Unidentified(Box<Origin>),
}

/// The item read at `origin`, written as `written`, without its cards yet,
/// or why it is not an item.
fn written_item(
    origin: Origin,
    written: String,
    programs: &Programs,
) -> std::result::Result<Item, Unread> {
    let at = |reason: String| {
        Unread::Problem(Error::Note {
            path: origin.address.clone(),
            line: origin.line,
            reason,
        })
    };
    let read = &origin.read;
    let kind = read
        .entries
        .kind
        .as_deref()
        .map_or(Ok(Kind::Qa), |name| {
            Kind::parse(name, |name| programs.has(name))
        })
        .map_err(at)?;
    let Some(id) = read.entries.id.clone() else {
        return Err(Unread::Unidentified(Box::new(origin)));
    };
    if let Some(reason) = repeated_field(&read.fields) {
        return Err(at(reason));
    }
    Ok(Item {
        id,
        kind,
        see: read.entries.see.clone(),
        draft: read.entries.draft,
        fields: read.fields.clone(),
        text: written,
        cards: Vec::new(),
        groups: Vec::new(),
        origin,
    })
}

/// Why `fields` can't be an item's fields, whatever its kind: a key
/// written twice.
fn repeated_field(fields: &[Field]) -> Option<String> {
    let mut seen = HashSet::new();
    fields
        .iter()
        .find(|field| !seen.insert(field.key.as_str()))
        .map(|field| format!("the field {} appears twice; keep one", field.key))
}

/// Keeps the first item of each ID, and reports the others.
fn unique(items: Vec<Item>) -> (Vec<Item>, Vec<Error>) {
    let mut first = HashMap::new();
    let mut kept = Vec::new();
    let mut duplicates = Vec::new();
    for item in items {
        if let Some((address, line)) = first.get(&item.id) {
            duplicates.push(Error::Note {
                path: item.origin.address.clone(),
                line: item.origin.line,
                reason: format!(
                    "the ID {} is also used by {address}:{line}; delete the ID from the copy \
                     and run `srl id`",
                    item.id
                ),
            });
        } else {
            first.insert(
                item.id.clone(),
                (item.origin.address.clone(), item.origin.line),
            );
            kept.push(item);
        }
    }
    (kept, duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_get_their_cards_and_text() {
        let text = "# Title\n\n```srl\n@  id:x7k2m9 \nQ: What?\nA: That.\n\n@ id:k3m9q1 draft see:a.md\nQ: {{c1::a}} {{c2::b}}\n```\n";
        let (items, problems) = read_items("n.md", text, &Programs::default());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text(), "```srl id:x7k2m9\nQ: What?\nA: That.\n```");
        assert_eq!(items[0].label(), "What?");
        assert_eq!(items[0].cards().len(), 1);
        assert!(items[1].is_draft());
        assert_eq!(items[1].see(), Some("a.md"));
        let ids = items[1]
            .cards()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["k3m9q1.1", "k3m9q1.2"]);
    }

    #[test]
    fn the_notes_extension_chooses_how_its_items_are_read() {
        let text = "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"abd\", \"see\": \"n.md\", \"draft\": true, \"Q\": \"{{c1::r}}\"}\n";
        let (items, problems) = read_items("n.jsonl", text, &Programs::default());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(items[1].see(), Some("n.md"));
        assert!(items[1].is_draft());
        assert_eq!(items[1].line(), 2);
        assert_eq!(items[1].text(), text.lines().nth(1).unwrap());
        assert_eq!(
            items[0].json(None).to_string(),
            r#"{"id":"abc","kind":"qa","see":null,"draft":false,"Q":"q","A":"a","address":"n.jsonl","line":1,"cards":["abc"],"context":null,"context_address":null}"#
        );
        // The same text in any other note is prose, with no item.
        let (md, problems) = read_items("n.md", text, &Programs::default());
        assert!(md.is_empty() && problems.is_empty());
    }

    #[test]
    fn a_field_written_twice_is_a_problem_whatever_the_kind() {
        let (items, problems) = read_items(
            "n.md",
            "```srl id:abc\nQ: a\nQ: b\nA: c\n```\n",
            &Programs::default(),
        );
        assert_eq!(items, [] as [Item; 0]);
        assert_eq!(
            problems[0].to_string(),
            "n.md:1: the field Q appears twice; keep one"
        );
    }

    #[test]
    fn missing_ids_unknown_kinds_and_bad_fields_are_problems() {
        let text = "```srl\nQ: a\nA: b\n```\n```srl id:abc kind:conj\nVERB: x\n```\n```srl\n@ id:abd\nQ: only a question\n```\n";
        let (items, problems) = read_items("n.md", text, &Programs::default());
        assert_eq!(items, [] as [Item; 0]);
        assert!(matches!(problems[0], Error::MissingId { line: 1, .. }));
        let lines = problems
            .iter()
            .map(|problem| {
                let (Error::Note { line, .. } | Error::MissingId { line, .. }) = problem else {
                    panic!("unexpected {problem}");
                };
                *line
            })
            .collect::<Vec<_>>();
        assert_eq!(lines, [1, 5, 9]);
    }

    #[test]
    fn a_duplicate_id_keeps_the_first_item() {
        let (first, _) = read_items(
            "a.md",
            "```srl id:abc\nQ: a\nA: b\n```\n",
            &Programs::default(),
        );
        let (second, _) = read_items(
            "b.md",
            "```srl id:abc\nQ: c\nA: d\n```\n",
            &Programs::default(),
        );
        let (kept, duplicates) = unique(first.into_iter().chain(second).collect());
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].address(), "a.md");
        assert!(
            duplicates[0]
                .to_string()
                .starts_with("b.md:1: the ID abc is also used by a.md:1")
        );
    }
}
