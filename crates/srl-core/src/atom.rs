//! Atoms: the file format, identity and status (see `docs/atoms.md`).

use std::collections::HashMap;
use std::fmt;
use std::fs;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::store::Store;
use crate::{Error, Result, yaml};

/// The number of characters in an ID.
const ID_LEN: usize = 6;

/// The characters an ID is made of.
const ID_CHARS: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// Names that are neither a front-matter field nor a facet, because the atom
/// JSON format uses them for something else (see `docs/atoms.md`).
const RESERVED: [&str; 2] = ["body", "path"];

/// An atom's stable identity: six characters `[a-z0-9]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Id(String);

impl Id {
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() == ID_LEN && text.bytes().all(|byte| ID_CHARS.contains(&byte)) {
            Ok(Self(text.to_owned()))
        } else {
            Err(Error::InvalidId {
                text: text.to_owned(),
            })
        }
    }

    /// The ID made of the last six base-36 digits of `number`.
    pub(crate) fn from_number(number: u64) -> Self {
        Self(
            std::iter::successors(Some(number), |rest| Some(rest / 36))
                .take(ID_LEN)
                .map(|rest| id_char(rest % 36))
                .collect(),
        )
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "callers pass a digit below 36"
)]
fn id_char(digit: u64) -> char {
    char::from(ID_CHARS[digit as usize])
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether an atom is studied (see `docs/atoms.md`, "Status").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
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

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Draft => "draft",
            Self::Suspended => "suspended",
        }
    }
}

/// Everything about an atom except its ID and where its file is: the
/// front-matter fields, the facets and the body.
///
/// Serializes to the atom JSON format without `id` and `path`, which is the
/// format of one line of `srl add --stdin`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Content {
    cue: String,
    answer: Option<String>,
    tags: Vec<String>,
    status: Status,
    source: Option<String>,
    #[serde(flatten)]
    facets: Map<String, Value>,
    body: String,
}

impl Content {
    /// Reads the fields from front-matter (or from a line of JSON), without
    /// `id`. Fields that are absent or `null` get their defaults. Every other
    /// key is a facet.
    ///
    /// Errors are plain reasons, to be put into a sentence by the caller.
    pub(crate) fn from_fields(
        fields: Map<String, Value>,
        body: String,
    ) -> std::result::Result<Self, String> {
        let (known, facets): (Map<_, _>, Map<_, _>) = fields.into_iter().partition(|(key, _)| {
            ["cue", "answer", "tags", "status", "source"].contains(&key.as_str())
        });
        if let Some(key) = facets.keys().find(|key| RESERVED.contains(&key.as_str())) {
            return Err(format!("`{key}` is a reserved name and cannot be a facet"));
        }
        if facets.contains_key("id") {
            return Err("`id` cannot be given here".to_owned());
        }
        let cue = optional_string(&known, "cue")?
            .filter(|cue| !cue.trim().is_empty())
            .ok_or_else(|| "it has no `cue`".to_owned())?;
        let status = optional_string(&known, "status")?
            .map(|text| {
                Status::parse(&text).ok_or_else(|| {
                    format!("`status` is `{text}`, but must be active, draft or suspended")
                })
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            cue,
            answer: optional_string(&known, "answer")?,
            tags: tags(&known)?,
            status,
            source: optional_string(&known, "source")?,
            facets,
            body,
        })
    }

    pub fn cue(&self) -> &str {
        &self.cue
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub(crate) fn with_status(self, status: Status) -> Self {
        Self { status, ..self }
    }

    /// The front-matter keys and values of this content under `id`, in the
    /// order of `docs/atoms.md`, leaving out absent and default values.
    pub(crate) fn fields(&self, id: &Id) -> Map<String, Value> {
        let fixed = [
            ("id", Some(Value::from(id.as_str()))),
            ("cue", Some(Value::from(self.cue.as_str()))),
            ("answer", self.answer.as_deref().map(Value::from)),
            (
                "tags",
                Some(Value::from(self.tags.clone())).filter(|_| !self.tags.is_empty()),
            ),
            (
                "status",
                Some(Value::from(self.status.as_str())).filter(|_| self.status != Status::Active),
            ),
            ("source", self.source.as_deref().map(Value::from)),
        ];
        fixed
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value)))
            .chain(self.facets.clone())
            .collect()
    }
}

fn optional_string(
    fields: &Map<String, Value>,
    key: &str,
) -> std::result::Result<Option<String>, String> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(other) => Err(format!(
            "`{key}` must be text, but is {other}; put it in quotes"
        )),
    }
}

fn tags(fields: &Map<String, Value>) -> std::result::Result<Vec<String>, String> {
    let not_a_list = || "`tags` must be a list of text, such as [a, b/c]".to_owned();
    match fields.get("tags") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned).ok_or_else(not_a_list))
            .collect(),
        Some(_) => Err(not_a_list()),
    }
}

/// One atom read from its file.
///
/// Serializes to the atom JSON format of `docs/atoms.md`.
#[derive(Debug, Clone, Serialize)]
pub struct Atom {
    id: Id,
    path: String,
    #[serde(flatten)]
    content: Content,
    #[serde(skip)]
    text: String,
}

impl Atom {
    /// Parses the file at `path` (relative to `atoms/`, with `/` between
    /// folders) whose text is `text`.
    pub fn parse(path: String, text: String) -> Result<Self> {
        let invalid = |reason: String| Error::InvalidAtom {
            path: path.clone(),
            reason,
        };
        let (front, body) = split_front_matter(&text).ok_or_else(|| {
            invalid("it does not start with front-matter between `---` lines".to_owned())
        })?;
        let mut fields = yaml::parse_mapping(front).map_err(invalid)?;
        let id = match fields.remove("id") {
            Some(Value::String(id)) => {
                Id::parse(&id).map_err(|error| invalid(error.to_string()))?
            }
            Some(other) => return Err(invalid(format!("its `id` is {other}, which is not an ID"))),
            None => return Err(Error::MissingId { path }),
        };
        let content = Content::from_fields(fields, body.to_owned()).map_err(invalid)?;
        Ok(Self {
            id,
            path,
            content,
            text,
        })
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The file's path relative to `atoms/`, with `/` between folders.
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The whole file, as it was read.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Splits an atom file into its front-matter and body. The front-matter sits
/// between a first line `---` and the next line `---`; a UTF-8 BOM and CRLF
/// line endings are allowed.
pub(crate) fn split_front_matter(text: &str) -> Option<(&str, &str)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let (start, line) = rest
        .split_inclusive('\n')
        .scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .find(|(_, line)| line.trim_end_matches(['\n', '\r']) == "---")?;
    Some((&rest[..start], &rest[start + line.len()..]))
}

/// Every atom in a store, and the files that could not be read as atoms.
#[derive(Debug)]
pub struct Atoms {
    /// The atoms, ordered by path.
    pub atoms: Vec<Atom>,
    /// One error per file that was skipped. Report them, then carry on with
    /// the atoms that were read.
    pub problems: Vec<Error>,
}

impl Atoms {
    /// Reads every atom file of the store. A file that is not a valid atom,
    /// or whose ID an earlier file (by path) already has, is skipped and
    /// reported in `problems`.
    pub fn load(store: &Store) -> Result<Self> {
        let dir = store.atoms_dir();
        let (atoms, problems) = store
            .atom_paths()?
            .into_iter()
            .map(|path| {
                let file = dir.join(&path);
                let text = fs::read_to_string(&file).map_err(Error::io(file))?;
                Atom::parse(path, text)
            })
            .fold(
                (Vec::new(), Vec::new()),
                |(mut atoms, mut problems), parsed| {
                    match parsed {
                        Ok(atom) => atoms.push(atom),
                        Err(error) => problems.push(error),
                    }
                    (atoms, problems)
                },
            );
        let (atoms, duplicates) = unique(atoms);
        Ok(Self {
            atoms,
            problems: problems.into_iter().chain(duplicates).collect(),
        })
    }

    /// The atom with this ID.
    pub fn get(&self, id: &Id) -> Option<&Atom> {
        self.atoms.iter().find(|atom| atom.id() == id)
    }
}

/// Keeps the first atom of each ID, and reports the others.
fn unique(atoms: Vec<Atom>) -> (Vec<Atom>, Vec<Error>) {
    let mut first = HashMap::new();
    let mut kept = Vec::new();
    let mut duplicates = Vec::new();
    for atom in atoms {
        if let Some(path) = first.get(atom.id()) {
            duplicates.push(Error::DuplicateId {
                id: atom.id.clone(),
                path: atom.path,
                first: String::clone(path),
            });
        } else {
            first.insert(atom.id.clone(), atom.path.clone());
            kept.push(atom);
        }
    }
    (kept, duplicates)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn atom(text: &str) -> Result<Atom> {
        Atom::parse("a.md".to_owned(), text.to_owned())
    }

    #[test]
    fn the_smallest_atom_parses() {
        let atom = atom(
            "---\nid: a7f3k2\ncue: 〜をものともせず\ntags: [japanese/grammar, jlpt/n1]\n---\nUndeterred by (adversity).\n",
        )
        .unwrap();
        assert_eq!(atom.id().as_str(), "a7f3k2");
        assert_eq!(atom.content().cue(), "〜をものともせず");
        assert_eq!(atom.content().tags(), ["japanese/grammar", "jlpt/n1"]);
        assert_eq!(atom.content().status(), Status::Active);
        assert_eq!(atom.content().body(), "Undeterred by (adversity).\n");
    }

    #[test]
    fn bom_and_crlf_front_matter_parse() {
        let atom = atom("\u{feff}---\r\nid: a7f3k2\r\ncue: x\r\n---\r\nbody\r\n").unwrap();
        assert_eq!(atom.content().cue(), "x");
        assert_eq!(atom.content().body(), "body\r\n");
    }

    #[test]
    fn closing_fence_without_newline_ends_the_front_matter() {
        let atom = atom("---\nid: a7f3k2\ncue: x\n---").unwrap();
        assert_eq!(atom.content().body(), "");
    }

    #[test]
    fn cue_no_stays_a_string() {
        assert_eq!(
            atom("---\nid: a7f3k2\ncue: no\n---\n")
                .unwrap()
                .content()
                .cue(),
            "no"
        );
    }

    #[test]
    fn other_keys_are_facets_in_file_order() {
        let atom = atom("---\nid: a7f3k2\ncue: x\nreverse: true\ncloze: [a]\n---\n").unwrap();
        let json = serde_json::to_value(&atom).unwrap();
        assert_eq!(
            json,
            json!({"id": "a7f3k2", "path": "a.md", "cue": "x", "answer": null, "tags": [],
                   "status": "active", "source": null, "reverse": true, "cloze": ["a"], "body": ""})
        );
    }

    #[test]
    fn a_file_without_front_matter_is_invalid() {
        assert!(matches!(
            atom("just text\n"),
            Err(Error::InvalidAtom { .. })
        ));
        assert!(matches!(
            atom("---\nid: a7f3k2\n"),
            Err(Error::InvalidAtom { .. })
        ));
    }

    #[test]
    fn missing_id_is_reported_as_such() {
        assert!(matches!(
            atom("---\ncue: x\n---\n"),
            Err(Error::MissingId { .. })
        ));
    }

    #[test]
    fn invalid_fields_are_reported() {
        for text in [
            "---\nid: A7F3K2\ncue: x\n---\n",
            "---\nid: a7f3k2\n---\n",
            "---\nid: a7f3k2\ncue: 1984\n---\n",
            "---\nid: a7f3k2\ncue: x\nstatus: paused\n---\n",
            "---\nid: a7f3k2\ncue: x\ntags: a\n---\n",
            "---\nid: a7f3k2\ncue: x\nbody: y\n---\n",
            "---\nid: a7f3k2\ncue: x\ncue: y\n---\n",
        ] {
            assert!(
                matches!(atom(text), Err(Error::InvalidAtom { .. })),
                "accepted:\n{text}"
            );
        }
    }

    #[test]
    fn ids_are_six_lowercase_letters_or_digits() {
        assert!(Id::parse("a7f3k2").is_ok());
        for bad in ["a7f3k", "a7f3k2x", "A7F3K2", "a7f-k2", ""] {
            assert!(Id::parse(bad).is_err(), "accepted {bad}");
        }
        assert_eq!(Id::from_number(0).as_str(), "aaaaaa");
        assert_eq!(Id::from_number(35).as_str(), "9aaaaa");
        assert!(Id::parse(Id::from_number(u64::MAX).as_str()).is_ok());
    }

    #[test]
    fn fields_leave_out_defaults() {
        let content = Content::from_fields(
            json!({"cue": "鮪", "answer": "tuna", "reverse": true})
                .as_object()
                .unwrap()
                .clone(),
            String::new(),
        )
        .unwrap();
        let id = Id::parse("k3m9q1").unwrap();
        assert_eq!(
            Value::Object(content.fields(&id)),
            json!({"id": "k3m9q1", "cue": "鮪", "answer": "tuna", "reverse": true})
        );
    }
}
