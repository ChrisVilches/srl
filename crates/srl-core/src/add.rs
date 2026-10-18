//! Creating atoms from flags or JSON lines (see `docs/atoms.md`, "Creating
//! atoms").

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde_json::Value;

use crate::atom::Content;
use crate::{Atom, Atoms, Error, Id, Result, Status, Store, yaml};

/// The most characters of a cue that go into a file name.
const SLUG_LEN: usize = 60;

/// The fields of `srl add --cue C [--answer A] [--tag T]… [--source S]`.
#[derive(Debug, Clone, Default)]
pub struct Flags {
    pub cue: String,
    pub answer: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<String>,
}

/// An atom that was (or, with `dry_run`, would be) added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub id: Id,
    /// Relative to `atoms/`, with `/` between folders.
    pub path: String,
}

impl Content {
    /// The content of `srl add --cue …`.
    pub fn from_flags(flags: Flags) -> Result<Self> {
        let fields = [
            ("cue", Some(Value::from(flags.cue))),
            ("answer", flags.answer.map(Value::from)),
            ("tags", Some(Value::from(flags.tags))),
            ("source", flags.source.map(Value::from)),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value)))
        .collect();
        Self::from_fields(fields, String::new()).map_err(|reason| Error::InvalidNewAtom { reason })
    }

    /// The contents given to `srl add --stdin`: one JSON object per line.
    /// Blank lines are ignored. Fails on the first invalid line.
    pub fn from_json_lines(text: &str) -> Result<Vec<Self>> {
        text.lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(index, line)| {
                Self::from_json_line(line).map_err(|reason| Error::InvalidLine {
                    line: index + 1,
                    reason,
                })
            })
            .collect()
    }

    /// The content of one line of `srl add --stdin`: a JSON object with the
    /// front-matter fields except `id`, and an optional `body`. A body that
    /// does not end with a newline gets one, so that the file does.
    fn from_json_line(text: &str) -> std::result::Result<Self, String> {
        let mut fields = match serde_json::from_str(text) {
            Ok(Value::Object(fields)) => fields,
            Ok(_) => return Err("it is not a JSON object".to_owned()),
            Err(error) => return Err(format!("it is not valid JSON ({error})")),
        };
        let body = match fields.remove("body") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(body)) if body.is_empty() || body.ends_with('\n') => body,
            Some(Value::String(body)) => format!("{body}\n"),
            Some(_) => return Err("`body` must be text".to_owned()),
        };
        Self::from_fields(fields, body)
    }
}

/// Adds atoms to the store, as drafts if `draft` is set, and returns their
/// IDs and paths. The new IDs come from `seed`, which the caller makes
/// random; the same seed gives the same IDs.
///
/// Every file is prepared and checked before the first one is written, and no
/// existing file is ever overwritten. With `dry_run`, nothing is written.
pub fn add(
    store: &Store,
    existing: &Atoms,
    contents: Vec<Content>,
    draft: bool,
    seed: u64,
    dry_run: bool,
) -> Result<Vec<Added>> {
    let mut used_ids = existing
        .atoms
        .iter()
        .map(|atom| atom.id().clone())
        .collect::<HashSet<_>>();
    let mut used_paths = store.atom_paths()?.into_iter().collect::<HashSet<_>>();
    let mut ids = random_ids(seed);
    let prepared = contents
        .into_iter()
        .map(|content| {
            let content = if draft {
                content.with_status(Status::Draft)
            } else {
                content
            };
            let id = ids
                .by_ref()
                .find(|id| !used_ids.contains(id))
                .expect("the sequence of IDs should be endless");
            used_ids.insert(id.clone());
            let path = place(&content, &id, |path| {
                used_paths.contains(path) || store.atom_file(path).exists()
            });
            used_paths.insert(path.clone());
            let text = render(&content, &id)?;
            Ok((Added { id, path }, text))
        })
        .collect::<Result<Vec<_>>>()?;
    if !dry_run {
        for (added, text) in &prepared {
            write_new(&store.atom_file(&added.path), text)?;
        }
    }
    Ok(prepared.into_iter().map(|(added, _)| added).collect())
}

/// An endless sequence of IDs made from `seed`, with the splitmix64
/// generator.
fn random_ids(seed: u64) -> impl Iterator<Item = Id> {
    std::iter::successors(Some(seed), |state| {
        Some(state.wrapping_add(0x9E37_79B9_7F4A_7C15))
    })
    .skip(1)
    .map(|state| {
        let mixed = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Id::from_number(mixed ^ (mixed >> 31))
    })
}

/// The path of a new atom, relative to `atoms/`: a folder made from the first
/// tag, and a file name made from the cue. If the cue gives no name, the name
/// is the ID; if the name is `taken`, the ID is added to it.
///
// TODO(platform): Windows limits paths to 260 characters unless long paths
// are enabled. Names are at most `SLUG_LEN` characters, but a store deep in
// the file system with a long first tag can still exceed it, and then
// writing fails with an error. Decide whether to cap the whole path (see
// docs/rust.md, "Platform independence").
fn place(content: &Content, id: &Id, taken: impl Fn(&str) -> bool) -> String {
    let folder = content
        .tags()
        .first()
        .map(|tag| {
            tag.split('/')
                .map(slug)
                .filter(|part| !part.is_empty())
                .flat_map(|part| [part, "/".to_owned()])
                .collect::<String>()
        })
        .unwrap_or_default();
    let name = slug(content.cue());
    let plain = if name.is_empty() {
        format!("{folder}{id}.md")
    } else {
        format!("{folder}{name}.md")
    };
    if taken(&plain) {
        format!("{folder}{name}-{id}.md")
    } else {
        plain
    }
}

/// A file name from text: lowercase, letters and digits of any script kept,
/// every other run of characters turned into one `-`, at most `SLUG_LEN`
/// characters. A name that Windows reserves for a device gets a `_`, so that
/// the same name works on every platform.
fn slug(text: &str) -> String {
    let words = text
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .join("-");
    let name = words
        .chars()
        .take(SLUG_LEN)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned();
    if is_device_name(&name) {
        format!("{name}_")
    } else {
        name
    }
}

/// Whether Windows reserves `name` for a device, which makes it unusable as
/// a file or folder name there, with any extension.
fn is_device_name(name: &str) -> bool {
    const DEVICES: [&str; 4] = ["con", "prn", "aux", "nul"];
    const NUMBERED: [&str; 2] = ["com", "lpt"];
    const DIGITS: &str = "0123456789¹²³";
    DEVICES.contains(&name)
        || NUMBERED.iter().any(|prefix| {
            name.strip_prefix(prefix).is_some_and(|rest| {
                let mut chars = rest.chars();
                chars.next().is_some_and(|digit| DIGITS.contains(digit)) && chars.next().is_none()
            })
        })
}

/// The file of a new atom. Checks that it reads back as the same atom.
fn render(content: &Content, id: &Id) -> Result<String> {
    let front = yaml::render_mapping(&content.fields(id), &["tags"])
        .map_err(|reason| Error::InvalidNewAtom { reason })?;
    let text = format!("---\n{front}---\n{}", content.body());
    let reread = Atom::parse(String::new(), text.clone())?;
    if reread.id() == id && reread.content() == content {
        Ok(text)
    } else {
        Err(Error::InvalidNewAtom {
            reason: format!("its file would not read back as the same atom:\n{text}"),
        })
    }
}

/// Writes a file that must not exist yet.
fn write_new(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(Error::io(parent))?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(Error::io(path))
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};
    use tempfile::TempDir;

    use super::*;
    use crate::Lookup;

    fn object(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            other @ (Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_)) => panic!("not an object: {other}"),
        }
    }

    fn store() -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let lookup = Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        };
        let store = Store::open(&lookup).unwrap();
        (tmp, store)
    }

    fn content(value: Value) -> Content {
        Content::from_fields(object(value), String::new()).unwrap()
    }

    fn add_all(store: &Store, contents: Vec<Content>) -> Vec<Added> {
        let atoms = Atoms::load(store).unwrap();
        add(store, &atoms, contents, false, 0, false).unwrap()
    }

    #[test]
    fn slug_keeps_letters_of_any_script() {
        assert_eq!(slug("〜をものともせず"), "をものともせず");
        assert_eq!(slug("Move semantics in Rust"), "move-semantics-in-rust");
        assert_eq!(slug("  C++ / Rust!  "), "c-rust");
        assert_eq!(slug("〜"), "");
        assert_eq!(slug(&"a".repeat(100)).chars().count(), SLUG_LEN);
    }

    #[test]
    fn windows_device_names_get_an_underscore() {
        for name in ["CON", "nul", "Com1", "lpt9", "com¹", "AUX!"] {
            assert!(slug(name).ends_with('_'), "{name} became {}", slug(name));
        }
        for name in ["console", "com10", "lpt", "nuls"] {
            assert!(!slug(name).ends_with('_'), "{name} became {}", slug(name));
        }
    }

    #[test]
    fn files_go_in_the_folder_of_the_first_tag() {
        let content =
            content(json!({"cue": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"]}));
        let id = Id::parse("a7f3k2").unwrap();
        assert_eq!(
            place(&content, &id, |_| false),
            "japanese/grammar/をものともせず.md"
        );
    }

    #[test]
    fn tags_cannot_leave_the_atoms_folder() {
        let content = content(json!({"cue": "x", "tags": ["../../etc", "/abs"]}));
        let id = Id::parse("a7f3k2").unwrap();
        assert_eq!(place(&content, &id, |_| false), "etc/x.md");
    }

    #[test]
    fn a_cue_without_letters_is_named_after_the_id() {
        let id = Id::parse("a7f3k2").unwrap();
        assert_eq!(
            place(&content(json!({"cue": "〜"})), &id, |_| false),
            "a7f3k2.md"
        );
    }

    #[test]
    fn taken_name_gets_id_suffix() {
        let (_tmp, store) = store();
        let added = add_all(
            &store,
            vec![content(json!({"cue": "鮪"})), content(json!({"cue": "鮪"}))],
        );
        assert_eq!(added[0].path, "鮪.md");
        assert_eq!(added[1].path, format!("鮪-{}.md", added[1].id));
    }

    #[test]
    fn added_atoms_read_back_with_their_content() {
        let (_tmp, store) = store();
        let line = r#"{"cue": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"], "source": "Tobira ch.3",
                       "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred by (adversity)."}"#;
        let added = add_all(&store, vec![Content::from_json_line(line).unwrap()]);
        let atoms = Atoms::load(&store).unwrap();
        assert!(atoms.problems.is_empty());
        let atom = atoms.get(&added[0].id).unwrap();
        assert_eq!(atom.path(), "japanese/grammar/をものともせず.md");
        assert_eq!(
            serde_json::to_value(atom).unwrap(),
            json!({"id": added[0].id, "path": "japanese/grammar/をものともせず.md",
                   "cue": "〜をものともせず", "answer": null, "tags": ["japanese/grammar", "jlpt/n1"],
                   "status": "active", "source": "Tobira ch.3",
                   "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"],
                   "body": "Undeterred by (adversity).\n"})
        );
    }

    #[test]
    fn drafts_are_written_with_their_status() {
        let (_tmp, store) = store();
        let atoms = Atoms::load(&store).unwrap();
        let added = add(
            &store,
            &atoms,
            vec![content(json!({"cue": "x"}))],
            true,
            0,
            false,
        )
        .unwrap();
        let atoms = Atoms::load(&store).unwrap();
        assert_eq!(
            atoms.get(&added[0].id).unwrap().content().status(),
            Status::Draft
        );
    }

    #[test]
    fn ids_already_in_use_are_skipped() {
        let (_tmp, store) = store();
        let first = add_all(&store, vec![content(json!({"cue": "a"}))]);
        let second = add_all(&store, vec![content(json!({"cue": "b"}))]);
        assert_ne!(first[0].id, second[0].id);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let (_tmp, store) = store();
        let atoms = Atoms::load(&store).unwrap();
        let added = add(
            &store,
            &atoms,
            vec![content(json!({"cue": "x"}))],
            false,
            0,
            true,
        )
        .unwrap();
        assert_eq!(added.len(), 1);
        assert!(store.atom_paths().unwrap().is_empty());
    }

    #[test]
    fn json_lines_with_an_id_or_bad_fields_are_refused() {
        for line in [
            r#"{"id": "a7f3k2", "cue": "x"}"#,
            r#"{"cue": ""}"#,
            r#"{"answer": "x"}"#,
            r#"{"cue": "x", "body": 3}"#,
            r#"{"cue": "x", "path": "y"}"#,
            r#"["cue"]"#,
            "not json",
        ] {
            assert!(Content::from_json_line(line).is_err(), "accepted {line}");
        }
    }

    #[test]
    fn one_bad_line_rejects_all_and_names_the_line() {
        let text = "{\"cue\": \"a\"}\n\n{\"cue\": 1}\n";
        assert!(matches!(
            Content::from_json_lines(text),
            Err(Error::InvalidLine { line: 3, .. })
        ));
        assert_eq!(
            Content::from_json_lines("{\"cue\": \"a\"}\n\n")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn body_without_final_newline_still_reads_back() {
        let content = Content::from_json_line(r#"{"cue": "x", "body": "no newline"}"#).unwrap();
        let id = Id::parse("a7f3k2").unwrap();
        assert!(render(&content, &id).is_ok());
    }
}
