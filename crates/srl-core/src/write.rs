//! The only place that changes notes: `srl id`, `srl accept`, `srl add`
//! and `srl edit ID --stdin` (see `docs/notes.md`, "How srl writes to
//! notes", and `docs/rust.md`, "Note editing").
//!
//! Every change edits lines, never data: header lines get an `id:` or lose
//! `draft`, one item's lines are replaced, or lines are appended. The new
//! text is parsed again and compared with the old before it is written,
//! the file must not have changed since it was read, and it is replaced
//! atomically.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::Path;

use serde_json::{Map, Value};

use crate::note::parse::{self, Field, Header, Line, RawItem};
use crate::note::{read_items, read_note};
use crate::store::nfc;
use crate::{Config, Error, Item, ItemId, Notes, Result, Store, random};

/// One change to a note: which item, where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub id: ItemId,
    pub path: String,
    pub line: usize,
}

/// One line of `srl add --stdin`: its header entries and its fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NewItem {
    kind: Option<String>,
    see: Option<String>,
    draft: bool,
    fields: Vec<(String, String)>,
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
            edit_line(&text, *line, |header| {
                let start = parse::header_start(header)
                    .expect("a line with a missing ID should be a header");
                let (before, after) = header.split_at(start);
                format!("{before} id:{id}{after}")
            })
        })?;
        let expected = keys(&old)
            .into_iter()
            .map(|(line, header, fields)| {
                let id = ids
                    .iter()
                    .find(|(at, _)| *at == line)
                    .map(|(_, id)| id.clone());
                let header = Header {
                    id: id.or(header.id),
                    ..header
                };
                (header, fields)
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
            edit_line(&text, item.line(), |header| {
                let span = parse::header(header)
                    .and_then(std::result::Result::ok)
                    .and_then(|line| line.draft)
                    .expect("a draft's header should have `draft`");
                let before = header.get(..span.start).unwrap_or_default();
                let after = header.get(span.end..).unwrap_or_default();
                format!("{}{after}", before.trim_end_matches(' '))
            })
        })?;
        let lines = drafts.iter().map(|item| item.line()).collect::<Vec<_>>();
        let expected = keys(&old)
            .into_iter()
            .map(|(line, header, fields)| {
                let draft = header.draft && !lines.contains(&line);
                (Header { draft, ..header }, fields)
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
            let invalid = |reason| Error::InvalidLine {
                line: *line,
                reason,
            };
            let text = format_item(id, item).map_err(invalid)?;
            match read_items(path, &text).1.first() {
                Some(problem) => Err(invalid(reason_of(problem))),
                None => Ok(text),
            }
        })
        .collect::<Result<Vec<_>>>()?;
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
        .map(|(_, header, fields)| (header, fields))
        .chain(items.iter().flat_map(|item| {
            keys(item)
                .into_iter()
                .map(|(_, header, fields)| (header, fields))
        }))
        .collect::<Vec<_>>();
    commit(store, path, old.as_deref(), &new, &expected, dry_run)?;
    Ok(Added { ids, text: added })
}

/// `srl edit ID --stdin`: replaces the lines of `item` with `input`, which
/// must be exactly one item with the same ID.
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
    let parsed = parse::parse(input);
    if let Some((line, reason)) = parsed.errors.first() {
        return Err(invalid(format!("line {line}: {reason}")));
    }
    let [new_item] = parsed.items.as_slice() else {
        return Err(invalid(format!("it holds {} items", parsed.items.len())));
    };
    if new_item.header.id.as_ref() != Some(item.id()) {
        return Err(invalid("its header has another ID, or none".to_owned()));
    }
    let lines = parse::lines(input);
    let stray = lines.iter().zip(&parsed.lines).any(|(line, kind)| {
        (*kind == Line::Text && !line.trim().is_empty()) || *kind == Line::Code
    });
    if stray {
        return Err(invalid("it has text outside the item".to_owned()));
    }
    if let Some(problem) = read_items(item.path(), input).1.first() {
        return Err(invalid(reason_of(problem)));
    }
    let old = read_note(store, item.path())?;
    let ending = line_ending(&old);
    let replacement = lines
        .get(new_item.line - 1..new_item.end)
        .unwrap_or_default()
        .join(ending);
    let new = replace_lines(&old, item.line(), item.end(), &replacement);
    let expected = keys(&old)
        .into_iter()
        .map(|(line, header, fields)| {
            if line == item.line() {
                (new_item.header.clone(), new_item.fields.clone())
            } else {
                (header, fields)
            }
        })
        .collect::<Vec<_>>();
    commit(store, item.path(), Some(&old), &new, &expected, dry_run)
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
    expected: &[(Header, Vec<Field>)],
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
fn reason_of(problem: &Error) -> String {
    if let Error::Note { reason, .. } = problem {
        reason.clone()
    } else {
        problem.to_string()
    }
}

/// What verification compares: each item's line, header and fields.
fn keys(text: &str) -> Vec<(usize, Header, Vec<Field>)> {
    parse::parse(text)
        .items
        .into_iter()
        .map(
            |RawItem {
                 header,
                 line,
                 fields,
                 ..
             }| (line, header, fields),
        )
        .collect()
}

/// Checks that `new` parses without errors into exactly the `expected`
/// items. A mismatch is a bug in `srl`, so nothing is written.
fn verify(path: &str, new: &str, expected: &[(Header, Vec<Field>)]) -> Result<()> {
    let parsed = parse::parse(new);
    let found = parsed
        .items
        .into_iter()
        .map(|item| (item.header, item.fields))
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

/// `text` with line `number` (counting from 1) replaced by what `edit`
/// makes of it. The line's ending and a byte order mark stay as they are.
fn edit_line(text: &str, number: usize, edit: impl Fn(&str) -> String) -> Result<String> {
    let raw = text.split_inclusive('\n').collect::<Vec<_>>();
    let line = raw.get(number - 1).ok_or_else(|| Error::CannotWrite {
        path: String::new(),
        reason: format!("line {number} is gone"),
    })?;
    let (bom, rest) = match line.strip_prefix('\u{feff}') {
        Some(rest) if number == 1 => ("\u{feff}", rest),
        Some(_) | None => ("", *line),
    };
    let content = rest.trim_end_matches(['\n', '\r']);
    let ending = rest.get(content.len()..).unwrap_or_default();
    let before = raw.get(..number - 1).unwrap_or_default().concat();
    let after = raw.get(number..).unwrap_or_default().concat();
    Ok(format!("{before}{bom}{}{ending}{after}", edit(content)))
}

/// `text` with lines `first` to `last` (counting from 1) replaced by
/// `replacement`, which gets the ending of line `last`.
fn replace_lines(text: &str, first: usize, last: usize, replacement: &str) -> String {
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
    if Path::new(path)
        .extension()
        .is_none_or(|extension| extension != "md")
    {
        return Err(invalid("a note's name ends in .md"));
    }
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
            item.draft = draft;
            continue;
        }
        let Value::String(text) = value else {
            return Err(format!("the value of `{key}` must be text, not {value}"));
        };
        if key == "kind" {
            item.kind = Some(text);
        } else if key == "see" {
            let see_nfc = nfc(&text);
            if !notes.paths().iter().any(|known| nfc(known) == see_nfc) {
                return Err(format!(
                    "`see` {text} doesn't name a note of the store; give a path relative to the \
                     store's folder"
                ));
            }
            item.see = Some(text);
        } else if parse::is_field_key(&key) {
            item.fields.push((key, text));
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

/// An item as `srl add` writes it: a header, then each field, with
/// continuation lines for multi-line text. Errors are plain reasons.
fn format_item(id: &ItemId, item: &NewItem) -> std::result::Result<String, String> {
    let mut entries = vec![format!("id:{id}")];
    if let Some(kind) = &item.kind {
        if kind.is_empty() || kind.contains([' ', '"']) {
            return Err(format!("`{kind}` can't be a kind"));
        }
        entries.push(format!("kind:{kind}"));
    }
    if let Some(see) = &item.see {
        if see.contains('"') {
            return Err("a `see:` path can't contain `\"`".to_owned());
        }
        entries.push(if see.contains(' ') {
            format!("see:\"{see}\"")
        } else {
            format!("see:{see}")
        });
    }
    if item.draft {
        entries.push("draft".to_owned());
    }
    let mut lines = vec![format!("<!-- srl {} -->", entries.join(" "))];
    for (key, text) in &item.fields {
        let text = text.replace("\r\n", "\n");
        let mut parts = text.split('\n').collect::<Vec<_>>();
        while parts.len() > 1 && parts.last().is_some_and(|line| line.trim().is_empty()) {
            parts.pop();
        }
        let (first, rest) = parts
            .split_first()
            .map_or(("", &[][..]), |(first, rest)| (*first, rest));
        lines.push(if first.is_empty() {
            format!("{key}:")
        } else {
            format!("{key}: {first}")
        });
        lines.extend(rest.iter().map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        }));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::Lookup;
    use crate::note::is_id_text as is_id;

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
        Notes::load(store, &Config::default()).unwrap()
    }

    fn read(store: &Store, path: &str) -> String {
        fs::read_to_string(store.note_file(path)).unwrap()
    }

    #[test]
    fn ids_are_added_to_headers_without_one_and_nothing_else_changes() {
        let text = "\u{feff}<!--srl-->\r\nQ: a\r\nA: b\r\n\r\nProse.\r\n<!-- srl  draft -->\r\nQ: c\r\nA: d\r\n<!-- srl id:keep -->\r\nQ: e\r\nA: f";
        let (_tmp, store) = store(&[("a.md", text)]);
        let written = assign_ids(&store, &notes(&store), 7, false).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!((written[0].line, written[1].line), (1, 6));
        assert!(written.iter().all(|done| is_id(done.id.as_str())));
        let (one, two) = (&written[0].id, &written[1].id);
        assert_eq!(
            read(&store, "a.md"),
            format!(
                "\u{feff}<!--srl id:{one}-->\r\nQ: a\r\nA: b\r\n\r\nProse.\r\n<!-- srl id:{two}  draft -->\r\nQ: c\r\nA: d\r\n<!-- srl id:keep -->\r\nQ: e\r\nA: f"
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
        let (_tmp, store) = store(&[(
            "a.md",
            "<!-- srl -->\nQ: a\nA: b\n<!-- srl darft -->\nQ: c\n",
        )]);
        let before = read(&store, "a.md");
        assert!(matches!(
            assign_ids(&store, &notes(&store), 7, false),
            Err(Error::NotesInvalid { count: 1 })
        ));
        assert_eq!(read(&store, "a.md"), before);
    }

    #[test]
    fn dry_runs_write_nothing() {
        let (_tmp, store) = store(&[("a.md", "<!-- srl -->\nQ: a\nA: b\n")]);
        let before = read(&store, "a.md");
        assert_eq!(
            assign_ids(&store, &notes(&store), 7, true).unwrap().len(),
            1
        );
        assert_eq!(read(&store, "a.md"), before);
    }

    #[test]
    fn accept_removes_draft_only() {
        let text = "<!-- srl draft id:aaa1 -->\nQ: a\nA: b\n\n<!-- srl id:aaa2 draft see:a.md -->\nQ: c\nA: d\n\n<!-- srl id:aaa3 draft -->\nQ: e\nA: f\n";
        let (_tmp, store) = store(&[("a.md", text)]);
        let notes = notes(&store);
        let chosen = notes.items().iter().take(2).collect::<Vec<_>>();
        let written = accept(&store, &notes, &chosen, false).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            read(&store, "a.md"),
            "<!-- srl id:aaa1 -->\nQ: a\nA: b\n\n<!-- srl id:aaa2 see:a.md -->\nQ: c\nA: d\n\n<!-- srl id:aaa3 draft -->\nQ: e\nA: f\n"
        );
    }

    #[test]
    fn add_appends_items_after_a_blank_line_or_creates_the_note() {
        let (_tmp, store) = store(&[("a.md", "Prose.\r\n"), ("my notes/b c.md", "")]);
        let input = "{\"Q\": \"Two\\nlines\\n\\nand more\", \"see\": \"my notes/b c.md\", \"A\": \"x\", \"draft\": true}\n\n{\"kind\": \"qa\", \"Q\": \"{{c1::y}}\", \"draft\": false}\n";
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
            "<!-- srl id:{one} see:\"my notes/b c.md\" draft -->\nQ: Two\n  lines\n\n  and more\nA: x\n\n<!-- srl id:{two} kind:qa -->\nQ: {{{{c1::y}}}}\n"
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
    fn replace_swaps_one_item_and_keeps_the_rest() {
        let text = "Before.\n<!-- srl id:aaa1 -->\nQ: old\nA: old\n\nAfter.\n";
        let (_tmp, store) = store(&[("a.md", text)]);
        let notes = notes(&store);
        let item = &notes.items()[0];
        replace(
            &store,
            &notes,
            item,
            "<!-- srl id:aaa1 draft -->\nQ: new\n\nA: new\n  more\n",
            false,
        )
        .unwrap();
        assert_eq!(
            read(&store, "a.md"),
            "Before.\n<!-- srl id:aaa1 draft -->\nQ: new\n\nA: new\n  more\n\nAfter.\n"
        );
    }

    #[test]
    fn replace_needs_exactly_one_item_with_the_same_id() {
        let (_tmp, store) = store(&[("a.md", "<!-- srl id:aaa1 -->\nQ: q\nA: a\n")]);
        let notes = notes(&store);
        let item = &notes.items()[0];
        for input in [
            "<!-- srl id:aaa2 -->\nQ: q\nA: a\n",
            "<!-- srl -->\nQ: q\nA: a\n",
            "<!-- srl id:aaa1 -->\nQ: q\nA: a\nProse.\n",
            "<!-- srl id:aaa1 -->\nQ: q\nA: a\n<!-- srl id:aaa1 -->\nQ: q\nA: a\n",
            "<!-- srl id:aaa1 -->\nQ: q\n",
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
        let (_tmp, store) = store(&[("a.md", "<!-- srl -->\nQ: a\nA: b\n")]);
        assert!(matches!(
            replace_file(&store, "a.md", Some("something else"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert!(matches!(
            replace_file(&store, "b.md", Some("x"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert_eq!(read(&store, "a.md"), "<!-- srl -->\nQ: a\nA: b\n");
    }
}
