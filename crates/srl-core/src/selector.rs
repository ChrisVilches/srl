//! The selector language (see `docs/selectors.md`).

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::{Atom, Error, Id, Result, Status};

/// The keys of `key:value` terms.
const KEYS: &str = "tag, path, id, status, state";

/// A parsed selector: atoms match when they match every term.
#[derive(Debug, Clone)]
pub struct Selector {
    terms: Vec<Term>,
}

#[derive(Debug, Clone)]
struct Term {
    negated: bool,
    filter: Filter,
}

/// What one term checks. Lists are alternatives (`tag:a,b`).
#[derive(Debug, Clone)]
enum Filter {
    Tag(Vec<String>),
    Path(GlobSet),
    Id(Vec<Id>),
    Status(Vec<Status>),
    /// Lowercased.
    Text(String),
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

    pub fn matches(&self, atom: &Atom) -> bool {
        self.terms
            .iter()
            .all(|term| term.filter.matches(atom) != term.negated)
    }
}

impl Filter {
    fn matches(&self, atom: &Atom) -> bool {
        match self {
            Self::Tag(wanted) => atom.content().tags().iter().any(|tag| {
                wanted.iter().any(|want| {
                    tag == want
                        || tag
                            .strip_prefix(want.as_str())
                            .is_some_and(|rest| rest.starts_with('/'))
                })
            }),
            Self::Path(globs) => globs.is_match(atom.path()),
            Self::Id(ids) => ids.contains(atom.id()),
            Self::Status(statuses) => statuses.contains(&atom.content().status()),
            Self::Text(text) => atom.text().to_lowercase().contains(text.as_str()),
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
        "tag" => Ok(Filter::Tag(values.into_iter().map(str::to_owned).collect())),
        "path" => globs(&values).map(Filter::Path),
        "id" => values
            .into_iter()
            .map(|value| Id::parse(value).map_err(|error| error.to_string()))
            .collect::<std::result::Result<_, _>>()
            .map(Filter::Id),
        "status" => values
            .into_iter()
            .map(|value| {
                Status::parse(value).ok_or_else(|| {
                    format!("`{value}` is not a status; use active, draft or suspended")
                })
            })
            .collect::<std::result::Result<_, _>>()
            .map(Filter::Status),
        "state" => Err(
            "`state:` needs the review log, which is not implemented yet; leave it out for now"
                .to_owned(),
        ),
        _ => Err(format!(
            "`{key}:` is not a key; use one of {KEYS}, or put the term in double quotes to search for the text"
        )),
    }
}

/// Globs where `*` stops at `/` and `**` crosses it.
fn globs(patterns: &[&str]) -> std::result::Result<GlobSet, String> {
    patterns
        .iter()
        .map(|pattern| {
            GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|error| format!("`{pattern}` is not a valid glob ({error})"))
        })
        .try_fold(GlobSetBuilder::new(), |mut set, glob| {
            set.add(glob?);
            Ok::<_, String>(set)
        })?
        .build()
        .map_err(|error| format!("the globs are not valid ({error})"))
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
    use super::*;

    fn atom(id: &str, path: &str, front: &str, body: &str) -> Atom {
        Atom::parse(
            path.to_owned(),
            format!("---\nid: {id}\n{front}\n---\n{body}"),
        )
        .unwrap()
    }

    /// The IDs of the atoms in `atoms()` that `selector` picks.
    fn pick(selector: &str) -> Vec<String> {
        let selector = Selector::parse(selector).unwrap();
        atoms()
            .iter()
            .filter(|atom| selector.matches(atom))
            .map(|atom| atom.id().to_string())
            .collect()
    }

    fn atoms() -> Vec<Atom> {
        vec![
            atom(
                "aaaaa1",
                "japanese/grammar/wo.md",
                "cue: 〜をものともせず\ntags: [japanese/grammar, jlpt/n1]",
                "Undeterred by (adversity).\n",
            ),
            atom(
                "aaaaa2",
                "japanese/n5.md",
                "cue: です\ntags: [japanese/grammar, jlpt/n5]",
                "",
            ),
            atom(
                "aaaaa3",
                "kanji/food/maguro.md",
                "cue: 鮪\ntags: [kanji/food]\nstatus: draft",
                "Tuna.\n",
            ),
            atom(
                "aaaaa4",
                "rust/ownership/move.md",
                "cue: Move semantics\ntags: [rust/ownership, rust]\nstatus: suspended",
                "Ownership MOVES on assignment.\n",
            ),
        ]
    }

    #[test]
    fn empty_selector_matches_every_atom() {
        assert_eq!(pick(""), ["aaaaa1", "aaaaa2", "aaaaa3", "aaaaa4"]);
    }

    #[test]
    fn tag_matches_the_tag_and_tags_under_it() {
        assert_eq!(pick("tag:japanese"), ["aaaaa1", "aaaaa2"]);
        assert_eq!(pick("tag:japanese/gram"), Vec::<String>::new());
    }

    #[test]
    fn comma_is_or() {
        assert_eq!(pick("tag:jlpt/n1,kanji/food"), ["aaaaa1", "aaaaa3"]);
    }

    #[test]
    fn space_is_and() {
        assert_eq!(pick("tag:japanese tag:jlpt/n5"), ["aaaaa2"]);
    }

    #[test]
    fn negated_tag_excludes_children() {
        assert_eq!(pick("tag:japanese -tag:jlpt/n5"), ["aaaaa1"]);
        assert_eq!(pick("-tag:japanese"), ["aaaaa3", "aaaaa4"]);
    }

    #[test]
    fn path_star_stays_in_one_folder() {
        assert_eq!(pick("path:japanese/*"), ["aaaaa2"]);
        assert_eq!(pick("path:japanese/**"), ["aaaaa1", "aaaaa2"]);
        assert_eq!(pick("path:rust/ownership/*"), ["aaaaa4"]);
    }

    #[test]
    fn id_and_status_terms() {
        assert_eq!(pick("id:aaaaa2,aaaaa3"), ["aaaaa2", "aaaaa3"]);
        assert_eq!(pick("status:draft"), ["aaaaa3"]);
        assert_eq!(pick("status:active"), ["aaaaa1", "aaaaa2"]);
        assert_eq!(pick("-status:active"), ["aaaaa3", "aaaaa4"]);
    }

    #[test]
    fn text_matches_anywhere_in_the_file_ignoring_case() {
        assert_eq!(pick("ものともせず"), ["aaaaa1"]);
        assert_eq!(pick("moves"), ["aaaaa4"]);
        assert_eq!(pick("jlpt/n5"), ["aaaaa2"]);
        assert_eq!(pick("-tuna"), ["aaaaa1", "aaaaa2", "aaaaa4"]);
    }

    #[test]
    fn quotes_group_words_and_make_keys_text() {
        assert_eq!(pick("\"moves on\""), ["aaaaa4"]);
        assert_eq!(pick("assignment on"), ["aaaaa4"]);
        assert_eq!(pick("\"assignment on\""), Vec::<String>::new());
        assert_eq!(pick("\"tags: [kanji\""), ["aaaaa3"]);
        assert_eq!(pick("-\"by (adversity)\""), ["aaaaa2", "aaaaa3", "aaaaa4"]);
    }

    #[test]
    fn and_status_adds_a_term() {
        let selector = Selector::parse("tag:kanji,rust")
            .unwrap()
            .and_status(Status::Draft);
        let picked = atoms()
            .iter()
            .filter(|atom| selector.matches(atom))
            .map(|atom| atom.id().to_string())
            .collect::<Vec<_>>();
        assert_eq!(picked, ["aaaaa3"]);
    }

    #[test]
    fn unknown_selector_key_is_an_error() {
        let error = Selector::parse("tga:x").unwrap_err();
        assert!(error.to_string().contains("tag, path"), "{error}");
    }

    #[test]
    fn invalid_terms_are_errors() {
        for text in [
            "tag:",
            "tag:a,,b",
            "status:paused",
            "id:XYZ",
            "state:new",
            "path:[",
            "-",
            "\"\"",
            "\"open",
            "\"closed\"glued",
        ] {
            assert!(Selector::parse(text).is_err(), "accepted {text}");
        }
    }
}
