//! The built-in drills: `cloze` and `cards` (see `docs/drills.md`,
//! "Built-in drills").
//!
//! They write whole refs (`cloze:0:1`, `cards:0`), and their cards are
//! checked like any drill's.

use serde_json::Value;

use crate::Atom;
use crate::card::RawCard;

/// One built-in drill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Builtin {
    Cloze,
    Cards,
}

impl Builtin {
    pub(crate) const ALL: [Self; 2] = [Self::Cloze, Self::Cards];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Cloze => "cloze",
            Self::Cards => "cards",
        }
    }

    /// The first line of the drill's help.
    pub(crate) fn summary(self) -> &'static str {
        match self {
            Self::Cloze => "Sentences with {{marked}} spans to fill in.",
            Self::Cards => "Hand-written questions and answers.",
        }
    }

    /// The drill's cards for `atom`. Errors are plain reasons: the facet is
    /// not in the drill's shape.
    pub(crate) fn cards(self, atom: &Atom) -> Result<Vec<RawCard>, String> {
        let Some(facet) = atom.content().facets().get(self.name()) else {
            return Ok(Vec::new());
        };
        match self {
            Self::Cloze => cloze(facet),
            Self::Cards => cards(facet),
        }
    }
}

/// A piece of a cloze sentence: plain text, or a marked span and its group.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    Span { group: u32, text: String },
}

fn cloze(facet: &Value) -> Result<Vec<RawCard>, String> {
    let sentences = list_of_text(facet, "cloze")?;
    let cards = sentences
        .iter()
        .enumerate()
        .map(|(index, sentence)| {
            let pieces =
                pieces(sentence).map_err(|reason| format!("`cloze` sentence {index} {reason}"))?;
            Ok(cloze_cards(index, &pieces))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(cards.concat())
}

/// One card per group of `pieces`, in group order (see `docs/drills.md`,
/// `cloze`).
fn cloze_cards(index: usize, pieces: &[Piece]) -> Vec<RawCard> {
    let mut groups = pieces
        .iter()
        .filter_map(|piece| match piece {
            Piece::Span { group, .. } => Some(*group),
            Piece::Text(_) => None,
        })
        .collect::<Vec<_>>();
    groups.sort_unstable();
    groups.dedup();
    groups
        .into_iter()
        .map(|blanked| {
            // Each piece as it appears in the prompt and in the answer.
            let parts = pieces
                .iter()
                .scan(0, |blanks, piece| {
                    Some(match piece {
                        Piece::Span { group, text } if *group == blanked => {
                            *blanks += 1;
                            (format!("==[{blanks}]=="), format!("=={text}=="))
                        }
                        Piece::Span { text, .. } | Piece::Text(text) => {
                            (text.clone(), text.clone())
                        }
                    })
                })
                .collect::<Vec<_>>();
            let prompt = parts.iter().map(|(prompt, _)| prompt.as_str()).collect();
            let answer = parts.iter().map(|(_, answer)| answer.as_str()).collect();
            RawCard::new(format!("cloze:{index}:{blanked}"), prompt, answer)
        })
        .collect()
}

/// Splits a cloze sentence into text and spans. A span is `{{text}}` or
/// `{{N::text}}`; spans without a number get the numbers after the highest
/// given one, in order. Errors complete the sentence "`cloze` sentence I …".
fn pieces(sentence: &str) -> Result<Vec<Piece>, String> {
    let mut pieces = Vec::new();
    let mut rest = sentence;
    let mut unnumbered = Vec::new();
    while let Some((before, after)) = rest.split_once("{{") {
        let (inner, next) = after
            .split_once("}}")
            .ok_or_else(|| "has a `{{` without a closing `}}`".to_owned())?;
        if !before.is_empty() {
            pieces.push(Piece::Text(before.to_owned()));
        }
        let (group, text) = match inner.split_once("::") {
            Some((number, text))
                if !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                let group = number.parse::<u32>().map_err(|error| {
                    format!("has a group number `{number}` that is too big ({error})")
                })?;
                (Some(group), text)
            }
            Some(_) | None => (None, inner),
        };
        if text.is_empty() {
            return Err("has an empty span".to_owned());
        }
        if group.is_none() {
            unnumbered.push(pieces.len());
        }
        pieces.push(Piece::Span {
            group: group.unwrap_or(0),
            text: text.to_owned(),
        });
        rest = next;
    }
    if !rest.is_empty() {
        pieces.push(Piece::Text(rest.to_owned()));
    }
    let highest = pieces
        .iter()
        .enumerate()
        .filter(|(position, _)| !unnumbered.contains(position))
        .filter_map(|(_, piece)| match piece {
            Piece::Span { group, .. } => Some(*group),
            Piece::Text(_) => None,
        })
        .max();
    if highest.is_none() && unnumbered.is_empty() {
        return Err("has no {{…}} span".to_owned());
    }
    let first_free = highest.map_or(1, |highest| highest + 1);
    for (offset, position) in unnumbered.iter().enumerate() {
        if let Some(Piece::Span { group, .. }) = pieces.get_mut(*position) {
            let offset =
                u32::try_from(offset).map_err(|error| format!("has too many spans ({error})"))?;
            *group = first_free + offset;
        }
    }
    Ok(pieces)
}

fn cards(facet: &Value) -> Result<Vec<RawCard>, String> {
    let Value::Array(items) = facet else {
        return Err("`cards` must be a list of questions with `q` and `a`".to_owned());
    };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            hand_written(index, item).map_err(|reason| format!("`cards` item {index} {reason}"))
        })
        .collect()
}

/// One item of `cards`. Errors complete the sentence "`cards` item I …".
fn hand_written(index: usize, item: &Value) -> Result<RawCard, String> {
    let Value::Object(fields) = item else {
        return Err("is not a mapping with `q` and `a`".to_owned());
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !["q", "a"].contains(&key.as_str()))
    {
        return Err(format!("has an unknown key `{key}`; use q and a"));
    }
    let text = |key: &str| match fields.get(key) {
        None | Some(Value::Null) => Err(format!("has no `{key}`")),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(other) => Err(format!("has a `{key}` that is not text: {other}")),
    };
    Ok(RawCard::new(
        format!("cards:{index}"),
        text("q")?,
        text("a")?,
    ))
}

/// A facet value that must be a list of text. Errors are plain reasons.
fn list_of_text(value: &Value, name: &str) -> Result<Vec<String>, String> {
    let not_a_list = || format!("`{name}` must be a list of text");
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned).ok_or_else(not_a_list))
            .collect(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
            Err(not_a_list())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(builtin: Builtin, text: &str) -> Result<Vec<RawCard>, String> {
        builtin.cards(&Atom::parse("a.md".to_owned(), text.to_owned()).unwrap())
    }

    #[test]
    fn a_drill_without_its_facet_makes_no_cards() {
        for builtin in Builtin::ALL {
            assert!(
                run(builtin, "---\nid: k3m9q1\ntitle: 鮪\n---\nA fish.\n")
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn cloze_makes_one_card_per_group_per_sentence() {
        let cards = run(
            Builtin::Cloze,
            "---\nid: a7f3k2\ntitle: x\ncloze:\n  - 嵐を{{ものともせず}}、漁師たちは海へ出た。\n  - '{{1::周囲の反対}}を{{1::ものともせず}}、{{2::彼女}}は起業した。'\n---\n",
        )
        .unwrap();
        let refs = cards
            .iter()
            .map(|card| card.reference.clone().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(refs, ["cloze:0:1", "cloze:1:1", "cloze:1:2"]);
        assert_eq!(
            cards[1].prompt.as_deref(),
            Some("==[1]==を==[2]==、彼女は起業した。")
        );
        assert_eq!(
            cards[1].answer.as_deref(),
            Some("==周囲の反対==を==ものともせず==、彼女は起業した。")
        );
        assert_eq!(
            cards[2].prompt.as_deref(),
            Some("周囲の反対をものともせず、==[1]==は起業した。")
        );
    }

    #[test]
    fn unnumbered_spans_come_after_the_highest_number() {
        let pieces = pieces("{{a}} {{2::b}} {{c}}").unwrap();
        let groups = pieces
            .iter()
            .filter_map(|piece| match piece {
                Piece::Span { group, .. } => Some(*group),
                Piece::Text(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(groups, [3, 2, 4]);
    }

    #[test]
    fn a_colon_pair_without_a_number_is_text() {
        assert_eq!(
            pieces("use {{std::move}}").unwrap(),
            [
                Piece::Text("use ".to_owned()),
                Piece::Span {
                    group: 1,
                    text: "std::move".to_owned()
                }
            ]
        );
    }

    #[test]
    fn bad_cloze_sentences_are_errors() {
        for sentence in ["no span", "open {{span", "empty {{}}", "empty {{1::}}"] {
            assert!(pieces(sentence).is_err(), "accepted {sentence}");
        }
        assert!(cloze(&json!("not a list")).is_err());
        assert!(cloze(&json!([1])).is_err());
    }

    #[test]
    fn hand_written_cards_are_questions_and_answers() {
        let cards = cards(&json!([
            {"q": "Which particle?", "a": "を"},
            {"q": "Explain.", "a": "Because."}
        ]))
        .unwrap();
        assert_eq!(cards[0].prompt.as_deref(), Some("Which particle?"));
        assert_eq!(cards[0].answer.as_deref(), Some("を"));
        assert_eq!(cards[1].reference.as_deref(), Some("cards:1"));
        assert!(super::cards(&json!([])).unwrap().is_empty());
        for bad in [
            json!({"q": "x"}),
            json!([{"q": "x"}]),
            json!([{"q": "x", "a": "y", "hint": "z"}]),
            json!([{"q": "x", "a": "y", "accept": ["y"]}]),
            json!([{"q": "x", "a": "y", "explanation": "z"}]),
            json!([{"q": "x", "a": 1}]),
        ] {
            assert!(super::cards(&bad).is_err(), "accepted {bad}");
        }
    }
}
