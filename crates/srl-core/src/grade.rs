//! Grading an answer to a card (see `docs/grading.md`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::{Atom, Atoms, Card, Error, Grading, Result, Search, process};

/// The rating `srl grade` suggests when every blank is right: Good.
const ALL_RIGHT: u8 = 3;

/// The rating `srl grade` suggests when any blank is wrong: Again.
const ANY_WRONG: u8 = 1;

/// What `srl grade` reads: a card and the answer to it.
#[derive(Debug, Clone)]
pub struct GradeRequest {
    card: Card,
    response: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    card: Value,
    response: Vec<String>,
}

impl GradeRequest {
    /// Reads `{"card": …, "response": […]}`. The card is checked, and the
    /// response must have one string per blank (`exact`) or exactly one
    /// (`choice` and `cmd`).
    pub fn parse(text: &str, atoms: &Atoms) -> Result<Self> {
        let invalid = |reason: String| Error::InvalidGradeRequest { reason };
        let raw = serde_json::from_str::<RawRequest>(text)
            .map_err(|error| invalid(format!("it is not a grading request ({error})")))?;
        let card = Card::from_value(raw.card, atoms)
            .map_err(|reason| invalid(format!("the card: {reason}")))?;
        let wanted = match card.grading() {
            Grading::SelfRated => None,
            Grading::Exact { accept } => Some(accept.len()),
            Grading::Choice { .. } | Grading::Cmd => Some(1),
        };
        if let Some(wanted) = wanted
            && raw.response.len() != wanted
        {
            return Err(invalid(format!(
                "a `{}` card needs {wanted} response strings, but there are {}",
                card.grading().name(),
                raw.response.len()
            )));
        }
        Ok(Self {
            card,
            response: raw.response,
        })
    }
}

/// What `srl grade` prints (see `docs/grading.md`, "`srl grade`").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Graded {
    pub correct: Option<Vec<bool>>,
    pub suggested_rating: Option<u8>,
    pub feedback: Option<String>,
    /// `Some("self")` when a `cmd` card could not be graded, and the
    /// frontend should let you rate yourself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<&'static str>,
}

impl Graded {
    fn by_you() -> Self {
        Self {
            correct: None,
            suggested_rating: None,
            feedback: None,
            fallback: None,
        }
    }

    fn fallback() -> Self {
        Self {
            fallback: Some("self"),
            ..Self::by_you()
        }
    }

    fn compared(correct: Vec<bool>) -> Self {
        let rating = if correct.iter().all(|right| *right) {
            ALL_RIGHT
        } else {
            ANY_WRONG
        };
        Self {
            correct: Some(correct),
            suggested_rating: Some(rating),
            feedback: None,
            fallback: None,
        }
    }
}

/// Grades the answer in `request`. `cmd` is `[grading] cmd` from `srl.toml`,
/// and `search` where to find its program.
///
/// A `cmd` card with no grader configured, or whose grader fails, gets the
/// fallback result; a failure is also returned, to be reported.
pub fn grade(
    request: &GradeRequest,
    atoms: &Atoms,
    cmd: &str,
    search: &Search,
) -> (Graded, Option<Error>) {
    match request.card.grading() {
        Grading::SelfRated => (Graded::by_you(), None),
        Grading::Exact { accept } | Grading::Choice { accept, .. } => {
            let correct = request
                .response
                .iter()
                .zip(accept)
                .map(|(response, accepted)| is_accepted(response, accepted))
                .collect();
            (Graded::compared(correct), None)
        }
        Grading::Cmd if cmd.trim().is_empty() => (Graded::fallback(), None),
        Grading::Cmd => match external(request, atoms, cmd, search) {
            Ok(graded) => (graded, None),
            Err(reason) => (
                Graded::fallback(),
                Some(Error::GraderFailed {
                    command: cmd.to_owned(),
                    reason,
                }),
            ),
        },
    }
}

/// Whether `response` equals any of `accepted`, after normalizing both (see
/// `docs/grading.md`, "Normalization").
fn is_accepted(response: &str, accepted: &[String]) -> bool {
    let response = normalize(response);
    accepted.iter().any(|text| normalize(text) == response)
}

/// NFKC, whitespace trimmed and collapsed, lowercase.
fn normalize(text: &str) -> String {
    text.nfkc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// What the external grader answers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Verdict {
    correct: Vec<bool>,
    rating: u8,
    feedback: Option<String>,
}

/// Runs the external grader (see `docs/grading.md`, "The external grader").
/// `cmd` is a program, found like any on `PATH`, and its arguments,
/// separated by spaces. Errors are plain reasons.
fn external(
    request: &GradeRequest,
    atoms: &Atoms,
    cmd: &str,
    search: &Search,
) -> std::result::Result<Graded, String> {
    let mut words = cmd.split_whitespace();
    let name = words.next().ok_or_else(|| "it is empty".to_owned())?;
    let args = words.collect::<Vec<_>>();
    let paths = std::env::join_paths(&search.path)
        .map_err(|error| format!("PATH cannot be used ({error})"))?;
    let program = which::which_in(name, Some(paths), &search.cwd)
        .map_err(|error| format!("`{name}` is not on PATH ({error})"))?;
    let atom = request
        .card
        .atoms()
        .first()
        .and_then(|id| atoms.get(id))
        .ok_or_else(|| "the card's atom is not in the store".to_owned())?;
    let input = serde_json::json!({
        "card": request.card,
        "response": request.response,
        "atom": atom_summary(atom),
    });
    let output = process::run(&program, &args, input.to_string().as_bytes(), None)?;
    let verdict = serde_json::from_str::<Verdict>(output.trim())
        .map_err(|error| format!("its output is not a verdict ({error})"))?;
    if !(1..=4).contains(&verdict.rating) {
        return Err(format!("its rating is {}, not 1 to 4", verdict.rating));
    }
    if verdict.correct.len() != request.response.len() {
        return Err(format!(
            "it judged {} answers, but there are {}",
            verdict.correct.len(),
            request.response.len()
        ));
    }
    Ok(Graded {
        correct: Some(verdict.correct),
        suggested_rating: Some(verdict.rating),
        feedback: verdict.feedback,
        fallback: None,
    })
}

/// The atom as the external grader gets it: `id`, `title`, `answer`, `body`.
fn atom_summary(atom: &Atom) -> Value {
    let content = atom.content();
    serde_json::json!({
        "id": atom.id(),
        "title": content.title(),
        "answer": content.answer(),
        "body": content.body(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atoms() -> Atoms {
        Atoms {
            atoms: vec![
                Atom::parse(
                    "a.md".to_owned(),
                    "---\nid: a7f3k2\ntitle: x\n---\n".to_owned(),
                )
                .unwrap(),
            ],
            problems: Vec::new(),
        }
    }

    fn request(card: &str, response: &str) -> Result<GradeRequest> {
        GradeRequest::parse(
            &format!(r#"{{"card": {card}, "response": {response}}}"#),
            &atoms(),
        )
    }

    fn run(card: &str, response: &str, cmd: &str) -> (Graded, Option<Error>) {
        grade(
            &request(card, response).unwrap(),
            &atoms(),
            cmd,
            &Search::default(),
        )
    }

    const EXACT: &str = r#"{"atoms": ["a7f3k2"], "drill": "cloze", "ref": "cloze:1:1", "prompt": "==[1]==を==[2]==", "answer": "x", "grading": "exact", "accept": [["周囲の反対"], ["ものともせず"]]}"#;

    #[test]
    fn normalization_ignores_width_case_and_spacing() {
        assert_eq!(normalize("  ＡＢＣ   def\t"), "abc def");
        assert!(is_accepted("ﾏｸﾞﾛ", &["マグロ".to_owned()]));
        assert!(!is_accepted("まぐろ", &["マグロ".to_owned()]));
    }

    #[test]
    fn exact_cards_are_graded_per_blank() {
        let (graded, problem) = run(EXACT, r#"["周囲の反対", " ものともせず "]"#, "");
        assert!(problem.is_none());
        assert_eq!(
            serde_json::to_value(&graded).unwrap(),
            serde_json::json!({"correct": [true, true], "suggested_rating": 3, "feedback": null})
        );
        let (graded, _) = run(EXACT, r#"["周囲の反対", "x"]"#, "");
        assert_eq!(graded.correct, Some(vec![true, false]));
        assert_eq!(graded.suggested_rating, Some(1));
    }

    #[test]
    fn the_response_must_fit_the_card() {
        assert!(request(EXACT, r#"["only one"]"#).is_err());
        assert!(request(EXACT, r#"["a", "b", "c"]"#).is_err());
        assert!(request(r#"{"not": "a card"}"#, "[]").is_err());
    }

    #[test]
    fn self_cards_are_not_graded() {
        let card = r#"{"atoms": ["a7f3k2"], "drill": "reveal", "ref": "reveal", "prompt": "x", "answer": "y", "grading": "self"}"#;
        let (graded, problem) = run(card, "[]", "");
        assert!(problem.is_none());
        assert_eq!(graded, Graded::by_you());
    }

    #[test]
    fn cmd_cards_fall_back_without_a_grader_and_report_a_failing_one() {
        let card = r#"{"atoms": ["a7f3k2"], "drill": "llm", "ref": "llm:1", "prompt": "x", "answer": "y", "grading": "cmd"}"#;
        let (graded, problem) = run(card, r#"["my answer"]"#, "");
        assert_eq!(graded, Graded::fallback());
        assert!(problem.is_none());
        let (graded, problem) = run(card, r#"["my answer"]"#, "srl-no-such-grader-for-tests");
        assert_eq!(
            serde_json::to_value(&graded).unwrap(),
            serde_json::json!({"correct": null, "suggested_rating": null, "feedback": null, "fallback": "self"})
        );
        assert!(matches!(problem, Some(Error::GraderFailed { .. })));
    }
}
