//! Cards: the fixed format between drills and frontends (see `docs/cards.md`).

use serde::{Deserialize, Serialize};

use crate::{Atom, Atoms, Id};

/// One ready-to-show question about one or more atoms, checked against the
/// rules of `docs/cards.md`.
///
/// Serializes to the card format, with every field present (`null` where a
/// field doesn't apply).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    atoms: Vec<Id>,
    drill: String,
    reference: String,
    prompt: String,
    answer: String,
    grading: Grading,
    explanation: Option<String>,
}

/// How an answer to a card is judged (see `docs/grading.md`), with the data
/// each mode needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grading {
    /// You rate yourself; nothing is checked.
    SelfRated,
    /// Typed answers, one per blank. Each blank accepts any of its strings.
    Exact { accept: Vec<Vec<String>> },
    /// One of `choices`; `accept` has one entry, the accepted choices.
    Choice {
        accept: Vec<Vec<String>>,
        choices: Vec<String>,
    },
    /// Free text, judged by the external grader.
    Cmd,
}

impl Grading {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SelfRated => "self",
            Self::Exact { .. } => "exact",
            Self::Choice { .. } => "choice",
            Self::Cmd => "cmd",
        }
    }
}

/// A card as written, before it is checked: every field optional, and no
/// field that the format doesn't have.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawCard {
    pub(crate) atoms: Option<Vec<String>>,
    pub(crate) drill: Option<String>,
    #[serde(rename = "ref")]
    pub(crate) reference: Option<String>,
    pub(crate) prompt: Option<String>,
    pub(crate) answer: Option<String>,
    pub(crate) grading: Option<String>,
    pub(crate) accept: Option<Vec<Vec<String>>>,
    pub(crate) choices: Option<Vec<String>>,
    pub(crate) explanation: Option<String>,
}

impl RawCard {
    /// A card with a ref, a prompt, an answer and a grading mode, and nothing else.
    pub(crate) fn new(reference: String, prompt: String, answer: String, grading: &str) -> Self {
        Self {
            reference: Some(reference),
            prompt: Some(prompt),
            answer: Some(answer),
            grading: Some(grading.to_owned()),
            ..Self::default()
        }
    }

    /// Parses one line of a drill's output. Errors are plain reasons.
    pub(crate) fn parse(line: &str) -> Result<Self, String> {
        serde_json::from_str(line).map_err(|error| format!("it is not a card ({error})"))
    }
}

/// What a drill may leave out of its cards, and what fills it in (see
/// `docs/drills.md`, "The drill protocol").
#[derive(Debug, Clone, Copy)]
pub(crate) struct DrillDefaults<'a> {
    /// The drill that produced the card.
    pub(crate) drill: &'a str,
    /// Whether the drill's refs still need the `drill:` prefix. Built-in
    /// drills write whole refs.
    pub(crate) prefix: bool,
    /// The atom the drill ran for.
    pub(crate) atom: &'a Atom,
}

impl Card {
    /// The card a drill wrote, with the fields it may leave out filled in,
    /// checked. IDs in `atoms` must be atoms of `known`.
    pub(crate) fn from_drill(
        raw: RawCard,
        defaults: DrillDefaults<'_>,
        known: &Atoms,
    ) -> Result<Self, String> {
        if let Some(drill) = &raw.drill
            && drill != defaults.drill
        {
            return Err(format!("it says it comes from the drill `{drill}`"));
        }
        let reference = match raw.reference {
            Some(reference) if defaults.prefix => Some(format!("{}:{reference}", defaults.drill)),
            other @ (Some(_) | None) => other,
        };
        let filled = RawCard {
            atoms: raw
                .atoms
                .or_else(|| Some(vec![defaults.atom.id().to_string()])),
            drill: Some(defaults.drill.to_owned()),
            reference,
            explanation: raw
                .explanation
                .or_else(|| Some(defaults.atom.content().body().to_owned())),
            ..raw
        };
        Self::check(filled, known)
    }

    /// A whole card in the card format, as frontends send it back to
    /// `srl grade` and `srl review`, checked. Errors are plain reasons.
    pub(crate) fn from_value(value: serde_json::Value, known: &Atoms) -> Result<Self, String> {
        let raw = serde_json::from_value::<RawCard>(value)
            .map_err(|error| format!("it is not a card ({error})"))?;
        Self::check(raw, known)
    }

    /// Checks a card against the rules of `docs/cards.md`. Errors are plain
    /// reasons.
    fn check(raw: RawCard, known: &Atoms) -> Result<Self, String> {
        let atoms = required(raw.atoms, "atoms")?
            .iter()
            .map(|text| {
                let id = Id::parse(text).map_err(|error| error.to_string())?;
                if known.get(&id).is_some() {
                    Ok(id)
                } else {
                    Err(format!("`atoms` names {id}, which no atom has"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        if atoms.is_empty() {
            return Err("`atoms` is empty".to_owned());
        }
        let drill = required(raw.drill, "drill")?;
        let reference = required(raw.reference, "ref")?;
        if reference != drill && !reference.starts_with(&format!("{drill}:")) {
            return Err(format!(
                "its ref `{reference}` does not start with `{drill}:`"
            ));
        }
        if reference == format!("{drill}:") {
            return Err("its ref is empty".to_owned());
        }
        let prompt = required(raw.prompt, "prompt")?;
        if prompt.trim().is_empty() {
            return Err("its `prompt` is empty".to_owned());
        }
        let grading = grading(&required(raw.grading, "grading")?, raw.accept, raw.choices)?;
        Ok(Self {
            atoms,
            drill,
            reference,
            prompt,
            answer: required(raw.answer, "answer")?,
            grading,
            explanation: raw.explanation,
        })
    }

    pub fn atoms(&self) -> &[Id] {
        &self.atoms
    }

    pub fn drill(&self) -> &str {
        &self.drill
    }

    /// The card's identity within its atom: `drill` or `drill:…`.
    pub fn reference(&self) -> &str {
        &self.reference
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn answer(&self) -> &str {
        &self.answer
    }

    pub fn grading(&self) -> &Grading {
        &self.grading
    }

    pub fn explanation(&self) -> Option<&str> {
        self.explanation.as_deref()
    }
}

fn required<T>(value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("it has no `{field}`"))
}

/// The grading mode named `name`, with the fields it needs and none it doesn't.
fn grading(
    name: &str,
    accept: Option<Vec<Vec<String>>>,
    choices: Option<Vec<String>>,
) -> Result<Grading, String> {
    let needs_accept = |accept: Option<Vec<Vec<String>>>| {
        let accept = required(accept, "accept")?;
        if accept.is_empty() || accept.iter().any(Vec::is_empty) {
            Err("its `accept` needs at least one entry, each with at least one string".to_owned())
        } else {
            Ok(accept)
        }
    };
    let refuse = |present: bool, field: &str| {
        if present {
            Err(format!("a `{name}` card has no `{field}`"))
        } else {
            Ok(())
        }
    };
    match name {
        "self" | "cmd" => {
            refuse(accept.is_some(), "accept")?;
            refuse(choices.is_some(), "choices")?;
            Ok(if name == "self" {
                Grading::SelfRated
            } else {
                Grading::Cmd
            })
        }
        "exact" => {
            refuse(choices.is_some(), "choices")?;
            Ok(Grading::Exact {
                accept: needs_accept(accept)?,
            })
        }
        "choice" => {
            let accept = needs_accept(accept)?;
            if accept.len() != 1 {
                return Err("a `choice` card has exactly one `accept` entry".to_owned());
            }
            let choices = required(choices, "choices")?;
            if choices.len() < 2 {
                return Err("its `choices` needs at least two options".to_owned());
            }
            Ok(Grading::Choice { accept, choices })
        }
        other => Err(format!(
            "its `grading` is `{other}`, but must be self, exact, choice or cmd"
        )),
    }
}

/// The card format, field by field, as it is written out.
#[derive(Serialize)]
struct Written<'a> {
    atoms: &'a [Id],
    drill: &'a str,
    #[serde(rename = "ref")]
    reference: &'a str,
    prompt: &'a str,
    answer: &'a str,
    grading: &'a str,
    accept: Option<&'a [Vec<String>]>,
    choices: Option<&'a [String]>,
    explanation: Option<&'a str>,
}

impl Serialize for Card {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (accept, choices) = match &self.grading {
            Grading::SelfRated | Grading::Cmd => (None, None),
            Grading::Exact { accept } => (Some(accept.as_slice()), None),
            Grading::Choice { accept, choices } => {
                (Some(accept.as_slice()), Some(choices.as_slice()))
            }
        };
        Written {
            atoms: &self.atoms,
            drill: &self.drill,
            reference: &self.reference,
            prompt: &self.prompt,
            answer: &self.answer,
            grading: self.grading.name(),
            accept,
            choices,
            explanation: self.explanation.as_deref(),
        }
        .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn atoms() -> Atoms {
        Atoms {
            atoms: vec![
                Atom::parse(
                    "a.md".to_owned(),
                    "---\nid: a7f3k2\ntitle: x\n---\nThe body.\n".to_owned(),
                )
                .unwrap(),
            ],
            problems: Vec::new(),
        }
    }

    fn from_drill(line: &str) -> Result<Card, String> {
        let atoms = atoms();
        let defaults = DrillDefaults {
            drill: "conj",
            prefix: true,
            atom: &atoms.atoms[0],
        };
        Card::from_drill(RawCard::parse(line)?, defaults, &atoms)
    }

    #[test]
    fn a_drill_card_gets_its_defaults() {
        let card = from_drill(
            r#"{"ref": "te", "prompt": "て-form of 食べる?", "answer": "食べて", "grading": "exact", "accept": [["食べて"]]}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&card).unwrap(),
            json!({"atoms": ["a7f3k2"], "drill": "conj", "ref": "conj:te",
                   "prompt": "て-form of 食べる?", "answer": "食べて", "grading": "exact",
                   "accept": [["食べて"]], "choices": null, "explanation": "The body.\n"})
        );
    }

    #[test]
    fn an_empty_explanation_stays_empty() {
        let card = from_drill(
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "self", "explanation": ""}"#,
        )
        .unwrap();
        assert_eq!(card.explanation(), Some(""));
    }

    #[test]
    fn invalid_cards_are_refused() {
        for line in [
            r#"{"prompt": "q", "answer": "a", "grading": "self"}"#,
            r#"{"ref": "a", "answer": "a", "grading": "self"}"#,
            r#"{"ref": "a", "prompt": " ", "answer": "a", "grading": "self"}"#,
            r#"{"ref": "a", "prompt": "q", "grading": "self"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "fuzzy"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "exact"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "exact", "accept": []}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "exact", "accept": [[]]}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "self", "accept": [["a"]]}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "choice", "accept": [["a"]]}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "choice", "accept": [["a"]], "choices": ["a"]}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "choice", "accept": [["a"], ["b"]], "choices": ["a", "b"]}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "self", "atoms": ["zzzzzz"]}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "self", "atoms": []}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "self", "drill": "cloze"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "self", "colour": "red"}"#,
            r#"{"ref": "a", "prompt": 1, "answer": "a", "grading": "self"}"#,
            "not json",
        ] {
            assert!(from_drill(line).is_err(), "accepted {line}");
        }
    }

    #[test]
    fn a_choice_card_keeps_its_choices() {
        let card = from_drill(
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "choice", "accept": [["a"]], "choices": ["a", "b"]}"#,
        )
        .unwrap();
        assert_eq!(
            card.grading(),
            &Grading::Choice {
                accept: vec![vec!["a".to_owned()]],
                choices: vec!["a".to_owned(), "b".to_owned()],
            }
        );
    }
}
