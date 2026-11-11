//! The catalog: every item of a store, read from its sources and checked by
//! their kinds, with the problems found (see `docs/rust.md`, "Sources").
//!
//! Everything that crosses sources happens here: item IDs unique in the
//! store, collections given by one source each, and `see` naming a file of
//! study material, whose text is the item's context.

use std::collections::{HashMap, HashSet};
use std::fs;

use globset::GlobSet;
use jiff::civil::Date;
use serde::Serialize;

use crate::domain::card::{Card, CardId};
use crate::domain::item::object::Object;
use crate::domain::item::{Field, Item, ItemId, Origin};
use crate::error::Spot;
use crate::kind::{Answer, Asked, Cache, Kind, Programs, ToMake, check};
use crate::source::Sources;
use crate::store::nfc;
use crate::{Config, Error, Index, Result, Store};

/// Every item of a store, and the problems in its sources.
#[derive(Debug)]
pub struct Catalog {
    sources: Sources,
    items: Vec<Item>,
    problems: Vec<Error>,
    collections: Vec<Located>,
    unidentified: Vec<Origin>,
    programs: Programs,
    notes: Notes,
}

/// A collection of the store: its name, as its source gives it, and its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    pub(crate) collection: String,
    pub(crate) source: usize,
}

impl Catalog {
    /// Reads every source of the store. Problems in sources are collected
    /// in [`Catalog::problems`], in the order of the collections and items
    /// they are about; only failing to read a source is an error here. The
    /// items of every source are checked together, so that each kind
    /// program is asked once about all of its items, and only about the
    /// items it has no answer for in `index` (see `docs/cards.md`, "How srl
    /// calls them"). Problems with the cache are reported by
    /// [`Index::problems`].
    pub fn load(store: &Store, config: &Config, index: &mut Index) -> Result<Self> {
        Self::read(store, config, &mut Cache::Use(index))
    }

    /// Like [`Catalog::load`], asking kind programs about every item, and
    /// keeping their new answers in `index` (`srl check`).
    pub fn load_asking(store: &Store, config: &Config, index: &mut Index) -> Result<Self> {
        Self::read(store, config, &mut Cache::Refresh(index))
    }

    fn read(store: &Store, config: &Config, cache: &mut Cache<'_>) -> Result<Self> {
        let sources = Sources::new(store, config);
        let programs = Programs::new(store, config);
        let notes = Notes::new(store, config);
        let mut collections = Vec::new();
        let mut written = Vec::new();
        let mut problems = Vec::new();
        let mut unidentified = Vec::new();
        for (index, named) in sources.all().iter().enumerate() {
            let listed = named.source.collections()?;
            let known = listed
                .iter()
                .map(|listed| (listed.collection.clone(), listed.in_names.clone()))
                .collect::<HashMap<_, _>>();
            collections.extend(listed.into_iter().map(|listed| Located {
                collection: listed.collection,
                source: index,
            }));
            let read = named.source.read()?;
            problems.extend(read.problems.into_iter().map(|problem| Error::At {
                collection: problem.collection,
                spot: None,
                reason: problem.reason,
            }));
            let mut counts = HashMap::<String, usize>::new();
            for item in read.items {
                let in_names = known.get(&item.collection).cloned();
                if in_names.is_none() {
                    problems.push(Error::Collection {
                        collection: item.collection.clone(),
                        reason: format!(
                            "the source {} gives an item in it, but doesn't list it among its \
                             collections",
                            named.name
                        ),
                    });
                }
                let count = counts.entry(item.collection.clone()).or_default();
                *count += 1;
                let origin = Origin {
                    source: index,
                    source_name: named.name.clone(),
                    in_names: in_names.unwrap_or_else(|| vec![item.collection.clone()]),
                    collection: item.collection,
                    handle: item.handle,
                    position: *count,
                    read: item.item,
                };
                match written_item(origin, &programs) {
                    Ok(item) => written.push(item),
                    Err(Unread::Problem(problem)) => problems.push(problem),
                    Err(Unread::Unidentified(origin)) => {
                        problems.push(Error::MissingId {
                            collection: origin.collection.clone(),
                            position: origin.position,
                        });
                        unidentified.push(*origin);
                    }
                }
            }
        }
        // Collections, and so items, in the order of their names, whatever
        // their sources; items of one collection in the order read.
        collections.sort_by_cached_key(|located| by_collection(&located.collection));
        written.sort_by_cached_key(|item: &Item| by_collection(&item.origin.collection));
        unidentified.sort_by_cached_key(|origin| by_collection(&origin.collection));
        problems.extend(clashes(&sources, &collections));
        let (items, checked) = check_items(written, &programs, cache);
        problems.extend(checked);
        let (items, duplicates) = unique(items);
        problems.extend(duplicates);
        let mut seen = HashMap::<String, Option<String>>::new();
        problems.extend(items.iter().filter_map(|item| {
            let see = item.see.as_deref()?;
            let refusal = seen
                .entry(nfc(see))
                .or_insert_with(|| notes.refuses(see))
                .clone()?;
            Some(Error::At {
                collection: item.origin.collection.clone(),
                spot: Some(item.spot()),
                reason: refusal,
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
            notes,
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

    /// The collection named `collection`, compared in NFC (see
    /// `docs/store.md`, "File names").
    pub(crate) fn located(&self, collection: &str) -> Option<&Located> {
        let collection = nfc(collection);
        self.collections
            .iter()
            .find(|located| nfc(&located.collection) == collection)
    }

    /// Why `see` can't name an item's context, or `None` if it can (see
    /// `docs/items.md`, "Context").
    pub(crate) fn refuses_see(&self, see: &str) -> Option<String> {
        self.notes.refuses(see)
    }

    /// The context of `item` (see `docs/items.md`, "Context"):
    /// [`Catalog::contexts`] for one item.
    pub fn context(&self, item: &Item) -> Result<Option<String>> {
        let mut contexts = self.contexts(&[item])?;
        Ok(contexts.pop().flatten())
    }

    /// The context of each of `items`, in order: the text of the file its
    /// `see` names, without a byte order mark. `None` for an item without
    /// `see`, and for a file that is not UTF-8 or holds nothing but blank
    /// space. Each file is read once.
    pub fn contexts(&self, items: &[&Item]) -> Result<Vec<Option<String>>> {
        let mut read = HashMap::<&str, Option<String>>::new();
        items
            .iter()
            .map(|item| {
                let Some(see) = item.see() else {
                    return Ok(None);
                };
                if let Some(text) = read.get(see) {
                    return Ok(text.clone());
                }
                let text = self.notes.text(see)?;
                read.insert(see, text.clone());
                Ok(text)
            })
            .collect()
    }

    /// The card `card` of `item`, made by its kind as it is about to be
    /// shown on the study day `day`: [`Catalog::make_cards`] for one card.
    pub fn make_card(&self, item: &Item, card: &CardId, day: Date) -> Result<Card> {
        let mut made = self.make_cards(&[(item, card)], day)?;
        Ok(made.pop().expect("one card should have been made"))
    }

    /// The cards `cards`, each with its item, made by their kinds for the
    /// study day `day`, in order; each kind program makes all of its cards
    /// in one call (see `docs/cards.md`, "Kind programs"). Fails at the
    /// first card, in order, that can't be made: only a kind program can
    /// fail here, reading its item's context, or the program itself.
    pub fn make_cards(&self, cards: &[(&Item, &CardId)], day: Date) -> Result<Vec<Card>> {
        // Only kind programs get the context, read once for each item.
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
            .zip(self.contexts(&programs_items)?)
            .collect::<HashMap<_, _>>();
        let wanted = cards
            .iter()
            .map(|(item, card)| ToMake {
                card,
                kind: &item.kind,
                fields: &item.fields,
                see: item.see(),
                context: contexts.get(&item.id).and_then(Option::as_deref),
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

    /// The items, in the order they are written: by collection, then their
    /// order in it.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The problems in the sources. Every command that reads items stops
    /// if there are any (see `docs/items.md`, "Errors").
    pub fn problems(&self) -> &[Error] {
        &self.problems
    }

    /// The name of every collection, in order.
    pub fn collections(&self) -> impl Iterator<Item = &str> {
        self.collections
            .iter()
            .map(|located| located.collection.as_str())
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

/// The study material of a store: the files `[context]` chooses, which
/// items name with `see` (see `docs/items.md`, "Context").
#[derive(Debug)]
struct Notes {
    store: Store,
    include: GlobSet,
    exclude: GlobSet,
}

impl Notes {
    fn new(store: &Store, config: &Config) -> Self {
        Self {
            store: store.clone(),
            include: config.context.include_set(),
            exclude: config.context.exclude_set(),
        }
    }

    /// Why `see` can't name a context, or `None` if it can: it must be a
    /// path relative to the store's folder that `[context]` chooses, outside
    /// folders whose name starts with `.`, as `srl notes` lists them, of a
    /// file that exists.
    fn refuses(&self, see: &str) -> Option<String> {
        let path = nfc(see);
        if !is_relative_path(see) {
            return Some(format!(
                "`see` {see} is not a path relative to the store's folder, with / between \
                 folders, such as japanese/grammar/wo.md"
            ));
        }
        if !self.include.is_match(&path)
            || self.exclude.is_match(&path)
            || path.split('/').any(|part| part.starts_with('.'))
        {
            return Some(format!(
                "`see` {see} is not study material: `[context]` in .srl/config.toml doesn't \
                 include it (`srl notes` lists the files it does)"
            ));
        }
        if !self.store.file(see).is_file() {
            return Some(format!(
                "`see` {see} names no file of the store; give the path of a file `srl notes` \
                 lists"
            ));
        }
        None
    }

    /// The text of the file at `see`, without a byte order mark, or `None`
    /// when it is not UTF-8 or holds nothing but blank space.
    fn text(&self, see: &str) -> Result<Option<String>> {
        let file = self.store.file(see);
        let bytes = fs::read(&file).map_err(Error::io(file))?;
        let Ok(text) = String::from_utf8(bytes) else {
            return Ok(None);
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        Ok((!text.trim().is_empty()).then(|| text.to_owned()))
    }

    /// Every file of study material, in order.
    fn paths(&self) -> Result<Vec<String>> {
        self.store.files(&self.include, &self.exclude)
    }
}

/// Whether `path` is a path relative to a folder, with `/` between its
/// parts, none of them empty, `.` or `..`.
fn is_relative_path(path: &str) -> bool {
    !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// A collection of the store, as `srl collections` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionInfo {
    pub collection: String,
    /// The name of its source.
    pub source: String,
    /// Its `in` names: the values of `in:` that select it.
    #[serde(rename = "in")]
    pub in_names: Vec<String>,
}

/// Every collection of the store, in the order of their names, without
/// reading any item (`srl collections`).
pub fn collections(store: &Store, config: &Config) -> Result<Vec<CollectionInfo>> {
    let mut found = Vec::new();
    for named in Sources::new(store, config).all() {
        found.extend(
            named
                .source
                .collections()?
                .into_iter()
                .map(|listed| CollectionInfo {
                    collection: listed.collection,
                    source: named.name.clone(),
                    in_names: listed.in_names,
                }),
        );
    }
    found.sort_by_cached_key(|collection| by_collection(&collection.collection));
    Ok(found)
}

/// Every file of study material of the store: those items may name with
/// `see`, in order (`srl notes`).
pub fn notes(store: &Store, config: &Config) -> Result<Vec<String>> {
    Notes::new(store, config).paths()
}

/// A source of the store, as `srl sources` describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceDescription {
    pub name: String,
    /// Its driver: `filesystem`, or `program` for a source program.
    pub driver: String,
    pub description: String,
    /// The collections `srl add` may create in it, as globs: those a glob
    /// of `include` matches and none of `exclude` does.
    pub creates: Creates,
}

/// The globs of the collections a source may create.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Creates {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Every source of the store, in the order of their names (`srl sources`).
pub fn sources(store: &Store, config: &Config) -> Result<Vec<SourceDescription>> {
    Sources::new(store, config)
        .all()
        .iter()
        .map(|named| {
            let described = named.source.describe()?;
            Ok(SourceDescription {
                name: named.name.clone(),
                driver: named.driver.as_str().to_owned(),
                description: described.summary,
                creates: Creates {
                    include: described.include,
                    exclude: described.exclude,
                },
            })
        })
        .collect()
}

/// The items of the collection file at `path` whose text is `text`,
/// checked by their kinds, and its problems in the order of their items.
#[cfg(test)]
pub(crate) fn read_items(path: &str, text: &str, programs: &Programs) -> (Vec<Item>, Vec<Error>) {
    let parsed = crate::source::parse(path, text);
    let mut problems = parsed
        .errors
        .into_iter()
        .map(|reason| Error::At {
            collection: path.to_owned(),
            spot: None,
            reason,
        })
        .collect::<Vec<_>>();
    let mut written = Vec::new();
    for (index, item) in parsed.items.into_iter().enumerate() {
        let origin = Origin {
            source: 0,
            source_name: crate::config::DEFAULT_SOURCE.to_owned(),
            collection: path.to_owned(),
            in_names: crate::source::file_in_names(path),
            handle: index.to_string(),
            position: index + 1,
            read: Object {
                entries: item.entries,
                fields: item.fields,
            },
        };
        match written_item(origin, programs) {
            Ok(item) => written.push(item),
            Err(Unread::Problem(problem)) => problems.push(problem),
            Err(Unread::Unidentified(origin)) => problems.push(Error::MissingId {
                collection: origin.collection,
                position: origin.position,
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
                problems.extend(reasons.into_iter().map(|reason| Error::At {
                    collection: item.origin.collection.clone(),
                    spot: Some(item.spot()),
                    reason,
                }));
            }
            // Its program failed as a whole, which is a problem already.
            None => {}
        }
    }
    (items, problems)
}

/// The collections that break a rule of collections (see
/// `docs/sources.md`, "Collections"): a name that two sources give.
fn clashes(sources: &Sources, collections: &[Located]) -> Vec<Error> {
    let named = sources.all();
    let name = |index: usize| named.get(index).map_or("", |source| source.name.as_str());
    let mut problems = Vec::new();
    let mut seen = HashMap::<String, &Located>::new();
    for located in collections {
        if let Some(first) = seen.insert(nfc(&located.collection), located) {
            problems.push(Error::Collection {
                collection: located.collection.clone(),
                reason: format!(
                    "the sources {} and {} both give it; leave it out of one of them",
                    name(first.source),
                    name(located.source)
                ),
            });
        }
    }
    problems
}

/// The key collections are sorted by: their name in NFC, so that the
/// Unicode form of a name never changes the order (see
/// `docs/store.md`, "File names"), then as written.
fn by_collection(collection: &str) -> (String, String) {
    (nfc(collection), collection.to_owned())
}

/// Sorts `problems` by the collection and item they are about, collections
/// in the order they are read, keeping the order of problems about the
/// same item; problems about no collection come last.
fn in_place_order(problems: &mut [Error]) {
    problems.sort_by_cached_key(|problem| match problem.place() {
        Some((collection, position)) => (false, nfc(collection), collection.to_owned(), position),
        None => (true, String::new(), String::new(), 0),
    });
}

/// Checks new items, as if they were read from a source, all at once, so
/// that each kind program is asked once about all of them. A problem's
/// position is that of its item in `new`, counting from 1; a kind program
/// that failed as a whole is a problem without a position.
pub(crate) fn check_new(new: &[Object], programs: &Programs) -> Vec<Error> {
    let mut problems = Vec::new();
    let mut items = Vec::new();
    for (index, object) in new.iter().enumerate() {
        let origin = Origin {
            source: 0,
            source_name: String::new(),
            collection: String::new(),
            in_names: Vec::new(),
            handle: String::new(),
            position: index + 1,
            read: object.clone(),
        };
        match written_item(origin, programs) {
            Ok(item) => items.push(item),
            Err(Unread::Problem(problem)) => problems.push(problem),
            Err(Unread::Unidentified(origin)) => problems.push(Error::MissingId {
                collection: origin.collection,
                position: origin.position,
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

/// The item read at `origin`, without its cards yet, or why it is not an
/// item.
fn written_item(origin: Origin, programs: &Programs) -> std::result::Result<Item, Unread> {
    let at = |reason: String| {
        Unread::Problem(Error::At {
            collection: origin.collection.clone(),
            spot: Some(Spot {
                position: origin.position,
                id: origin.read.entries.id.as_ref().map(ToString::to_string),
            }),
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

/// Keeps the first item of each ID, and reports the others, each by its
/// position, since its ID names the first.
fn unique(items: Vec<Item>) -> (Vec<Item>, Vec<Error>) {
    let mut first = HashMap::<ItemId, String>::new();
    let mut kept = Vec::new();
    let mut duplicates = Vec::new();
    for item in items {
        if let Some(collection) = first.get(&item.id) {
            duplicates.push(Error::At {
                collection: item.origin.collection.clone(),
                spot: Some(Spot {
                    position: item.origin.position,
                    id: None,
                }),
                reason: format!(
                    "the ID {} is also used by an item of {collection}; delete the ID from the copy \
                     and run `srl id`",
                    item.id,
                ),
            });
        } else {
            first.insert(item.id.clone(), item.origin.collection.clone());
            kept.push(item);
        }
    }
    (kept, duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_get_their_cards() {
        let text = "{\"id\": \"x7k2m9\", \"Q\": \"What?\", \"A\": \"That.\"}\n\n{\"id\": \"k3m9q1\", \"draft\": true, \"see\": \"a.md\", \"Q\": \"{{c1::a}} {{c2::b}}\"}\n";
        let (items, problems) = read_items("n.jsonl", text, &Programs::default());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(items.len(), 2);
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
        assert_eq!(
            items[0].json(None).to_string(),
            r#"{"id":"x7k2m9","kind":"qa","see":null,"draft":false,"Q":"What?","A":"That.","collection":"n.jsonl","source":"items","cards":["x7k2m9"],"context":null}"#
        );
    }

    #[test]
    fn a_file_of_another_format_is_a_problem() {
        let (items, problems) = read_items("n.md", "# Title\n", &Programs::default());
        assert_eq!(items, [] as [Item; 0]);
        assert!(
            problems[0]
                .to_string()
                .starts_with("n.md: srl reads collections"),
            "{problems:?}"
        );
    }

    #[test]
    fn a_field_written_twice_is_a_problem_whatever_the_kind() {
        let (items, problems) = read_items(
            "n.jsonl",
            "{\"id\": \"abc\", \"Q\": \"a\", \"Q\": \"b\", \"A\": \"c\"}\n",
            &Programs::default(),
        );
        assert_eq!(items, [] as [Item; 0]);
        assert_eq!(
            problems[0].to_string(),
            "n.jsonl: line 1: the key `Q` appears twice; keep one"
        );
    }

    #[test]
    fn missing_ids_unknown_kinds_and_bad_fields_are_problems() {
        let text = "{\"Q\": \"a\", \"A\": \"b\"}\n{\"id\": \"abc\", \"kind\": \"conj\", \"VERB\": \"x\"}\n{\"id\": \"abd\", \"Q\": \"only a question\"}\n";
        let (items, problems) = read_items("n.jsonl", text, &Programs::default());
        assert_eq!(items, [] as [Item; 0]);
        let messages = problems.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(messages.len(), 3, "{messages:?}");
        assert_eq!(
            messages[0],
            "n.jsonl: item 1 has no ID; run `srl id` to give it one"
        );
        assert!(
            messages[1].starts_with("n.jsonl: item abc: "),
            "{messages:?}"
        );
        assert!(
            messages[2].starts_with("n.jsonl: item abd: "),
            "{messages:?}"
        );
    }

    #[test]
    fn a_duplicate_id_keeps_the_first_item() {
        let item = "{\"id\": \"abc\", \"Q\": \"a\", \"A\": \"b\"}\n";
        let (first, _) = read_items("a.jsonl", item, &Programs::default());
        let (second, _) = read_items("b.jsonl", item, &Programs::default());
        let (kept, duplicates) = unique(first.into_iter().chain(second).collect());
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].collection(), "a.jsonl");
        assert!(
            duplicates[0]
                .to_string()
                .starts_with("b.jsonl: item 1: the ID abc is also used by an item of a.jsonl"),
            "{duplicates:?}"
        );
    }

    #[test]
    fn see_must_be_a_relative_path() {
        for path in ["/a.md", "a//b.md", "../a.md", "./a.md", "a\\b.md", "c:a.md"] {
            assert!(!is_relative_path(path), "accepted {path}");
        }
        for path in ["a.md", "a/b c/d.md", ".hidden/a.md"] {
            assert!(is_relative_path(path), "refused {path}");
        }
    }
}
