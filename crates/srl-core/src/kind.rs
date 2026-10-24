//! Kinds: how an item's fields become cards (see `docs/cards.md`,
//! "Kinds"). There is one, `qa`.

mod cloze;

use std::collections::BTreeSet;

use crate::card::{Card, CardId};
use crate::note::{Field, ItemId};

use cloze::Piece;
pub(crate) use cloze::has_deletion;

/// A kind of item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A question and an answer, with optional cloze deletions.
    Qa,
}

impl Kind {
    /// The kind named `name`. Errors are plain reasons.
    pub(crate) fn parse(name: &str) -> Result<Self, String> {
        match name {
            "qa" => Ok(Self::Qa),
            other => Err(format!(
                "there is no kind `{other}`; the only kind is qa, which is also the default"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qa => "qa",
        }
    }

    /// The card IDs of an item of this kind with ID `id` and these fields,
    /// in groups of siblings (see `docs/scheduling.md`, "Siblings"). No key
    /// is written twice in `fields`. Errors are plain reasons.
    pub(crate) fn groups(self, id: &ItemId, fields: &[Field]) -> Result<Vec<Vec<CardId>>, String> {
        match self {
            Self::Qa => Ok(vec![
                qa(id, fields)?.into_iter().map(|card| card.id).collect(),
            ]),
        }
    }

    /// The card `card` of an item of this kind with these fields, which
    /// [`Kind::groups`] accepted and gave `card` for.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "kind programs, which can fail, are about to be added"
    )]
    pub(crate) fn card(self, card: &CardId, fields: &[Field]) -> Result<Card, String> {
        match self {
            Self::Qa => Ok(qa(card.item(), fields)
                .expect("the fields of a qa item should have been checked when read")
                .into_iter()
                .find(|made| &made.id == card)
                .expect("a qa item should give the cards it gave when read")),
        }
    }
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
            prompt_revealed: question.to_owned(),
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
            prompt_revealed: render(&pieces, |deletion| {
                (deletion.number == number).then(|| format!("=={}==", deletion.text))
            }),
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
    fn cards(pairs: &[(&str, &str)]) -> Result<Vec<serde_json::Value>, String> {
        let id = ItemId::parse("k3m9q1").unwrap();
        let fields = fields(pairs);
        Ok(Kind::Qa
            .groups(&id, &fields)?
            .concat()
            .iter()
            .map(|card| serde_json::to_value(Kind::Qa.card(card, &fields).unwrap()).unwrap())
            .collect())
    }

    #[test]
    fn the_cards_of_a_qa_item_are_one_group_of_siblings() {
        let id = ItemId::parse("k3m9q1").unwrap();
        let groups = Kind::Qa
            .groups(&id, &fields(&[("Q", "{{c1::a}} {{c2::b}}")]))
            .unwrap();
        let groups = groups
            .iter()
            .map(|group| group.iter().map(ToString::to_string).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        assert_eq!(groups, [["k3m9q1-1", "k3m9q1-2"]]);
    }

    #[test]
    fn a_question_and_an_answer_give_one_card_with_the_item_id() {
        assert_eq!(
            cards(&[("Q", "What?"), ("A", "That.")]).unwrap(),
            [serde_json::json!({
                "id": "k3m9q1", "prompt": "What?", "prompt_revealed": "What?", "answer": "That.",
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
                    "id": "k3m9q1-1",
                    "prompt": "==[...]==を==[...]==、彼女は起業した。",
                    "prompt_revealed": "==周囲の反対==を==ものともせず==、彼女は起業した。",
                    "answer": "Despite everyone's opposition, she started a company.",
                }),
                serde_json::json!({
                    "id": "k3m9q1-2",
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
        assert_eq!(ids, ["k3m9q1-2", "k3m9q1-10"]);
        assert_eq!(
            made[0],
            serde_json::json!({
                "id": "k3m9q1-2", "prompt": "a ==[...]==", "prompt_revealed": "a ==b==", "answer": null,
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
