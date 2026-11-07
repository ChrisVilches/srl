//! Items: their entries, fields and IDs (see `docs/items.md`), and the item
//! object, the one JSON form of an item.
//!
//! How items are written is the business of their sources (see
//! [`crate::source`]); the catalog turns what every source reads into
//! [`Item`]s, which the rest of `srl` works with.

pub(crate) mod object;

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::domain::card::CardId;
use crate::error::Spot;
use crate::kind::Kind;
use crate::{Error, Result};
use object::Object;

/// One field of an item: its key and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub text: String,
}

/// What an item says about itself besides its fields: its header entries
/// (see `docs/items.md`, "Header entries"), however its source writes
/// them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Entries {
    pub(crate) id: Option<ItemId>,
    /// The kind's name, if one was given. Which kinds exist is checked
    /// later.
    pub(crate) kind: Option<String>,
    pub(crate) see: Option<String>,
    pub(crate) draft: bool,
}

/// Whether `key` can name a field: uppercase letters `A`–`Z`, digits and
/// `_`, starting with a letter.
pub(crate) fn is_field_key(key: &str) -> bool {
    let mut characters = key.chars();
    characters.next().is_some_and(|c| c.is_ascii_uppercase())
        && characters.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// The most characters an item ID may have.
const ID_MAX: usize = 32;

/// The number of characters of the IDs `srl` makes.
const NEW_ID_LEN: usize = 6;

/// The characters `srl` makes IDs of: an ID's characters other than
/// [`ID_DASH`].
const ID_CHARS: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// The character that joins words in an ID, as in `jp-grammar-desu`.
const ID_DASH: u8 = b'-';

/// An item's stable identity: 1 to 32 characters, words of `[a-z0-9]`
/// joined by single `-` (see [`is_id_text`]).
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

/// Whether `text` is an item ID, or a card ID's suffix: 1 to 32
/// characters, words of `[a-z0-9]` joined by single `-`, so `-` is never
/// first, last or doubled. It never contains
/// [`SUFFIX_SEPARATOR`](crate::domain::card::SUFFIX_SEPARATOR).
pub(crate) fn is_id_text(text: &str) -> bool {
    text.len() <= ID_MAX
        && text
            .split(char::from(ID_DASH))
            .all(|word| !word.is_empty() && word.bytes().all(|byte| ID_CHARS.contains(&byte)))
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where an item comes from: what names it to people, and to the source
/// that changes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Origin {
    /// The item's source, by its place among the store's sources.
    pub(crate) source: usize,
    /// The source's name.
    pub(crate) source_name: String,
    /// The name of the collection the item is in, as its source
    /// gives it.
    pub(crate) collection: String,
    /// What names the item to its source when it is changed.
    pub(crate) handle: String,
    /// The item's position among the items its source read in its
    /// collection, counting from 1: for messages about an item without an
    /// ID, and to keep problems in order.
    pub(crate) position: usize,
    /// The item as its source read it, which a change gives back as what
    /// it replaces.
    pub(crate) read: Object,
}

/// One item of a store, with its cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub(crate) id: ItemId,
    pub(crate) origin: Origin,
    pub(crate) kind: Kind,
    pub(crate) see: Option<String>,
    pub(crate) draft: bool,
    pub(crate) fields: Vec<Field>,
    /// The card IDs in groups of siblings, as the kind gave them.
    pub(crate) groups: Vec<Vec<CardId>>,
    /// Every card ID of `groups`, in order.
    pub(crate) cards: Vec<CardId>,
}

impl Item {
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// The name of the item's collection: for a collection of the
    /// `filesystem` source, its path relative to the store's folder, with
    /// `/` between folders.
    pub fn collection(&self) -> &str {
        &self.origin.collection
    }

    /// The name of the item's source.
    pub fn source(&self) -> &str {
        &self.origin.source_name
    }

    /// Which item of its collection this is, for messages.
    pub(crate) fn spot(&self) -> Spot {
        Spot {
            position: self.origin.position,
            id: Some(self.id.to_string()),
        }
    }

    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// The path of the file `see` names as the item's context, if any.
    pub fn see(&self) -> Option<&str> {
        self.see.as_deref()
    }

    pub fn is_draft(&self) -> bool {
        self.draft
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
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

    /// The item object, with every entry and the keys only `srl` gives
    /// (see `docs/items.md`, "The item object"), with the text of its
    /// context and where it comes from.
    pub fn json(&self, context: Option<&str>) -> Value {
        let mut object = Map::new();
        object.insert("id".to_owned(), Value::from(self.id.as_str()));
        object.insert("kind".to_owned(), Value::from(self.kind.as_str()));
        object.insert("see".to_owned(), Value::from(self.see.clone()));
        object.insert("draft".to_owned(), Value::Bool(self.draft));
        for field in &self.fields {
            object.insert(field.key.clone(), Value::from(field.text.as_str()));
        }
        object.insert(
            "collection".to_owned(),
            Value::from(self.origin.collection.as_str()),
        );
        object.insert(
            "source".to_owned(),
            Value::from(self.origin.source_name.as_str()),
        );
        object.insert("cards".to_owned(), serde_json::json!(self.cards));
        object.insert("context".to_owned(), Value::from(context));
        Value::Object(object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_ids_are_1_to_32_lowercase_letters_or_digits_and_dashes() {
        for good in [
            "a",
            "a7f3k2",
            "verb1",
            "a7f-k2",
            "jp-grammar-desu",
            "1-2-3",
            &"a".repeat(32),
            &format!("{}-b", "a".repeat(30)),
        ] {
            assert!(ItemId::parse(good).is_ok(), "refused {good}");
        }
        for bad in [
            "",
            "A7F3K2",
            "a b",
            &"a".repeat(33),
            &format!("{}-b", "a".repeat(31)),
            // The dot separates a card's suffix, so no ID contains it.
            "a.b",
            ".",
            // A dash only joins two words.
            "-",
            "-a",
            "a-",
            "a--b",
            "a_b",
            "a:b",
        ] {
            assert!(ItemId::parse(bad).is_err(), "accepted {bad}");
        }
        assert_eq!(ItemId::from_number(0).as_str(), "aaaaaa");
        assert_eq!(ItemId::from_number(35).as_str(), "9aaaaa");
    }
}
