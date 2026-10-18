//! Cards: the fixed format between drills and frontends (see `docs/cards.md`).

use serde::{Deserialize, Serialize};

use crate::{Atom, Atoms, Id};

/// One ready-to-show question about one or more atoms, checked against the
/// rules of `docs/cards.md`.
///
/// Serializes to the card format, with every field present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    atoms: Vec<Id>,
    drill: String,
    #[serde(rename = "ref")]
    reference: String,
    prompt: String,
    answer: String,
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
}

impl RawCard {
    /// A card with a ref, a prompt and an answer, and nothing else.
    pub(crate) fn new(reference: String, prompt: String, answer: String) -> Self {
        Self {
            reference: Some(reference),
            prompt: Some(prompt),
            answer: Some(answer),
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
            ..raw
        };
        Self::check(filled, known)
    }

    /// A whole card in the card format, as frontends send it back to
    /// `srl review` and sessions hold it, checked. Errors are plain reasons.
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
        Ok(Self {
            atoms,
            drill,
            reference,
            prompt,
            answer: required(raw.answer, "answer")?,
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
}

fn required<T>(value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("it has no `{field}`"))
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
        let card =
            from_drill(r#"{"ref": "te", "prompt": "て-form of 食べる?", "answer": "食べて"}"#)
                .unwrap();
        assert_eq!(
            serde_json::to_value(&card).unwrap(),
            json!({"atoms": ["a7f3k2"], "drill": "conj", "ref": "conj:te",
                   "prompt": "て-form of 食べる?", "answer": "食べて"})
        );
    }

    #[test]
    fn an_empty_answer_is_allowed() {
        let card = from_drill(r#"{"ref": "a", "prompt": "q", "answer": ""}"#).unwrap();
        assert_eq!(card.answer(), "");
    }

    #[test]
    fn invalid_cards_are_refused() {
        for line in [
            r#"{"prompt": "q", "answer": "a"}"#,
            r#"{"ref": "a", "answer": "a"}"#,
            r#"{"ref": "a", "prompt": " ", "answer": "a"}"#,
            r#"{"ref": "a", "prompt": "q"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "grading": "self"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "explanation": "e"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "atoms": ["zzzzzz"]}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "atoms": []}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "drill": "cloze"}"#,
            r#"{"ref": "a", "prompt": "q", "answer": "a", "colour": "red"}"#,
            r#"{"ref": "a", "prompt": 1, "answer": "a"}"#,
            "not json",
        ] {
            assert!(from_drill(line).is_err(), "accepted {line}");
        }
    }
}
