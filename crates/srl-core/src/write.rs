//! The only place that changes notes: `srl id`, `srl accept`, `srl add`
//! and `srl edit ID --stdin` (see `docs/notes.md`, "How srl writes to
//! notes", and `docs/rust.md`, "Note editing").
//!
//! Every change edits lines, never data. Which lines, and what they become,
//! is the business of the item syntax ([`syntax::with_id`],
//! [`syntax::without_draft`], [`syntax::replace`], [`syntax::format`]);
//! this module makes the change safe: the new text is parsed again and
//! compared with the old before it is written, the file must not have
//! changed since it was read, and it is replaced atomically.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::Path;

use serde_json::{Map, Value};

use crate::note::syntax::{self, Edit, RawItem};
use crate::note::{Entries, Field, is_field_key, read_items, read_note};
use crate::store::nfc;
use crate::{Config, Error, Item, ItemId, Notes, Result, Store, random};

/// One change to a note: which item, where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub id: ItemId,
    pub path: String,
    pub line: usize,
}

/// One line of `srl add --stdin`: its header entries, without an ID, and
/// its fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NewItem {
    entries: Entries,
    fields: Vec<Field>,
}

/// What `srl add` did, or would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub ids: Vec<ItemId>,
    /// The new items, as they appear in the note.
    pub text: String,
}

/// `srl id`: gives an ID to every item without one. `notes` must have no
/// problem other than items without an ID.
pub fn assign_ids(store: &Store, notes: &Notes, seed: u64, dry_run: bool) -> Result<Vec<Written>> {
    refuse_problems(notes, true)?;
    let missing = notes
        .problems()
        .iter()
        .filter_map(|problem| {
            if let Error::MissingId { path, line } = problem {
                Some((path.clone(), *line))
            } else {
                None
            }
        })
        .fold(
            BTreeMap::<String, Vec<usize>>::new(),
            |mut by_note, (path, line)| {
                by_note.entry(path).or_default().push(line);
                by_note
            },
        );
    let mut fresh = fresh_ids(notes, seed);
    let mut written = Vec::new();
    for (path, lines) in missing {
        let old = read_note(store, &path)?;
        let ids = lines
            .iter()
            .map(|line| {
                let id = fresh.next().expect("random IDs should never run out");
                (*line, id)
            })
            .collect::<Vec<_>>();
        let new = ids.iter().try_fold(old.clone(), |text, (line, id)| {
            syntax::with_id(&text, *line, id)
                .map(|edit| apply(&text, &edit))
                .ok_or_else(|| no_header(&path, *line))
        })?;
        let expected = keys(&old)
            .into_iter()
            .map(|(line, entries, fields)| {
                let id = ids
                    .iter()
                    .find(|(at, _)| *at == line)
                    .map(|(_, id)| id.clone());
                let entries = Entries {
                    id: id.or(entries.id),
                    ..entries
                };
                (entries, fields)
            })
            .collect::<Vec<_>>();
        commit(store, &path, Some(&old), &new, &expected, dry_run)?;
        written.extend(ids.into_iter().map(|(line, id)| Written {
            id,
            path: path.clone(),
            line,
        }));
    }
    Ok(written)
}

/// `srl accept`: removes `draft` from the headers of `items` that have it.
pub fn accept(
    store: &Store,
    notes: &Notes,
    items: &[&Item],
    dry_run: bool,
) -> Result<Vec<Written>> {
    refuse_problems(notes, false)?;
    let by_note = items.iter().filter(|item| item.is_draft()).fold(
        BTreeMap::<&str, Vec<&Item>>::new(),
        |mut by_note, item| {
            by_note.entry(item.path()).or_default().push(item);
            by_note
        },
    );
    let mut written = Vec::new();
    for (path, drafts) in by_note {
        let old = read_note(store, path)?;
        let new = drafts.iter().try_fold(old.clone(), |text, item| {
            syntax::without_draft(&text, item.line())
                .map(|edit| apply(&text, &edit))
                .ok_or_else(|| no_header(path, item.line()))
        })?;
        let lines = drafts.iter().map(|item| item.line()).collect::<Vec<_>>();
        let expected = keys(&old)
            .into_iter()
            .map(|(line, entries, fields)| {
                let draft = entries.draft && !lines.contains(&line);
                (Entries { draft, ..entries }, fields)
            })
            .collect::<Vec<_>>();
        commit(store, path, Some(&old), &new, &expected, dry_run)?;
        written.extend(drafts.iter().map(|item| Written {
            id: item.id().clone(),
            path: path.to_owned(),
            line: item.line(),
        }));
    }
    Ok(written)
}

/// `srl add NOTE`: appends the items of `input` (one JSON object of header
/// entries and fields per line) to the note at `path`, or creates it.
pub fn add(
    store: &Store,
    config: &Config,
    notes: &Notes,
    path: &str,
    input: &str,
    seed: u64,
    dry_run: bool,
) -> Result<Added> {
    refuse_problems(notes, false)?;
    check_note_path(config, path)?;
    let lines = input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            item_of(notes, line)
                .map(|item| (index + 1, item))
                .map_err(|reason| Error::InvalidLine {
                    line: index + 1,
                    reason,
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let ids = fresh_ids(notes, seed).take(lines.len()).collect::<Vec<_>>();
    let items = lines
        .iter()
        .zip(&ids)
        .map(|((line, item), id)| {
            let entries = Entries {
                id: Some(id.clone()),
                ..item.entries.clone()
            };
            syntax::format(&entries, &item.fields).map_err(|reason| Error::InvalidLine {
                line: *line,
                reason,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    check_added(
        path,
        &items,
        &lines.iter().map(|(line, _)| *line).collect::<Vec<_>>(),
        notes,
    )?;
    let old = read_if_exists(&store.note_file(path))?;
    let ending = line_ending(old.as_deref().unwrap_or_default());
    let added = format!("{}\n", items.join("\n\n"));
    let tail = added.replace('\n', ending);
    let new = match old.as_deref() {
        None | Some("") => tail,
        Some(text) => {
            let separator = if text.ends_with('\n') { "\n" } else { "\n\n" };
            format!("{text}{}{tail}", separator.replace('\n', ending))
        }
    };
    let expected = keys(old.as_deref().unwrap_or_default())
        .into_iter()
        .map(|(_, entries, fields)| (entries, fields))
        .chain(items.iter().flat_map(|item| {
            keys(item)
                .into_iter()
                .map(|(_, entries, fields)| (entries, fields))
        }))
        .collect::<Vec<_>>();
    commit(store, path, old.as_deref(), &new, &expected, dry_run)?;
    Ok(Added { ids, text: added })
}

/// `srl edit ID --stdin`: replaces `item` with `input`, which must be
/// exactly one item with the same ID, as the syntax reads one given on its
/// own (see [`syntax::parse_one`] and [`syntax::replace`]).
pub fn replace(
    store: &Store,
    notes: &Notes,
    item: &Item,
    input: &str,
    dry_run: bool,
) -> Result<()> {
    refuse_problems(notes, false)?;
    let invalid = |reason: String| Error::InvalidEdit {
        id: item.id().to_string(),
        reason,
    };
    let new_item = syntax::parse_one(input).map_err(invalid)?;
    if new_item.entries.id.as_ref() != Some(item.id()) {
        return Err(invalid("its header has another ID, or none".to_owned()));
    }
    if let Some(problem) = read_items(item.path(), input, notes.programs()).1.first() {
        return Err(invalid(reason_of(problem)));
    }
    let old = read_note(store, item.path())?;
    let still_there = syntax::parse(&old)
        .items
        .iter()
        .any(|raw| raw.line == item.line() && raw.entries.id.as_ref() == Some(item.id()));
    if !still_there {
        return Err(Error::ChangedMeanwhile {
            path: item.path().to_owned(),
        });
    }
    let edit = syntax::replace(&old, item.line(), input).map_err(invalid)?;
    let new = apply(&old, &edit);
    let expected = keys(&old)
        .into_iter()
        .map(|(line, entries, fields)| {
            if line == item.line() {
                (new_item.entries.clone(), new_item.fields.clone())
            } else {
                (entries, fields)
            }
        })
        .collect::<Vec<_>>();
    commit(store, item.path(), Some(&old), &new, &expected, dry_run)
}

/// The error for a change to a header that is not on line `line` of the
/// note at `path`, which can only be a bug in `srl`.
fn no_header(path: &str, line: usize) -> Error {
    Error::CannotWrite {
        path: path.to_owned(),
        reason: format!("line {line} holds no header (a bug in srl; please report it)"),
    }
}

/// New item IDs from `seed`, each used by no item of `notes` and given
/// only once.
fn fresh_ids(notes: &Notes, seed: u64) -> impl Iterator<Item = ItemId> {
    let mut taken = notes
        .items()
        .iter()
        .map(|item| item.id().clone())
        .collect::<HashSet<_>>();
    random::numbers(seed)
        .map(ItemId::from_number)
        .filter(move |id| taken.insert(id.clone()))
}

/// Verifies that `new` reads back as the `expected` items, then, unless
/// `dry_run`, writes it over `old` (see [`verify`] and [`replace_file`]).
fn commit(
    store: &Store,
    path: &str,
    old: Option<&str>,
    new: &str,
    expected: &[(Entries, Vec<Field>)],
    dry_run: bool,
) -> Result<()> {
    verify(path, new, expected)?;
    if dry_run {
        Ok(())
    } else {
        replace_file(store, path, old, new)
    }
}

/// The text of `file`, or `None` if it doesn't exist.
fn read_if_exists(file: &Path) -> Result<Option<String>> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io(file)(error)),
    }
}

/// Fails if `notes` has problems, except items without an ID when
/// `missing_ids` allows them.
fn refuse_problems(notes: &Notes, missing_ids: bool) -> Result<()> {
    let count = notes
        .problems()
        .iter()
        .filter(|problem| !(missing_ids && matches!(problem, Error::MissingId { .. })))
        .count();
    if count == 0 {
        Ok(())
    } else {
        Err(Error::NotesInvalid { count })
    }
}

/// The reason of a problem in a note, without its place.
/// Checks `items`, the text of new items for the note at `path` that came
/// from the lines `lines` of standard input, as they would be read once
/// added, all at once, so that each kind program is asked once about all of
/// them. The first problem is an error about its line, unless it is a kind
/// program failing as a whole, which is no line's fault.
fn check_added(path: &str, items: &[String], lines: &[usize], notes: &Notes) -> Result<()> {
    // The items as `srl add` appends them, a blank line between two, and
    // the line each starts at.
    let mut starts = Vec::new();
    let mut text = String::new();
    let mut next = 1;
    for item in items {
        if !text.is_empty() {
            text.push('\n');
        }
        starts.push(next);
        next += item.lines().count() + 1;
        text.push_str(item);
        text.push('\n');
    }
    let Some(problem) = read_items(path, &text, notes.programs())
        .1
        .into_iter()
        .next()
    else {
        return Ok(());
    };
    let Some((_, at)) = problem.place() else {
        return Err(problem);
    };
    let line = starts
        .iter()
        .zip(lines)
        .rev()
        .find(|(start, _)| **start <= at)
        .map_or(0, |(_, line)| *line);
    Err(Error::InvalidLine {
        line,
        reason: reason_of(&problem),
    })
}

fn reason_of(problem: &Error) -> String {
    if let Error::Note { reason, .. } = problem {
        reason.clone()
    } else {
        problem.to_string()
    }
}

/// What verification compares: each item's line, entries and fields.
fn keys(text: &str) -> Vec<(usize, Entries, Vec<Field>)> {
    syntax::parse(text)
        .items
        .into_iter()
        .map(
            |RawItem {
                 entries,
                 line,
                 fields,
                 ..
             }| (line, entries, fields),
        )
        .collect()
}

/// Checks that `new` parses without errors into exactly the `expected`
/// items. A mismatch is a bug in `srl`, so nothing is written.
fn verify(path: &str, new: &str, expected: &[(Entries, Vec<Field>)]) -> Result<()> {
    let parsed = syntax::parse(new);
    let found = parsed
        .items
        .into_iter()
        .map(|item| (item.entries, item.fields))
        .collect::<Vec<_>>();
    if parsed.errors.is_empty() && found == expected {
        Ok(())
    } else {
        Err(Error::CannotWrite {
            path: path.to_owned(),
            reason: "the changed note did not read back as intended (a bug in srl; please \
                     report it)"
                .to_owned(),
        })
    }
}

/// The line ending of a note: that of its first line, or `\n`.
fn line_ending(text: &str) -> &'static str {
    match text.split_once('\n') {
        Some((first, _)) if first.ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

/// `text` with `edit` made: its lines joined with the note's line ending,
/// the last one getting the ending of the last line replaced, and a byte
/// order mark kept.
fn apply(text: &str, edit: &Edit) -> String {
    let (first, last) = (edit.first, edit.last);
    let replacement = edit.lines.join(line_ending(text));
    let raw = text.split_inclusive('\n').collect::<Vec<_>>();
    let before = raw.get(..first - 1).unwrap_or_default().concat();
    let last_line = raw.get(last - 1).copied().unwrap_or_default();
    let ending = last_line
        .get(last_line.trim_end_matches(['\n', '\r']).len()..)
        .unwrap_or_default();
    let bom = if first == 1 && text.starts_with('\u{feff}') {
        "\u{feff}"
    } else {
        ""
    };
    let after = raw.get(last..).unwrap_or_default().concat();
    format!("{before}{bom}{replacement}{ending}{after}")
}

/// Writes `new` to the note at `path`, if the file still holds `old` (or,
/// with `None`, still doesn't exist): to a temporary file in the same
/// folder, renamed over the note.
///
// TODO(platform): Not run on Windows yet. Windows refuses to rename over a
// file that another program holds open without sharing it; the error is
// then reported and the note left as it was. See docs/notes.md, "How srl
// writes to notes", and docs/rust.md, "Platform independence".
fn replace_file(store: &Store, path: &str, old: Option<&str>, new: &str) -> Result<()> {
    let file = store.note_file(path);
    if read_if_exists(&file)?.as_deref() != old {
        return Err(Error::ChangedMeanwhile {
            path: path.to_owned(),
        });
    }
    let folder = file.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(folder).map_err(Error::io(folder))?;
    let cannot = |reason: String| Error::CannotWrite {
        path: path.to_owned(),
        reason,
    };
    let mut temporary = tempfile::Builder::new()
        .prefix(".srl-")
        .suffix(".tmp")
        .tempfile_in(folder)
        .map_err(|error| cannot(error.to_string()))?;
    temporary
        .write_all(new.as_bytes())
        .map_err(|error| cannot(error.to_string()))?;
    temporary
        .persist(&file)
        .map_err(|error| cannot(error.error.to_string()))?;
    Ok(())
}

/// Checks that `path` can name a new note (see `docs/cli.md`, "Adding
/// items").
fn check_note_path(config: &Config, path: &str) -> Result<()> {
    let invalid = |reason: &str| Error::InvalidNotePath {
        path: path.to_owned(),
        reason: reason.to_owned(),
    };
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return Err(invalid(
            "give a path relative to the store's folder, with / between folders",
        ));
    }
    if path
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('.'))
    {
        return Err(invalid(
            "the path has an empty part, `.`, `..`, or a folder starting with `.`, which is not \
             read for notes",
        ));
    }
    if !config.notes.include_set().is_match(nfc(path)) {
        return Err(invalid(
            "the path is not matched by `include` in .srl/config.toml",
        ));
    }
    if config.notes.exclude_set().is_match(nfc(path)) {
        return Err(invalid(
            "the path is excluded by `exclude` in .srl/config.toml",
        ));
    }
    Ok(())
}

/// One line of `srl add --stdin`: a JSON object whose keys are field names,
/// with text values, or the header entries `kind` and `see`, with text
/// values, and `draft`, with `true` or `false`. A `see` must name a note of
/// `notes`. Errors are plain reasons.
fn item_of(notes: &Notes, line: &str) -> std::result::Result<NewItem, String> {
    let object = serde_json::from_str::<Map<String, Value>>(line)
        .map_err(|error| format!("it is not a JSON object ({error})"))?;
    let mut item = NewItem::default();
    for (key, value) in object {
        if key == "draft" {
            let Value::Bool(draft) = value else {
                return Err(format!(
                    "the value of `draft` must be true or false, not {value}"
                ));
            };
            item.entries.draft = draft;
            continue;
        }
        let Value::String(text) = value else {
            return Err(format!("the value of `{key}` must be text, not {value}"));
        };
        if key == "kind" {
            item.entries.kind = Some(text);
        } else if key == "see" {
            let see_nfc = nfc(&text);
            if !notes.paths().iter().any(|known| nfc(known) == see_nfc) {
                return Err(format!(
                    "`see` {text} doesn't name a note of the store; give a path relative to the \
                     store's folder"
                ));
            }
            item.entries.see = Some(text);
        } else if is_field_key(&key) {
            item.fields.push(Field { key, text });
        } else {
            return Err(format!(
                "`{key}` is neither a header entry (`kind`, `see`, `draft`; every item gets a \
                 new ID) nor a field name (uppercase letters, digits and _, starting with a \
                 letter)"
            ));
        }
    }
    if item.fields.is_empty() {
        return Err("it has no fields".to_owned());
    }
    Ok(item)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::note::is_id_text as is_id;
    use crate::{Index, Lookup};

    fn store(files: &[(&str, &str)]) -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        for (path, text) in files {
            let file = store.note_file(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }
        (tmp, store)
    }

    fn notes(store: &Store) -> Notes {
        Notes::load(store, &Config::default(), &mut Index::open(store)).unwrap()
    }

    fn read(store: &Store, path: &str) -> String {
        fs::read_to_string(store.note_file(path)).unwrap()
    }

    #[test]
    fn ids_are_added_to_headers_without_one_and_nothing_else_changes() {
        let text = "\u{feff}```srl\r\nQ: a\r\nA: b\r\n```\r\n\r\nProse.\r\n~~~  srl\r\n@  draft\r\nQ: c\r\nA: d\r\n@ id:keep\r\nQ: e\r\nA: f\r\n~~~";
        let (_tmp, store) = store(&[("a.md", text)]);
        let written = assign_ids(&store, &notes(&store), 7, false).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!((written[0].line, written[1].line), (1, 8));
        assert!(written.iter().all(|done| is_id(done.id.as_str())));
        let (one, two) = (&written[0].id, &written[1].id);
        assert_eq!(
            read(&store, "a.md"),
            format!(
                "\u{feff}```srl id:{one}\r\nQ: a\r\nA: b\r\n```\r\n\r\nProse.\r\n~~~  srl\r\n@ id:{two}  draft\r\nQ: c\r\nA: d\r\n@ id:keep\r\nQ: e\r\nA: f\r\n~~~"
            )
        );
        assert!(notes(&store).problems().is_empty());
        assert!(
            assign_ids(&store, &notes(&store), 7, false)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn ids_are_not_added_while_other_problems_remain() {
        let (_tmp, store) =
            store(&[("a.md", "```srl\nQ: a\nA: b\n```\n```srl darft\nQ: c\n```\n")]);
        let before = read(&store, "a.md");
        assert!(matches!(
            assign_ids(&store, &notes(&store), 7, false),
            Err(Error::NotesInvalid { count: 1 })
        ));
        assert_eq!(read(&store, "a.md"), before);
    }

    #[test]
    fn dry_runs_write_nothing() {
        let (_tmp, store) = store(&[("a.md", "```srl\nQ: a\nA: b\n```\n")]);
        let before = read(&store, "a.md");
        assert_eq!(
            assign_ids(&store, &notes(&store), 7, true).unwrap().len(),
            1
        );
        assert_eq!(read(&store, "a.md"), before);
    }

    #[test]
    fn accept_removes_draft_only() {
        let text = "```srl draft id:aaa1\nQ: a\nA: b\n```\n\n```srl\n@ id:aaa2 draft see:a.md\nQ: c\nA: d\n\n@ id:aaa3 draft\nQ: e\nA: f\n```\n";
        let (_tmp, store) = store(&[("a.md", text)]);
        let notes = notes(&store);
        let chosen = notes.items().iter().take(2).collect::<Vec<_>>();
        let written = accept(&store, &notes, &chosen, false).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            read(&store, "a.md"),
            "```srl id:aaa1\nQ: a\nA: b\n```\n\n```srl\n@ id:aaa2 see:a.md\nQ: c\nA: d\n\n@ id:aaa3 draft\nQ: e\nA: f\n```\n"
        );
    }

    #[test]
    fn add_appends_items_after_a_blank_line_or_creates_the_note() {
        let (_tmp, store) = store(&[("a.md", "Prose.\r\n"), ("my notes/b c.md", "")]);
        let input = "{\"Q\": \"Two\\nlines\\n\\nand more\", \"see\": \"my notes/b c.md\", \"A\": \"x\", \"draft\": true}\n\n{\"kind\": \"qa\", \"Q\": \"{{c1::y}}\\n```\\n````rust\", \"draft\": false}\n";
        let added = add(
            &store,
            &Config::default(),
            &notes(&store),
            "a.md",
            input,
            3,
            false,
        )
        .unwrap();
        let [one, two] = added.ids.as_slice() else {
            panic!("expected two IDs");
        };
        let expected = format!(
            "```srl id:{one} see:\"my notes/b c.md\" draft\nQ: Two\n  lines\n\n  and more\nA: x\n```\n\n`````srl id:{two} kind:qa\nQ: {{{{c1::y}}}}\n  ```\n  ````rust\n`````\n"
        );
        assert_eq!(added.text, expected);
        assert_eq!(
            read(&store, "a.md"),
            format!("Prose.\r\n\r\n{}", expected.replace('\n', "\r\n"))
        );
        let created = add(
            &store,
            &Config::default(),
            &notes(&store),
            "new/b.md",
            "{\"Q\": \"q\", \"A\": \"a\"}",
            4,
            false,
        )
        .unwrap();
        assert_eq!(read(&store, "new/b.md"), created.text);
    }

    #[test]
    fn add_checks_every_line_first() {
        let (_tmp, store) = store(&[("a.md", "Prose.\n")]);
        for input in [
            "{\"Q\": \"q\"}",
            "{\"q\": \"q\", \"A\": \"a\"}",
            "{\"Q\": 1, \"A\": \"a\"}",
            "[1]",
            "{}",
            "{\"see\": \"a.md\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": \"nowhere.md\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"see\": 1}",
            "{\"Q\": \"q\", \"A\": \"a\", \"kind\": \"nope\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"kind\": \"\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"draft\": \"yes\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"id\": \"abc123\"}",
            "{\"Q\": \"q\", \"A\": \"a\"}\nnot json",
        ] {
            assert!(
                add(
                    &store,
                    &Config::default(),
                    &notes(&store),
                    "a.md",
                    input,
                    3,
                    false
                )
                .is_err(),
                "accepted {input}"
            );
        }
        for path in [
            "a.txt",
            "/a.md",
            "../a.md",
            ".hidden/a.md",
            "a//b.md",
            "a\\b.md",
        ] {
            assert!(
                add(
                    &store,
                    &Config::default(),
                    &notes(&store),
                    path,
                    "{\"Q\": \"q\", \"A\": \"a\"}",
                    3,
                    false
                )
                .is_err(),
                "accepted {path}"
            );
        }
        assert_eq!(read(&store, "a.md"), "Prose.\n");
    }

    #[test]
    fn add_takes_any_path_include_matches() {
        let (_tmp, store) = store(&[]);
        let mut config = Config::default();
        config.notes.include = vec!["**/*.txt".to_owned()];
        let item = "{\"Q\": \"q\", \"A\": \"a\"}";
        assert!(add(&store, &config, &notes(&store), "a.md", item, 3, false).is_err());
        add(&store, &config, &notes(&store), "new/a.txt", item, 3, false).unwrap();
        assert!(read(&store, "new/a.txt").contains("Q: q"));
    }

    #[test]
    fn replace_swaps_one_item_and_keeps_the_rest() {
        let text = "Before.\n```srl id:aaa1\nQ: old\nA: old\n```\n\n````srl\n@ id:aaa2\nQ: old\nA: old\n\n@ id:aaa3\nQ: old\nA: old\n````\nAfter.\n";
        let (_tmp, store) = store(&[("a.md", text)]);
        let notes = notes(&store);
        replace(
            &store,
            &notes,
            &notes.items()[0],
            "~~~srl id:aaa1 draft\nQ: new\n\nA: new\n  more\n~~~\n",
            false,
        )
        .unwrap();
        let notes = self::notes(&store);
        replace(
            &store,
            &notes,
            &notes.items()[1],
            "````srl id:aaa2  kind:qa\nQ: new\n  ```\n  code\n  ```\nA: a\n````\n",
            false,
        )
        .unwrap();
        assert_eq!(
            read(&store, "a.md"),
            "Before.\n~~~srl id:aaa1 draft\nQ: new\n\nA: new\n  more\n~~~\n\n````srl\n@ id:aaa2  kind:qa\nQ: new\n  ```\n  code\n  ```\nA: a\n\n@ id:aaa3\nQ: old\nA: old\n````\nAfter.\n"
        );
        let notes = self::notes(&store);
        assert!(
            replace(
                &store,
                &notes,
                &notes.items()[2],
                "`````srl id:aaa3\nQ: q\n  ````\n`````\n",
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn replace_needs_exactly_one_item_with_the_same_id() {
        let (_tmp, store) = store(&[("a.md", "```srl id:aaa1\nQ: q\nA: a\n```\n")]);
        let notes = notes(&store);
        let item = &notes.items()[0];
        for input in [
            "```srl id:aaa2\nQ: q\nA: a\n```\n",
            "```srl\nQ: q\nA: a\n```\n",
            "```srl id:aaa1\nQ: q\nA: a\n```\nProse.\n",
            "```srl\n@ id:aaa1\nQ: q\nA: a\n@ id:aaa1\nQ: q\nA: a\n```\n",
            "```srl id:aaa1\nQ: q\n```\n",
            "```srl id:aaa1\nQ: q\nA: a\n",
            "Q: q\nA: a\n",
            "",
        ] {
            assert!(
                replace(&store, &notes, item, input, false).is_err(),
                "accepted {input:?}"
            );
        }
    }

    #[test]
    fn a_note_changed_meanwhile_is_not_written() {
        let (_tmp, store) = store(&[("a.md", "```srl\nQ: a\nA: b\n```\n")]);
        assert!(matches!(
            replace_file(&store, "a.md", Some("something else"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert!(matches!(
            replace_file(&store, "b.md", Some("x"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert_eq!(read(&store, "a.md"), "```srl\nQ: a\nA: b\n```\n");
    }
}
