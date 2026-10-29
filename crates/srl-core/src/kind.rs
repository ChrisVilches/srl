//! Kinds: how an item's fields become cards (see `docs/cards.md`,
//! "Kinds"): `qa`, built in, and the kind programs of `[kinds]`.

mod cloze;
mod program;

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::card::{Card, CardId};
use crate::context::Context;
use crate::note::{Field, ItemId};
use crate::{Config, Error, Index, Store};

use cloze::Piece;
pub(crate) use cloze::{has_deletion, has_near_deletion, has_revealing_hint};
pub(crate) use program::Programs;

/// A kind of item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A question and an answer, with optional cloze deletions.
    Qa,
    /// A kind program, by its name in `[kinds]`.
    Program(String),
}

impl Kind {
    /// The kind named `name`: `qa`, or a kind program, which `is_program`
    /// says yes to. Errors are plain reasons.
    pub(crate) fn parse(name: &str, is_program: impl Fn(&str) -> bool) -> Result<Self, String> {
        match name {
            "qa" => Ok(Self::Qa),
            other if is_program(other) => Ok(Self::Program(other.to_owned())),
            other => Err(format!(
                "there is no kind `{other}`; the kinds are qa (default) and those named in \
                 `[kinds]` of .srl/config.toml"
            )),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Qa => "qa",
            Self::Program(name) => name,
        }
    }
}

/// An item to ask its kind about: its kind, ID and fields. No key is
/// written twice in `fields`.
pub(crate) struct Asked<'a> {
    pub(crate) kind: &'a Kind,
    pub(crate) id: &'a ItemId,
    pub(crate) fields: &'a [Field],
}

/// What an item's kind says of it.
pub(crate) enum Answer {
    /// The item is valid, and gives these card IDs, in groups of siblings
    /// (see `docs/scheduling.md`, "Siblings").
    Cards(Vec<Vec<CardId>>),
    /// The item is invalid, for these reasons.
    Invalid(Vec<String>),
}

/// Where the answers of kind programs' `validate` and `groups` calls are
/// kept (see `docs/cards.md`, "How srl calls them").
pub(crate) enum Cache<'a> {
    /// Nowhere: every item is asked about.
    None,
    /// In the index: only the items without a kept answer are asked about.
    Use(&'a mut Index),
    /// In the index, asking about every item and keeping the new answers.
    Refresh(&'a mut Index),
}

/// Asks each item's kind about it (see `docs/cards.md`, "Kind programs"):
/// `qa` here, and each kind program in one `validate` call for all of its
/// items, then one `groups` call for the items `validate` accepted, except
/// for the items `cache` has answers for. The answers are in the order of
/// `items`. An item whose program failed as a whole gets none; the
/// failures, one per kind program, come second.
pub(crate) fn check(
    items: &[Asked<'_>],
    programs: &Programs,
    cache: &mut Cache<'_>,
) -> (Vec<Option<Answer>>, Vec<Error>) {
    let mut answers = items
        .iter()
        .map(|asked| match asked.kind {
            Kind::Qa => Some(match qa(asked.id, asked.fields) {
                Ok(cards) => Answer::Cards(vec![cards.into_iter().map(|card| card.id).collect()]),
                Err(reason) => Answer::Invalid(vec![reason]),
            }),
            Kind::Program(_) => None,
        })
        .collect::<Vec<_>>();
    let mut failures = Vec::new();
    for (name, asked) in by_program(items, |asked| asked.kind) {
        let inputs = asked
            .iter()
            .map(|(_, asked)| Value::Object(input(asked.id, asked.fields)))
            .collect::<Vec<_>>();
        let verdicts = match programs.validate(name, &inputs, cache) {
            Ok(verdicts) => verdicts,
            Err(reason) => {
                failures.push(failed(name, reason));
                continue;
            }
        };
        // The items `validate` accepted: their places and IDs, and their
        // inputs, for `groups`.
        let mut valid = Vec::new();
        let mut valid_inputs = Vec::new();
        for (((at, asked), input), reasons) in asked.into_iter().zip(inputs).zip(verdicts) {
            if reasons.is_empty() {
                valid.push((at, asked.id));
                valid_inputs.push(input);
            } else {
                set(&mut answers, at, Answer::Invalid(reasons));
            }
        }
        if valid.is_empty() {
            continue;
        }
        let ids = valid.iter().map(|&(_, id)| id).collect::<Vec<_>>();
        match programs.groups(name, &ids, &valid_inputs, cache) {
            Ok(groups) => {
                for ((at, _), groups) in valid.into_iter().zip(groups) {
                    let answer = match groups {
                        Ok(groups) => Answer::Cards(groups),
                        Err(reason) => Answer::Invalid(vec![reason]),
                    };
                    set(&mut answers, at, answer);
                }
            }
            Err(reason) => failures.push(failed(name, reason)),
        }
    }
    (answers, failures)
}

/// A card to make: its ID, and its item's kind and fields, which [`check`]
/// accepted and gave the card for, and the item's context in mode `prose`.
pub(crate) struct ToMake<'a> {
    pub(crate) card: &'a CardId,
    pub(crate) kind: &'a Kind,
    pub(crate) fields: &'a [Field],
    pub(crate) context: Option<&'a Context>,
}

/// Makes each card of `cards` for the study day `day`: `qa` here, and each
/// kind program in one `make` call for all of its cards. In the order of
/// `cards`: each card, or why it can't be made, as a plain reason.
pub(crate) fn make(
    cards: &[ToMake<'_>],
    programs: &Programs,
    day: Date,
) -> Vec<Result<Card, String>> {
    let mut made = cards
        .iter()
        .map(|wanted| match wanted.kind {
            Kind::Qa => Some(Ok(qa(wanted.card.item(), wanted.fields)
                .expect("the fields of a qa item should have been checked when read")
                .into_iter()
                .find(|card| &card.id == wanted.card)
                .expect("a qa item should give the cards it gave when read"))),
            Kind::Program(_) => None,
        })
        .collect::<Vec<_>>();
    for (name, wanted) in by_program(cards, |wanted| wanted.kind) {
        let inputs = wanted
            .iter()
            .map(|(_, wanted)| {
                let mut input = input(wanted.card.item(), wanted.fields);
                input.insert("suffix".to_owned(), Value::from(wanted.card.suffix()));
                input.insert(
                    "context".to_owned(),
                    Value::from(wanted.context.map(|context| context.text.as_str())),
                );
                input.insert(
                    "context_format".to_owned(),
                    Value::from(wanted.context.map(|context| context.format.as_str())),
                );
                input.insert("day".to_owned(), Value::from(day.to_string()));
                Value::Object(input)
            })
            .collect::<Vec<_>>();
        let ids = wanted
            .iter()
            .map(|(_, wanted)| wanted.card)
            .collect::<Vec<_>>();
        match programs.make(name, &ids, &inputs) {
            Ok(results) => {
                for ((at, _), result) in wanted.into_iter().zip(results) {
                    set(&mut made, at, result);
                }
            }
            Err(reason) => {
                for (at, _) in wanted {
                    set(&mut made, at, Err(reason.clone()));
                }
            }
        }
    }
    made.into_iter()
        .map(|card| card.expect("every card should have been made or failed"))
        .collect()
}

/// The things of `things` whose kind is a kind program, each with its
/// place, by the program's name.
fn by_program<'a, T>(
    things: &'a [T],
    kind: impl Fn(&'a T) -> &'a Kind,
) -> BTreeMap<&'a str, Vec<(usize, &'a T)>> {
    let mut found = BTreeMap::<_, Vec<_>>::new();
    for (at, thing) in things.iter().enumerate() {
        if let Kind::Program(name) = kind(thing) {
            found.entry(name.as_str()).or_default().push((at, thing));
        }
    }
    found
}

/// Puts `value` in place `at` of `slots`.
fn set<T>(slots: &mut [Option<T>], at: usize, value: T) {
    if let Some(slot) = slots.get_mut(at) {
        *slot = Some(value);
    }
}

/// The failure of the program of kind `name` as a whole.
fn failed(name: &str, reason: String) -> Error {
    Error::KindFailed {
        kind: name.to_owned(),
        reason,
    }
}

/// What a kind is for and which fields it takes, for people and editors
/// (see `docs/cards.md`, "Describing a kind"). It only informs: an item is
/// checked by its kind, never against its description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KindDescription {
    pub name: String,
    pub description: String,
    pub fields: Vec<FieldDescription>,
}

/// One field of a [`KindDescription`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDescription {
    pub name: String,
    pub description: String,
}

/// Every kind of the store, described: `qa`, then the kind programs by
/// name. Fails at the first program that can't describe its kind.
pub fn kinds(store: &Store, config: &Config) -> crate::Result<Vec<KindDescription>> {
    let programs = Programs::new(store, config);
    let mut described = vec![qa_description()];
    for name in programs.names() {
        let (description, fields) =
            programs
                .describe(name)
                .map_err(|reason| Error::DescribeFailed {
                    kind: name.to_owned(),
                    reason,
                })?;
        described.push(KindDescription {
            name: name.to_owned(),
            description,
            fields,
        });
    }
    Ok(described)
}

/// The description of `qa` (see `docs/cards.md`, "`qa`").
fn qa_description() -> KindDescription {
    let field = |name: &str, description: &str| FieldDescription {
        name: name.to_owned(),
        description: description.to_owned(),
    };
    KindDescription {
        name: "qa".to_owned(),
        description: "A question and an answer, with optional cloze deletions.".to_owned(),
        fields: vec![
            field(
                "Q",
                "The question. May contain cloze deletions, {{c1::text}} or \
                 {{c1::text::hint}}: one card per number.",
            ),
            field(
                "A",
                "The answer. Optional when Q has cloze deletions, and never contains any.",
            ),
        ],
    }
}

/// What a kind program gets for an item: its ID and fields, the fields as
/// in `srl show --json` (see `docs/cards.md`, "Kind programs").
fn input(id: &ItemId, fields: &[Field]) -> Map<String, Value> {
    let fields = fields
        .iter()
        .map(|field| (field.key.clone(), Value::from(field.text.as_str())))
        .collect::<Map<_, _>>();
    Map::from_iter([
        ("id".to_owned(), Value::from(id.as_str())),
        ("fields".to_owned(), Value::Object(fields)),
    ])
}

/// The cards of a `qa` item (see `docs/cards.md`, "`qa`"). No key is
/// written twice in `fields`.
fn qa(id: &ItemId, fields: &[Field]) -> Result<Vec<Card>, String> {
    if let Some(other) = fields
        .iter()
        .find(|field| !matches!(field.key.as_str(), "Q" | "A"))
    {
        return Err(format!(
            "a qa item has the fields Q and A, not {}; rename or remove it",
            other.key
        ));
    }
    let text = |key: &str| {
        fields
            .iter()
            .find(|field| field.key == key)
            .map(|field| field.text.as_str())
    };
    let question = text("Q")
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| "a qa item needs a question in `Q:`".to_owned())?;
    let answer = text("A").filter(|text| !text.trim().is_empty());
    if answer.is_some_and(has_deletion) {
        return Err(
            "cloze deletions go in Q only; write the text of A without `{{c…::…}}`".to_owned(),
        );
    }
    let pieces = cloze::pieces(question)?;
    let numbers = pieces
        .iter()
        .filter_map(|piece| match piece {
            Piece::Deletion(deletion) => Some(deletion.number),
            Piece::Text(_) => None,
        })
        .collect::<BTreeSet<_>>();
    if numbers.is_empty() {
        let answer = answer.ok_or_else(|| {
            "a qa item needs an answer in `A:`, unless Q has cloze deletions".to_owned()
        })?;
        return Ok(vec![Card {
            id: CardId::new(id.clone(), None),
            prompt: question.to_owned(),
            prompt_revealed: None,
            answer: Some(answer.to_owned()),
        }]);
    }
    Ok(numbers
        .into_iter()
        .map(|number| Card {
            id: CardId::new(id.clone(), Some(number.to_string())),
            prompt: render(&pieces, |deletion| {
                (deletion.number == number)
                    .then(|| format!("==[{}]==", deletion.hint.unwrap_or("...")))
            }),
            prompt_revealed: Some(render(&pieces, |deletion| {
                (deletion.number == number).then(|| format!("=={}==", deletion.text))
            })),
            answer: answer.map(str::to_owned),
        })
        .collect())
}

/// The text of `pieces`, with each deletion replaced by what `shown` gives
/// for it, or by its text.
fn render(pieces: &[Piece<'_>], shown: impl Fn(&cloze::Deletion<'_>) -> Option<String>) -> String {
    pieces
        .iter()
        .map(|piece| match piece {
            Piece::Text(text) => (*text).to_owned(),
            Piece::Deletion(deletion) => {
                shown(deletion).unwrap_or_else(|| deletion.text.to_owned())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> Vec<Field> {
        pairs
            .iter()
            .map(|(key, text)| Field {
                key: (*key).to_owned(),
                text: (*text).to_owned(),
            })
            .collect()
    }

    /// The cards of item `k3m9q1` with these fields, as JSON (see
    /// `docs/cards.md`, "Format").
    fn cards(pairs: &[(&str, &str)]) -> Result<Vec<Value>, String> {
        let fields = fields(pairs);
        let wanted = groups(&fields)?.concat();
        let wanted = wanted
            .iter()
            .map(|card| ToMake {
                card,
                kind: &Kind::Qa,
                fields: &fields,
                context: None,
            })
            .collect::<Vec<_>>();
        Ok(make(
            &wanted,
            &Programs::default(),
            jiff::civil::date(2026, 10, 27),
        )
        .into_iter()
        .map(|card| serde_json::to_value(card.unwrap()).unwrap())
        .collect())
    }

    /// The card IDs of a `qa` item `k3m9q1` with `fields`, in groups, or
    /// why it is invalid.
    fn groups(fields: &[Field]) -> Result<Vec<Vec<CardId>>, String> {
        let id = ItemId::parse("k3m9q1").unwrap();
        let asked = Asked {
            kind: &Kind::Qa,
            id: &id,
            fields,
        };
        let answer = check(&[asked], &Programs::default(), &mut Cache::None)
            .0
            .pop()
            .flatten()
            .expect("qa always answers");
        match answer {
            Answer::Cards(groups) => Ok(groups),
            Answer::Invalid(reasons) => Err(reasons.join("; ")),
        }
    }

    #[test]
    fn the_cards_of_a_qa_item_are_one_group_of_siblings() {
        let groups = groups(&fields(&[("Q", "{{c1::a}} {{c2::b}}")])).unwrap();
        let groups = groups
            .iter()
            .map(|group| group.iter().map(ToString::to_string).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        assert_eq!(groups, [["k3m9q1.1", "k3m9q1.2"]]);
    }

    #[test]
    fn a_question_and_an_answer_give_one_card_with_the_item_id() {
        assert_eq!(
            cards(&[("Q", "What?"), ("A", "That.")]).unwrap(),
            [serde_json::json!({
                "id": "k3m9q1", "prompt": "What?", "prompt_revealed": null, "answer": "That.",
            })]
        );
    }

    #[test]
    fn each_cloze_number_gives_a_card_as_in_anki() {
        let made = cards(&[
            (
                "Q",
                "{{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。",
            ),
            ("A", "Despite everyone's opposition, she started a company."),
        ])
        .unwrap();
        assert_eq!(
            made,
            [
                serde_json::json!({
                    "id": "k3m9q1.1",
                    "prompt": "==[...]==を==[...]==、彼女は起業した。",
                    "prompt_revealed": "==周囲の反対==を==ものともせず==、彼女は起業した。",
                    "answer": "Despite everyone's opposition, she started a company.",
                }),
                serde_json::json!({
                    "id": "k3m9q1.2",
                    "prompt": "周囲の反対をものともせず、==[who]==は起業した。",
                    "prompt_revealed": "周囲の反対をものともせず、==彼女==は起業した。",
                    "answer": "Despite everyone's opposition, she started a company.",
                }),
            ]
        );
    }

    #[test]
    fn a_cloze_item_needs_no_answer_and_numbers_are_in_order() {
        let made = cards(&[("Q", "{{c10::a}} {{c2::b}}")]).unwrap();
        let ids = made
            .iter()
            .map(|card| card["id"].clone())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["k3m9q1.2", "k3m9q1.10"]);
        assert_eq!(
            made[0],
            serde_json::json!({
                "id": "k3m9q1.2", "prompt": "a ==[...]==", "prompt_revealed": "a ==b==", "answer": null,
            })
        );
    }

    #[test]
    fn invalid_fields_are_errors() {
        for pairs in [
            &[("A", "x")][..],
            &[("Q", "x")],
            &[("Q", " "), ("A", "x")],
            &[("Q", "x"), ("A", "y"), ("B", "z")],
            &[("Q", "{{c1::x}}"), ("A", "{{c1::y}}")],
            &[("Q", "{{c1::x"), ("A", "y")],
        ] {
            assert!(cards(pairs).is_err(), "accepted {pairs:?}");
        }
    }
}
