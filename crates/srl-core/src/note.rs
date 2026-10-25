//! Notes and the items in them (see `docs/notes.md`).

pub(crate) mod parse;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::card::{Card, CardId};
use crate::context::{ContextMode, context};
use crate::kind::{Kind, Programs};
use crate::store::nfc;
use crate::{Config, Error, Result, Store};

pub use parse::Field;

/// The most characters an item ID may have.
const ID_MAX: usize = 32;

/// The number of characters of the IDs `srl` makes.
const NEW_ID_LEN: usize = 6;

/// The characters an ID is made of.
const ID_CHARS: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// An item's stable identity: 1 to 32 characters `[a-z0-9]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ItemId(String);

impl TryFrom<String> for ItemId {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        Self::parse(&text)
    }
}

impl From<ItemId> for String {
    fn from(id: ItemId) -> Self {
        id.0
    }
}

impl ItemId {
    pub fn parse(text: &str) -> Result<Self> {
        if is_id_text(text) {
            Ok(Self(text.to_owned()))
        } else {
            Err(Error::InvalidItemId {
                text: text.to_owned(),
            })
        }
    }

    /// The ID made of the last six base-36 digits of `number`.
    pub(crate) fn from_number(number: u64) -> Self {
        Self(
            std::iter::successors(Some(number), |rest| Some(rest / 36))
                .take(NEW_ID_LEN)
                .map(|rest| {
                    let digit =
                        usize::try_from(rest % 36).expect("a digit below 36 should fit in a usize");
                    let byte = ID_CHARS
                        .get(digit)
                        .expect("a digit below 36 should index ID_CHARS");
                    char::from(*byte)
                })
                .collect(),
        )
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether `text` is 1 to 32 characters `[a-z0-9]`: an item ID, or a card
/// ID's suffix.
pub(crate) fn is_id_text(text: &str) -> bool {
    !text.is_empty() && text.len() <= ID_MAX && text.bytes().all(|byte| ID_CHARS.contains(&byte))
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One item of a note, with its cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    id: ItemId,
    path: String,
    line: usize,
    end: usize,
    kind: Kind,
    see: Option<String>,
    draft: bool,
    fields: Vec<Field>,
    text: String,
    /// The card IDs in groups of siblings, as the kind gave them.
    groups: Vec<Vec<CardId>>,
    /// Every card ID of `groups`, in order.
    cards: Vec<CardId>,
}

impl Item {
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// The note's path, relative to the store's folder, with `/` between
    /// folders.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The line of the header, counting from 1.
    pub fn line(&self) -> usize {
        self.line
    }

    /// The last line of the item, counting from 1.
    pub fn end(&self) -> usize {
        self.end
    }

    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// The path `see:` gives, if any.
    pub fn see(&self) -> Option<&str> {
        self.see.as_deref()
    }

    /// The path of the note shown as context: `see:`, or the item's own
    /// note (see `docs/notes.md`, "Context").
    pub fn context_path(&self) -> &str {
        self.see().unwrap_or(self.path())
    }

    pub fn is_draft(&self) -> bool {
        self.draft
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// The item's lines as written, header included, joined with `\n`.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The IDs of the item's cards, in order.
    pub fn cards(&self) -> &[CardId] {
        &self.cards
    }

    /// The group of siblings `card` belongs to, by its position among the
    /// item's groups (see `docs/scheduling.md`, "Siblings").
    pub(crate) fn group(&self, card: &CardId) -> usize {
        self.groups
            .iter()
            .position(|group| group.contains(card))
            .expect("a card should be in one of its item's groups")
    }

    /// The cards of the group `group`, as [`Item::group`] gives it.
    pub(crate) fn siblings(&self, group: usize) -> &[CardId] {
        self.groups
            .get(group)
            .expect("a group should be one of the item's groups")
    }

    /// The first line of the first field: how listings name the item.
    pub fn label(&self) -> &str {
        self.fields
            .first()
            .and_then(|field| field.text.lines().next())
            .unwrap_or_default()
    }

    /// The text of every field, lowercased, for text search.
    pub(crate) fn search_text(&self) -> String {
        self.fields
            .iter()
            .map(|field| field.text.to_lowercase())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The item as JSON (see `docs/notes.md`, "Items as JSON"), with its
    /// context.
    pub fn json(&self, context: Option<&str>) -> Value {
        let fields = self
            .fields
            .iter()
            .map(|field| (field.key.clone(), Value::from(field.text.as_str())))
            .collect::<Map<_, _>>();
        serde_json::json!({
            "id": self.id,
            "path": self.path,
            "line": self.line,
            "kind": self.kind.as_str(),
            "see": self.see,
            "draft": self.draft,
            "fields": fields,
            "cards": self.cards,
            "context": context,
        })
    }
}

/// Every item of a store, and the problems in its notes.
#[derive(Debug)]
pub struct Notes {
    items: Vec<Item>,
    problems: Vec<Error>,
    paths: Vec<String>,
    programs: Programs,
}

impl Notes {
    /// Reads every note of the store. Problems in notes are collected in
    /// [`Notes::problems`]; only failing to read a file is an error here.
    pub fn load(store: &Store, config: &Config) -> Result<Self> {
        let paths = store.note_paths(&config.notes.include_set(), &config.notes.exclude_set())?;
        let programs = Programs::new(store, config);
        let mut items = Vec::new();
        let mut problems = Vec::new();
        for path in &paths {
            let text = read_note(store, path)?;
            let (note_items, note_problems) = read_items(path, &text, &programs);
            items.extend(note_items);
            problems.extend(note_problems);
        }
        let (items, duplicates) = unique(items);
        problems.extend(duplicates);
        let known = paths.iter().map(|path| nfc(path)).collect::<HashSet<_>>();
        problems.extend(items.iter().filter_map(|item| {
            let see = item.see.as_deref()?;
            (!known.contains(&nfc(see))).then(|| Error::Note {
                path: item.path.clone(),
                line: item.line,
                reason: format!(
                    "`see:{see}` doesn't name a note of the store; give a path relative to the \
                     store's folder, such as japanese/grammar/wo.md"
                ),
            })
        }));
        Ok(Self {
            items,
            problems,
            paths,
            programs,
        })
    }

    /// The kind programs the items were read with.
    pub(crate) fn programs(&self) -> &Programs {
        &self.programs
    }

    /// The card `card` of `item`, made by its kind as it is about to be
    /// shown on the study day `day` (see `docs/cards.md`, "Kind programs").
    /// Only a kind program can fail here.
    pub fn make_card(&self, store: &Store, item: &Item, card: &CardId, day: Date) -> Result<Card> {
        let context = match item.kind {
            Kind::Qa => None,
            Kind::Program(_) => context(store, item, ContextMode::Prose)?,
        };
        item.kind
            .card(card, &item.fields, &self.programs, context.as_deref(), day)
            .map_err(|reason| Error::CardFailed {
                card: card.to_string(),
                reason,
            })
    }

    /// The items, in the order they are written: by note path, then line.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The problems in the notes. Every command that reads items stops if
    /// there are any (see `docs/notes.md`, "Errors").
    pub fn problems(&self) -> &[Error] {
        &self.problems
    }

    /// Every note's path, in order.
    pub fn paths(&self) -> &[String] {
        &self.paths
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

/// The text of the note at `path`.
pub(crate) fn read_note(store: &Store, path: &str) -> Result<String> {
    let file = store.note_file(path);
    fs::read_to_string(&file).map_err(Error::io(file))
}

/// The items of the note at `path` whose text is `text`, and its problems.
pub(crate) fn read_items(path: &str, text: &str, programs: &Programs) -> (Vec<Item>, Vec<Error>) {
    let parsed = parse::parse(text);
    let lines = parse::lines(text);
    let mut problems = parsed
        .errors
        .into_iter()
        .map(|(line, reason)| Error::Note {
            path: path.to_owned(),
            line,
            reason,
        })
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    for raw in parsed.items {
        let at = |reason: String| Error::Note {
            path: path.to_owned(),
            line: raw.line,
            reason,
        };
        let kind = match raw.header.kind.as_deref().map_or(Ok(Kind::Qa), |name| {
            Kind::parse(name, |name| programs.has(name))
        }) {
            Ok(kind) => kind,
            Err(reason) => {
                problems.push(at(reason));
                continue;
            }
        };
        let Some(id) = raw.header.id else {
            problems.push(Error::MissingId {
                path: path.to_owned(),
                line: raw.line,
            });
            continue;
        };
        if let Some(reason) = repeated_field(&raw.fields) {
            problems.push(at(reason));
            continue;
        }
        match kind.groups(&id, &raw.fields, programs) {
            Ok(groups) => items.push(Item {
                id,
                path: path.to_owned(),
                line: raw.line,
                end: raw.end,
                kind,
                see: raw.header.see,
                draft: raw.header.draft,
                fields: raw.fields,
                text: lines
                    .get(raw.line - 1..raw.end)
                    .unwrap_or_default()
                    .join("\n"),
                cards: {
                    let mut cards = groups.concat();
                    cards.sort();
                    cards
                },
                groups,
            }),
            Err(reason) => problems.push(at(reason)),
        }
    }
    (items, problems)
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
        if let Some((path, line)) = first.get(&item.id) {
            duplicates.push(Error::Note {
                path: item.path.clone(),
                line: item.line,
                reason: format!(
                    "the ID {} is also used by {path}:{line}; delete the ID from the copy and \
                     run `srl id`",
                    item.id
                ),
            });
        } else {
            first.insert(item.id.clone(), (item.path.clone(), item.line));
            kept.push(item);
        }
    }
    (kept, duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_ids_are_1_to_32_lowercase_letters_or_digits() {
        for good in ["a", "a7f3k2", "verb1", &"a".repeat(32)] {
            assert!(ItemId::parse(good).is_ok(), "refused {good}");
        }
        for bad in ["", "A7F3K2", "a7f-k2", "a b", &"a".repeat(33)] {
            assert!(ItemId::parse(bad).is_err(), "accepted {bad}");
        }
        assert_eq!(ItemId::from_number(0).as_str(), "aaaaaa");
        assert_eq!(ItemId::from_number(35).as_str(), "9aaaaa");
    }

    #[test]
    fn items_get_their_cards_and_text() {
        let text = "# Title\n\n<!-- srl id:x7k2m9 -->\nQ: What?\nA: That.\n\n<!-- srl id:k3m9q1 draft see:a.md -->\nQ: {{c1::a}} {{c2::b}}\n";
        let (items, problems) = read_items("n.md", text, &Programs::default());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].text(),
            "<!-- srl id:x7k2m9 -->\nQ: What?\nA: That."
        );
        assert_eq!(items[0].label(), "What?");
        assert_eq!(items[0].cards().len(), 1);
        assert!(items[1].is_draft());
        assert_eq!(items[1].see(), Some("a.md"));
        let ids = items[1]
            .cards()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["k3m9q1-1", "k3m9q1-2"]);
    }

    #[test]
    fn a_field_written_twice_is_a_problem_whatever_the_kind() {
        let (items, problems) = read_items(
            "n.md",
            "<!-- srl id:abc -->\nQ: a\nQ: b\nA: c\n",
            &Programs::default(),
        );
        assert!(items.is_empty());
        assert_eq!(
            problems[0].to_string(),
            "n.md:1: the field Q appears twice; keep one"
        );
    }

    #[test]
    fn missing_ids_unknown_kinds_and_bad_fields_are_problems() {
        let text = "<!-- srl -->\nQ: a\nA: b\n\n<!-- srl id:abc kind:conj -->\nVERB: x\n\n<!-- srl id:abd -->\nQ: only a question\n";
        let (items, problems) = read_items("n.md", text, &Programs::default());
        assert!(items.is_empty());
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
        assert_eq!(lines, [1, 5, 8]);
    }

    #[test]
    fn a_duplicate_id_keeps_the_first_item() {
        let (first, _) = read_items(
            "a.md",
            "<!-- srl id:abc -->\nQ: a\nA: b\n",
            &Programs::default(),
        );
        let (second, _) = read_items(
            "b.md",
            "<!-- srl id:abc -->\nQ: c\nA: d\n",
            &Programs::default(),
        );
        let (kept, duplicates) = unique(first.into_iter().chain(second).collect());
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path(), "a.md");
        assert!(
            duplicates[0]
                .to_string()
                .starts_with("b.md:1: the ID abc is also used by a.md:1")
        );
    }
}
