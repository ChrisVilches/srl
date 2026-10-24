//! The selector language (see `docs/selectors.md`).

use std::path::Path;

use globset::GlobSet;

use crate::config::globs;
use crate::store::nfc;
use crate::{CardId, Error, Item, Memories, Result, State};

/// The keys of `key:value` terms.
const KEYS: &str = "in, path, id, status, state";

/// A parsed selector: cards match when they match every term.
#[derive(Debug, Clone)]
pub struct Selector {
    terms: Vec<Term>,
}

#[derive(Debug, Clone)]
struct Term {
    negated: bool,
    filter: Filter,
}

/// What one term checks. Lists are alternatives (`in:a,b`).
#[derive(Debug, Clone)]
enum Filter {
    /// Names in NFC, lowercased.
    In(Vec<String>),
    Path(GlobSet),
    /// Card IDs, or item IDs that stand for all their cards.
    Id(Vec<CardId>),
    Status(Vec<Status>),
    State(Vec<StateTerm>),
    /// Lowercased.
    Text(String),
}

/// A card's status (see `docs/notes.md`, "Status").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Active,
    Draft,
    Suspended,
}

impl Status {
    /// The status named `text`, if there is one.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "active" => Some(Self::Active),
            "draft" => Some(Self::Draft),
            "suspended" => Some(Self::Suspended),
            _ => None,
        }
    }

    /// The status of `card` of `item`: `draft` if the item is a draft, else
    /// `suspended` if the card is suspended, else `active`.
    pub fn of(item: &Item, card: &CardId, memories: &Memories) -> Self {
        if item.is_draft() {
            Self::Draft
        } else if memories.is_suspended(card) {
            Self::Suspended
        } else {
            Self::Active
        }
    }
}

/// A value of `state:`: one of the four exclusive states, or `leech`, which
/// overlaps them (see `docs/scheduling.md`, "States").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateTerm {
    State(State),
    Leech,
}

impl StateTerm {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "new" => Some(Self::State(State::New)),
            "due" => Some(Self::State(State::Due)),
            "young" => Some(Self::State(State::Young)),
            "mature" => Some(Self::State(State::Mature)),
            "leech" => Some(Self::Leech),
            _ => None,
        }
    }

    fn matches(self, id: &CardId, memories: &Memories) -> bool {
        match self {
            Self::State(state) => memories.state(id) == state,
            Self::Leech => memories.is_leech(id),
        }
    }
}

impl Selector {
    /// Parses a selector. Command-line arguments are joined with spaces
    /// before they get here.
    pub fn parse(text: &str) -> Result<Self> {
        let terms = words(text)?
            .into_iter()
            .map(Word::into_term)
            .collect::<Result<_>>()?;
        Ok(Self { terms })
    }

    /// The selector with one more term, `status:STATUS`, as some commands add
    /// (see `docs/selectors.md`, "Defaults per command").
    #[must_use]
    pub fn and_status(self, status: Status) -> Self {
        let term = Term {
            negated: false,
            filter: Filter::Status(vec![status]),
        };
        Self {
            terms: self.terms.into_iter().chain([term]).collect(),
        }
    }

    /// Whether `card` of `item` matches.
    pub fn matches(&self, item: &Item, card: &CardId, memories: &Memories) -> bool {
        self.terms
            .iter()
            .all(|term| term.filter.matches(item, card, memories) != term.negated)
    }

    /// The cards that match, with their items, in the order they are
    /// written.
    pub fn select<'a>(
        &self,
        cards: impl Iterator<Item = (&'a Item, &'a CardId)>,
        memories: &Memories,
    ) -> Vec<(&'a Item, &'a CardId)> {
        cards
            .filter(|(item, card)| self.matches(item, card, memories))
            .collect()
    }
}

/// The names `in:` matches for a note path: each folder, and the file name
/// without its extension, in NFC and lowercased.
fn names(path: &str) -> impl Iterator<Item = String> + '_ {
    let (folders, file) = path.rsplit_once('/').unwrap_or(("", path));
    let stem = Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file);
    folders
        .split('/')
        .filter(|folder| !folder.is_empty())
        .chain([stem])
        .map(|name| nfc(name).to_lowercase())
}

impl Filter {
    fn matches(&self, item: &Item, card: &CardId, memories: &Memories) -> bool {
        match self {
            Self::In(wanted) => names(item.path()).any(|name| wanted.contains(&name)),
            Self::Path(globs) => globs.is_match(nfc(item.path())),
            Self::Id(ids) => ids.iter().any(|id| match id.suffix() {
                Some(_) => id == card,
                None => id.item() == item.id(),
            }),
            Self::Status(statuses) => statuses.contains(&Status::of(item, card, memories)),
            Self::State(states) => states.iter().any(|state| state.matches(card, memories)),
            Self::Text(text) => item.search_text().contains(text.as_str()),
        }
    }
}

/// One term as written: optionally negated, and either quoted text or a
/// plain word.
#[derive(Debug)]
struct Word {
    written: String,
    negated: bool,
    quoted: bool,
    body: String,
}

impl Word {
    fn into_term(self) -> Result<Term> {
        let invalid = |reason: &str| Error::BadSelector {
            term: self.written.clone(),
            reason: reason.to_owned(),
        };
        if self.body.is_empty() {
            return Err(invalid("it is empty"));
        }
        let filter = match self.body.split_once(':') {
            Some((key, values)) if !self.quoted && is_key(key) => {
                keyed(key, values).map_err(|reason| invalid(&reason))?
            }
            Some(_) | None => Filter::Text(self.body.to_lowercase()),
        };
        Ok(Term {
            negated: self.negated,
            filter,
        })
    }
}

/// Whether `text` looks like a key: lowercase ASCII letters only.
fn is_key(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| character.is_ascii_lowercase())
}

fn keyed(key: &str, values: &str) -> std::result::Result<Filter, String> {
    let values = values.split(',').collect::<Vec<_>>();
    if values.iter().any(|value| value.is_empty()) {
        return Err("it has an empty value".to_owned());
    }
    match key {
        "in" => Ok(Filter::In(
            values
                .into_iter()
                .map(|value| nfc(value).to_lowercase())
                .collect(),
        )),
        "path" => globs(&values).map(Filter::Path),
        "id" => each(&values, |value| {
            CardId::parse(value).map_err(|error| error.to_string())
        })
        .map(Filter::Id),
        "status" => each(&values, |value| {
            Status::parse(value)
                .ok_or_else(|| format!("`{value}` is not a status; use active, draft or suspended"))
        })
        .map(Filter::Status),
        "state" => each(&values, |value| {
            StateTerm::parse(value).ok_or_else(|| {
                format!("`{value}` is not a state; use new, due, young, mature or leech")
            })
        })
        .map(Filter::State),
        _ => Err(format!(
            "`{key}:` is not a key; use one of {KEYS}, or put the term in double quotes to search for the text"
        )),
    }
}

/// Each of `values` parsed with `parse`, or the first error.
fn each<T>(
    values: &[&str],
    parse: impl Fn(&str) -> std::result::Result<T, String>,
) -> std::result::Result<Vec<T>, String> {
    values.iter().map(|value| parse(value)).collect()
}

/// Splits a selector into terms at spaces. A term may start with `-`, and
/// then with `"`, in which case it runs to the next `"`, spaces included.
fn words(text: &str) -> Result<Vec<Word>> {
    let mut chars = text.chars().peekable();
    let mut words = Vec::new();
    loop {
        while chars
            .next_if(|character| character.is_whitespace())
            .is_some()
        {}
        if chars.peek().is_none() {
            return Ok(words);
        }
        let negated = chars.next_if_eq(&'-').is_some();
        let quoted = chars.next_if_eq(&'"').is_some();
        let mut body = String::new();
        let mut closed = !quoted;
        for character in chars.by_ref() {
            if quoted && character == '"' {
                closed = true;
                break;
            }
            if !quoted && character.is_whitespace() {
                break;
            }
            body.push(character);
        }
        let written = format!(
            "{}{}{body}{}",
            if negated { "-" } else { "" },
            if quoted { "\"" } else { "" },
            if quoted && closed { "\"" } else { "" },
        );
        let invalid = |reason: &str| Error::BadSelector {
            term: written.clone(),
            reason: reason.to_owned(),
        };
        if !closed {
            return Err(invalid("its closing `\"` is missing"));
        }
        if quoted && chars.peek().is_some_and(|next| !next.is_whitespace()) {
            return Err(invalid("its closing `\"` must be followed by a space"));
        }
        words.push(Word {
            written,
            negated,
            quoted,
            body,
        });
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::note::read_items;
    use crate::{By, Config, Entry, Event, Log, Review};

    /// The notes of the tests, with their items.
    fn items() -> Vec<Item> {
        let notes = [
            (
                "japanese/Grammar/wo.md",
                "<!-- srl id:aaa1 -->\nQ: 〜をものともせず?\nA: Undeterred by (adversity).\n",
            ),
            (
                "japanese/n5.md",
                "<!-- srl id:aaa2 -->\nQ: {{c1::です}} {{c2::ます}}\n",
            ),
            (
                "kanji/food.md",
                "<!-- srl id:aaa3 draft -->\nQ: 鮪\nA: Tuna.\n",
            ),
            (
                "rust/ownership/move.md",
                "<!-- srl id:aaa4 -->\nQ: Move semantics?\nA: Ownership MOVES on assignment.\n",
            ),
        ];
        notes
            .iter()
            .flat_map(|(path, text)| {
                let (items, problems) = read_items(path, text);
                assert!(problems.is_empty(), "{problems:?}");
                items
            })
            .collect()
    }

    /// Card `aaa4` is suspended; `aaa1` was reviewed and is due.
    fn memories() -> Memories {
        let day = date(2026, 1, 1);
        let entries = vec![
            Entry {
                event: Event::Suspend(crate::CardEvent {
                    t: "2026-01-01T12:00:00+00:00".to_owned(),
                    card: CardId::parse("aaa4").unwrap(),
                }),
                day,
            },
            Entry {
                event: Event::Review(Review {
                    t: "2026-01-01T12:00:00+00:00".to_owned(),
                    card: CardId::parse("aaa1").unwrap(),
                    rating: 1,
                    response: None,
                    by: By::Human,
                    hash: String::new(),
                }),
                day,
            },
        ];
        let log = Log {
            entries,
            problems: Vec::new(),
        };
        Memories::replay(&log, &Config::default(), date(2026, 3, 1)).unwrap()
    }

    /// The IDs of the cards that `selector` picks.
    fn pick(selector: &str) -> Vec<String> {
        let selector = Selector::parse(selector).unwrap();
        let items = items();
        let cards = items
            .iter()
            .flat_map(|item| item.cards().iter().map(move |card| (item, card)));
        selector
            .select(cards, &memories())
            .into_iter()
            .map(|(_, card)| card.to_string())
            .collect()
    }

    #[test]
    fn empty_selector_matches_every_card() {
        assert_eq!(pick(""), ["aaa1", "aaa2-1", "aaa2-2", "aaa3", "aaa4"]);
    }

    #[test]
    fn in_matches_a_folder_or_the_file_name_ignoring_case() {
        assert_eq!(pick("in:grammar"), ["aaa1"]);
        assert_eq!(pick("in:japanese"), ["aaa1", "aaa2-1", "aaa2-2"]);
        assert_eq!(pick("in:food"), ["aaa3"]);
        assert_eq!(pick("in:foo"), Vec::<String>::new());
        assert_eq!(pick("in:wo.md"), Vec::<String>::new());
    }

    #[test]
    fn in_drops_only_the_file_names_extension() {
        assert_eq!(
            names("v1.2/Grammar.notes.TXT").collect::<Vec<_>>(),
            ["v1.2", "grammar.notes"]
        );
        assert_eq!(names("wo").collect::<Vec<_>>(), ["wo"]);
    }

    #[test]
    fn comma_is_or_space_is_and_minus_is_not() {
        assert_eq!(pick("in:grammar,food"), ["aaa1", "aaa3"]);
        assert_eq!(pick("in:japanese in:n5"), ["aaa2-1", "aaa2-2"]);
        assert_eq!(pick("in:japanese -in:n5"), ["aaa1"]);
    }

    #[test]
    fn path_star_stays_in_one_folder() {
        assert_eq!(pick("path:japanese/*"), ["aaa2-1", "aaa2-2"]);
        assert_eq!(pick("path:japanese/**"), ["aaa1", "aaa2-1", "aaa2-2"]);
    }

    #[test]
    fn an_item_id_stands_for_all_its_cards() {
        assert_eq!(pick("id:aaa2"), ["aaa2-1", "aaa2-2"]);
        assert_eq!(pick("id:aaa2-2,aaa3"), ["aaa2-2", "aaa3"]);
    }

    #[test]
    fn status_comes_from_the_header_and_the_log() {
        assert_eq!(pick("status:draft"), ["aaa3"]);
        assert_eq!(pick("status:suspended"), ["aaa4"]);
        assert_eq!(pick("status:active"), ["aaa1", "aaa2-1", "aaa2-2"]);
    }

    #[test]
    fn state_comes_from_the_log() {
        assert_eq!(pick("state:due"), ["aaa1"]);
        assert_eq!(pick("-state:new"), ["aaa1"]);
    }

    #[test]
    fn text_matches_the_fields_ignoring_case() {
        assert_eq!(pick("ものともせず"), ["aaa1"]);
        assert_eq!(pick("moves"), ["aaa4"]);
        assert_eq!(pick("\"moves on\""), ["aaa4"]);
        assert_eq!(pick("srl"), Vec::<String>::new());
        assert_eq!(pick("-tuna"), ["aaa1", "aaa2-1", "aaa2-2", "aaa4"]);
    }

    #[test]
    fn and_status_adds_a_term() {
        let selector = Selector::parse("in:food,rust")
            .unwrap()
            .and_status(Status::Draft);
        let items = items();
        let cards = items
            .iter()
            .flat_map(|item| item.cards().iter().map(move |card| (item, card)));
        let picked = selector.select(cards, &memories());
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].1.to_string(), "aaa3");
    }

    #[test]
    fn unknown_selector_key_is_an_error() {
        let error = Selector::parse("ni:x").unwrap_err();
        assert!(error.to_string().contains("in, path"), "{error}");
        assert!(Selector::parse("tag:x").is_err());
    }

    #[test]
    fn invalid_terms_are_errors() {
        for text in [
            "in:",
            "in:a,,b",
            "status:paused",
            "id:XYZ",
            "id:a-",
            "state:old",
            "path:[",
            "-",
            "\"\"",
            "\"open",
            "\"closed\"glued",
        ] {
            assert!(Selector::parse(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn names_match_whatever_unicode_form_they_are_in() {
        let (items, _) = read_items("cafe\u{301}/x.md", "<!-- srl id:abc -->\nQ: q\nA: a\n");
        let item = &items[0];
        let card = &item.cards()[0];
        let memories = memories();
        for selector in [
            "in:caf\u{e9}",
            "in:cafe\u{301}",
            "path:caf\u{e9}/*",
            "path:CAFE\u{301}/*",
        ] {
            let matched = Selector::parse(selector)
                .unwrap()
                .matches(item, card, &memories);
            assert_eq!(matched, !selector.starts_with("path:CAFE"), "{selector}");
        }
    }
}
