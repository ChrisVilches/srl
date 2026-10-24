//! Cards and card IDs (see `docs/cards.md`).

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::note::{ItemId, is_id_text};
use crate::{Error, Result};

/// A card's ID: its item's ID, alone or followed by `-` and a suffix (see
/// `docs/cards.md`, "Card IDs").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CardId {
    item: ItemId,
    suffix: Option<String>,
}

impl CardId {
    pub(crate) fn new(item: ItemId, suffix: Option<String>) -> Self {
        Self { item, suffix }
    }

    /// Parses `x7k2m9` or `k3m9q1-2`.
    pub fn parse(text: &str) -> Result<Self> {
        let invalid = || Error::InvalidCardId {
            text: text.to_owned(),
        };
        let (item, suffix) = match text.split_once('-') {
            Some((item, suffix)) if is_id_text(suffix) => (item, Some(suffix.to_owned())),
            Some(_) => return Err(invalid()),
            None => (text, None),
        };
        let item = ItemId::parse(item).map_err(|_error| invalid())?;
        Ok(Self { item, suffix })
    }

    pub fn item(&self) -> &ItemId {
        &self.item
    }

    pub fn suffix(&self) -> Option<&str> {
        self.suffix.as_deref()
    }
}

/// Card IDs are ordered by item, then by suffix, where suffixes that are
/// whole numbers come first and compare as numbers, so that `-2` comes
/// before `-10`.
impl Ord for CardId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.item
            .cmp(&other.item)
            .then_with(|| suffix_key(self.suffix()).cmp(&suffix_key(other.suffix())))
    }
}

/// What a suffix is ordered by: no suffix first, then whole numbers by
/// value, then other suffixes as text.
fn suffix_key(suffix: Option<&str>) -> Option<(bool, Option<u128>, &str)> {
    suffix.map(|suffix| {
        let number = suffix.parse::<u128>().ok();
        (number.is_none(), number, suffix)
    })
}

impl PartialOrd for CardId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TryFrom<String> for CardId {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        Self::parse(&text)
    }
}

impl From<CardId> for String {
    fn from(id: CardId) -> Self {
        id.to_string()
    }
}

impl fmt::Display for CardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.suffix {
            Some(suffix) => write!(f, "{}-{suffix}", self.item),
            None => write!(f, "{}", self.item),
        }
    }
}

/// One question, ready to show (see `docs/cards.md`, "Format").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    pub(crate) id: CardId,
    pub(crate) prompt: String,
    pub(crate) prompt_revealed: String,
    pub(crate) answer: Option<String>,
}

impl Card {
    pub fn id(&self) -> &CardId {
        &self.id
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn prompt_revealed(&self) -> &str {
        &self.prompt_revealed
    }

    pub fn answer(&self) -> Option<&str> {
        self.answer.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_ids_are_an_item_id_and_an_optional_suffix() {
        let plain = CardId::parse("x7k2m9").unwrap();
        assert_eq!((plain.item().as_str(), plain.suffix()), ("x7k2m9", None));
        let cloze = CardId::parse("k3m9q1-2").unwrap();
        assert_eq!(
            (cloze.item().as_str(), cloze.suffix()),
            ("k3m9q1", Some("2"))
        );
        assert_eq!(cloze.to_string(), "k3m9q1-2");
        for bad in ["", "-1", "k3m9q1-", "k3m9q1-2-3", "K3M9Q1", "a b"] {
            assert!(CardId::parse(bad).is_err(), "accepted {bad}");
        }
    }

    #[test]
    fn numeric_suffixes_order_as_numbers() {
        let mut ids = ["a-10", "a-te", "a-2", "a", "b-1"]
            .map(|id| CardId::parse(id).unwrap())
            .to_vec();
        ids.sort();
        let ids = ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(ids, ["a", "a-2", "a-10", "a-te", "b-1"]);
    }
}
