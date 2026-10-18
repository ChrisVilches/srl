//! The built-in drills: `reveal`, `reverse`, `cloze`, `cards` and
//! `confusable` (see `docs/drills.md`, "Built-in drills").
//!
//! They write whole refs (`cloze:0:1`, `reveal`), and their cards are
//! checked like any drill's.

use jiff::civil::Date;
use serde_json::Value;

use crate::card::RawCard;
use crate::{Atom, Atoms, Id, random};

/// One built-in drill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Builtin {
    Reveal,
    Reverse,
    Cloze,
    Cards,
    Confusable,
}

/// The most other atoms a `confusable` card offers as choices.
const MAX_CONFUSABLE: usize = 3;

impl Builtin {
    pub(crate) const ALL: [Self; 5] = [
        Self::Reveal,
        Self::Reverse,
        Self::Cloze,
        Self::Cards,
        Self::Confusable,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Reveal => "reveal",
            Self::Reverse => "reverse",
            Self::Cloze => "cloze",
            Self::Cards => "cards",
            Self::Confusable => "confusable",
        }
    }

    /// The first line of the drill's help.
    pub(crate) fn summary(self) -> &'static str {
        match self {
            Self::Reveal => "Shows the cue, then the answer or the body; you rate yourself.",
            Self::Reverse => {
                "Asks the other way round: shows the answer or the body, you recall the cue."
            }
            Self::Cloze => "Sentences with {{marked}} spans to fill in.",
            Self::Cards => "Hand-written questions and answers.",
            Self::Confusable => {
                "Multiple choice between this atom and atoms easily confused with it."
            }
        }
    }

    /// The drill's cards for `atom`. `atoms` is the whole store, and `day`
    /// the study day. Errors are plain reasons: the facet is not in the
    /// drill's shape.
    pub(crate) fn cards(
        self,
        atom: &Atom,
        atoms: &Atoms,
        day: Date,
    ) -> Result<Vec<RawCard>, String> {
        let facet = atom.content().facets().get(self.name());
        match (self, facet) {
            (Self::Reveal, _) => Ok(vec![reveal(atom)]),
            (Self::Reverse, Some(facet)) => reverse(atom, facet),
            (Self::Cloze, Some(facet)) => cloze(facet),
            (Self::Cards, Some(facet)) => cards(facet),
            (Self::Confusable, Some(facet)) => confusable(atom, facet, atoms, day),
            (Self::Reverse | Self::Cloze | Self::Cards | Self::Confusable, None) => Ok(Vec::new()),
        }
    }
}

/// The atom's `answer`, or else its body, and whether it was the body.
fn answer_or_body(atom: &Atom) -> (&str, bool) {
    let content = atom.content();
    content
        .answer()
        .map_or((content.body(), true), |answer| (answer, false))
}

/// The `explanation` of a card whose answer or prompt already shows the body:
/// none, rather than the body a second time.
fn no_explanation(card: RawCard) -> RawCard {
    RawCard {
        explanation: Some(String::new()),
        ..card
    }
}

fn reveal(atom: &Atom) -> RawCard {
    let (answer, is_body) = answer_or_body(atom);
    let card = RawCard::new(
        "reveal".to_owned(),
        atom.content().cue().to_owned(),
        answer.to_owned(),
        "self",
    );
    if is_body { no_explanation(card) } else { card }
}

fn reverse(atom: &Atom, facet: &Value) -> Result<Vec<RawCard>, String> {
    match facet {
        Value::Bool(false) => Ok(Vec::new()),
        Value::Bool(true) => {
            let (prompt, is_body) = answer_or_body(atom);
            if prompt.trim().is_empty() {
                return Err("`reverse` needs an `answer` or a body to ask about".to_owned());
            }
            let card = RawCard::new(
                "reverse".to_owned(),
                prompt.to_owned(),
                atom.content().cue().to_owned(),
                "self",
            );
            Ok(vec![if is_body { no_explanation(card) } else { card }])
        }
        Value::Null | Value::Number(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => {
            Err(format!("`reverse` must be true or false, but is {facet}"))
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
            // Each piece as it appears in the prompt and in the answer, and
            // what its blank accepts if it is one.
            let parts = pieces
                .iter()
                .scan(0, |blanks, piece| {
                    Some(match piece {
                        Piece::Span { group, text } if *group == blanked => {
                            *blanks += 1;
                            (
                                format!("==[{blanks}]=="),
                                format!("=={text}=="),
                                Some(vec![text.clone()]),
                            )
                        }
                        Piece::Span { text, .. } | Piece::Text(text) => {
                            (text.clone(), text.clone(), None)
                        }
                    })
                })
                .collect::<Vec<_>>();
            let prompt = parts.iter().map(|(prompt, _, _)| prompt.as_str()).collect();
            let answer = parts.iter().map(|(_, answer, _)| answer.as_str()).collect();
            RawCard {
                accept: Some(
                    parts
                        .into_iter()
                        .filter_map(|(_, _, accept)| accept)
                        .collect(),
                ),
                ..RawCard::new(format!("cloze:{index}:{blanked}"), prompt, answer, "exact")
            }
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
        .find(|key| !["q", "a", "accept", "explanation"].contains(&key.as_str()))
    {
        return Err(format!(
            "has an unknown key `{key}`; use q, a, accept and explanation"
        ));
    }
    let text = |key: &str| match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(other) => Err(format!("has a `{key}` that is not text: {other}")),
    };
    let question = text("q")?.ok_or_else(|| "has no `q`".to_owned())?;
    let answer = text("a")?.ok_or_else(|| "has no `a`".to_owned())?;
    let accept = match fields.get("accept") {
        None | Some(Value::Null) => None,
        Some(accept) => Some(list_of_text(accept, "accept")?),
    };
    let grading = if accept.is_some() { "exact" } else { "self" };
    Ok(RawCard {
        accept: accept.map(|accept| vec![accept]),
        explanation: text("explanation")?,
        ..RawCard::new(format!("cards:{index}"), question, answer, grading)
    })
}

fn confusable(
    atom: &Atom,
    facet: &Value,
    atoms: &Atoms,
    day: Date,
) -> Result<Vec<RawCard>, String> {
    let others = list_of_text(facet, "confusable")?
        .iter()
        .map(|text| {
            let id =
                Id::parse(text).map_err(|error| format!("`confusable` lists {text}: {error}"))?;
            if &id == atom.id() {
                return Err("`confusable` lists the atom itself".to_owned());
            }
            atoms
                .get(&id)
                .ok_or_else(|| format!("`confusable` lists {id}, which no atom has"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if others.is_empty() {
        return Ok(Vec::new());
    }
    let (shown, is_body) = answer_or_body(atom);
    if shown.trim().is_empty() {
        return Err("`confusable` needs an `answer` or a body to ask about".to_owned());
    }
    let cue = atom.content().cue().to_owned();
    let choices = std::iter::once(cue.clone())
        .chain(
            others
                .iter()
                .take(MAX_CONFUSABLE)
                .map(|other| other.content().cue().to_owned()),
        )
        .collect::<Vec<_>>();
    let seed = random::seed(&format!("{}:{day}", atom.id()));
    let card = RawCard {
        accept: Some(vec![vec![cue.clone()]]),
        choices: Some(random::shuffle(choices, seed)),
        ..RawCard::new(
            "confusable".to_owned(),
            format!("Which one matches?\n\n{shown}"),
            cue,
            "choice",
        )
    };
    Ok(vec![if is_body { no_explanation(card) } else { card }])
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

    fn atom(text: &str) -> Atom {
        Atom::parse("a.md".to_owned(), text.to_owned()).unwrap()
    }

    fn store(atoms: Vec<Atom>) -> Atoms {
        Atoms {
            atoms,
            problems: Vec::new(),
        }
    }

    fn day() -> Date {
        Date::constant(2026, 10, 26)
    }

    fn run(builtin: Builtin, text: &str) -> Result<Vec<RawCard>, String> {
        let atom = atom(text);
        builtin.cards(&atom, &store(vec![atom.clone()]), day())
    }

    #[test]
    fn reveal_shows_the_answer_and_explains_with_the_body() {
        let cards = run(
            Builtin::Reveal,
            "---\nid: k3m9q1\ncue: 鮪\nanswer: tuna\n---\nA fish.\n",
        )
        .unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].prompt.as_deref(), Some("鮪"));
        assert_eq!(cards[0].answer.as_deref(), Some("tuna"));
        assert_eq!(cards[0].explanation, None);
    }

    #[test]
    fn reveal_without_an_answer_shows_the_body_and_no_explanation() {
        let cards = run(Builtin::Reveal, "---\nid: k3m9q1\ncue: 鮪\n---\nA fish.\n").unwrap();
        assert_eq!(cards[0].answer.as_deref(), Some("A fish.\n"));
        assert_eq!(cards[0].explanation.as_deref(), Some(""));
    }

    #[test]
    fn reverse_asks_the_other_way_round() {
        let cards = run(
            Builtin::Reverse,
            "---\nid: k3m9q1\ncue: 鮪\nanswer: tuna\nreverse: true\n---\n",
        )
        .unwrap();
        assert_eq!(cards[0].prompt.as_deref(), Some("tuna"));
        assert_eq!(cards[0].answer.as_deref(), Some("鮪"));
        assert!(
            run(
                Builtin::Reverse,
                "---\nid: k3m9q1\ncue: 鮪\nreverse: false\n---\n"
            )
            .unwrap()
            .is_empty()
        );
        assert!(
            run(
                Builtin::Reverse,
                "---\nid: k3m9q1\ncue: 鮪\nreverse: true\n---\n"
            )
            .is_err()
        );
        assert!(
            run(
                Builtin::Reverse,
                "---\nid: k3m9q1\ncue: 鮪\nanswer: x\nreverse: yes\n---\n"
            )
            .is_err()
        );
    }

    #[test]
    fn cloze_makes_one_card_per_group_per_sentence() {
        let cards = run(
            Builtin::Cloze,
            "---\nid: a7f3k2\ncue: x\ncloze:\n  - 嵐を{{ものともせず}}、漁師たちは海へ出た。\n  - '{{1::周囲の反対}}を{{1::ものともせず}}、{{2::彼女}}は起業した。'\n---\n",
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
            cards[1].accept,
            Some(vec![
                vec!["周囲の反対".to_owned()],
                vec!["ものともせず".to_owned()]
            ])
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
    fn hand_written_cards_are_exact_with_accept_and_self_without() {
        let cards = cards(&json!([
            {"q": "Which particle?", "a": "を", "accept": ["を"]},
            {"q": "Explain.", "a": "Because.", "explanation": ""}
        ]))
        .unwrap();
        assert_eq!(cards[0].grading.as_deref(), Some("exact"));
        assert_eq!(cards[0].accept, Some(vec![vec!["を".to_owned()]]));
        assert_eq!(cards[1].grading.as_deref(), Some("self"));
        assert_eq!(cards[1].reference.as_deref(), Some("cards:1"));
        assert_eq!(cards[1].explanation.as_deref(), Some(""));
        for bad in [
            json!({"q": "x"}),
            json!([{"q": "x"}]),
            json!([{"q": "x", "a": "y", "hint": "z"}]),
            json!([{"q": "x", "a": "y", "accept": "y"}]),
        ] {
            assert!(super::cards(&bad).is_err(), "accepted {bad}");
        }
    }

    #[test]
    fn confusable_offers_the_cues_in_an_order_fixed_for_the_day() {
        let this =
            atom("---\nid: aaaaa1\ncue: 鮪\nanswer: tuna\nconfusable: [aaaaa2, aaaaa3]\n---\n");
        let others = [
            atom("---\nid: aaaaa2\ncue: 鯖\n---\n"),
            atom("---\nid: aaaaa3\ncue: 鮭\n---\n"),
        ];
        let atoms = store(std::iter::once(this.clone()).chain(others).collect());
        let cards = Builtin::Confusable.cards(&this, &atoms, day()).unwrap();
        let card = &cards[0];
        assert_eq!(card.prompt.as_deref(), Some("Which one matches?\n\ntuna"));
        assert_eq!(card.accept, Some(vec![vec!["鮪".to_owned()]]));
        let mut choices = card.choices.clone().unwrap();
        assert_eq!(
            Builtin::Confusable.cards(&this, &atoms, day()).unwrap()[0].choices,
            Some(choices.clone())
        );
        choices.sort();
        assert_eq!(choices, ["鮪", "鮭", "鯖"]);
    }

    #[test]
    fn confusable_refuses_unknown_atoms_and_itself() {
        for facet in ["[zzzzzz]", "[aaaaa1]", "aaaaa2"] {
            let text = format!("---\nid: aaaaa1\ncue: x\nanswer: y\nconfusable: {facet}\n---\n");
            assert!(run(Builtin::Confusable, &text).is_err(), "accepted {facet}");
        }
    }
}
