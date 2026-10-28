//! The only place that changes notes: `srl id`, `srl accept`, `srl add`
//! and `srl edit --stdin` (see `docs/notes.md`, "How srl writes to notes",
//! and `docs/rust.md`, "Note editing").
//!
//! Every change edits lines, never data. Which lines, and what they become,
//! is the business of the note's item syntax ([`Syntax::with_id`],
//! [`Syntax::without_draft`], [`Syntax::replace`], [`Syntax::format`]);
//! this module makes the change safe: the new text is parsed again and
//! compared with the old before it is written, the file must not have
//! changed since it was read, and it is replaced atomically.
//!
//! An item to change is found in the note as it is right before the
//! change: by its ID, or, for `srl id`, by having none. It is named to the
//! syntax by its position among the note's items, never by its line.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::Path;

use crate::note::object::{self, Object};
use crate::note::syntax::{self, Edit, ParsedItem, Syntax};
use crate::note::{Entries, Field, check_new, read_note};
use crate::store::nfc;
use crate::{Config, Error, Item, ItemId, Notes, Result, Store, random};

/// One change to a note: which item, where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub id: ItemId,
    pub path: String,
    pub line: usize,
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
    let paths = notes
        .problems()
        .iter()
        .filter_map(|problem| {
            if let Error::MissingId { path, .. } = problem {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    let mut fresh = fresh_ids(notes, HashSet::new(), seed);
    let mut written = Vec::new();
    for path in paths {
        let syntax = syntax::of(&path);
        let (old, items) = read_parsed(store, &path)?;
        // Each item without an ID, by its position, with its new ID.
        let ids = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.entries.id.is_none())
            .map(|(position, item)| {
                let id = fresh.next().expect("random IDs should never run out");
                (position, item.line, id)
            })
            .collect::<Vec<_>>();
        let new = ids
            .iter()
            .try_fold(old.clone(), |text, (position, _, id)| {
                syntax
                    .with_id(&text, *position, id)
                    .map(|edit| apply(&text, &edit))
                    .ok_or_else(|| lost_item(&path))
            })?;
        let expected = items
            .into_iter()
            .enumerate()
            .map(|(position, item)| {
                let id = ids
                    .iter()
                    .find(|(at, _, _)| *at == position)
                    .map(|(_, _, id)| id.clone());
                let entries = Entries {
                    id: id.or(item.entries.id),
                    ..item.entries
                };
                (entries, item.fields)
            })
            .collect::<Vec<_>>();
        commit(store, &path, Some(&old), &new, &expected, dry_run)?;
        written.extend(ids.into_iter().map(|(_, line, id)| Written {
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
        let syntax = syntax::of(path);
        let (old, items) = read_parsed(store, path)?;
        let positions = drafts
            .iter()
            .map(|item| position_of(&items, item.id(), path))
            .collect::<Result<Vec<_>>>()?;
        let new = positions.iter().try_fold(old.clone(), |text, position| {
            syntax
                .without_draft(&text, *position)
                .map(|edit| apply(&text, &edit))
                .ok_or_else(|| lost_item(path))
        })?;
        let expected = items
            .into_iter()
            .enumerate()
            .map(|(position, item)| {
                let draft = item.entries.draft && !positions.contains(&position);
                (
                    Entries {
                        draft,
                        ..item.entries
                    },
                    item.fields,
                )
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

/// `srl add NOTE`: appends the items of `input`, one item object per line,
/// to the note at `path`, or creates it. An item without an `id` gets a new
/// one.
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
    let syntax = syntax::of(path);
    let lines = read_input(notes, input)?;
    let mut given = HashSet::new();
    for (line, object) in &lines {
        let Some(id) = &object.entries.id else {
            continue;
        };
        let reason = if notes.item(id).is_some() {
            format!("the ID {id} is already used by an item; leave `id` out to get a new one")
        } else if !given.insert(id.clone()) {
            format!("the ID {id} is given twice")
        } else {
            continue;
        };
        return Err(Error::InvalidLine {
            line: *line,
            reason,
        });
    }
    let mut fresh = fresh_ids(notes, given, seed);
    let new = lines
        .iter()
        .map(|(_, object)| {
            let id = object
                .entries
                .id
                .clone()
                .or_else(|| fresh.next())
                .expect("random IDs should never run out");
            let entries = Entries {
                id: Some(id),
                ..object.entries.clone()
            };
            (entries, object.fields.clone())
        })
        .collect::<Vec<_>>();
    let items = new
        .iter()
        .zip(&lines)
        .map(|((entries, fields), (line, _))| {
            syntax
                .format(entries, fields)
                .map_err(|reason| Error::InvalidLine {
                    line: *line,
                    reason,
                })
        })
        .collect::<Result<Vec<_>>>()?;
    check_lines(&new, &lines, notes)?;
    let old = read_if_exists(&store.note_file(path))?;
    if old
        .as_deref()
        .is_some_and(|text| !syntax.parse(text).errors.is_empty())
    {
        return Err(Error::ChangedMeanwhile {
            path: path.to_owned(),
        });
    }
    let ending = line_ending(old.as_deref().unwrap_or_default());
    let gap = syntax.gap();
    let added = format!("{}\n", items.join(gap));
    let tail = added.replace('\n', ending);
    let new_text = match old.as_deref() {
        None | Some("") => tail,
        Some(text) => {
            // The note's last line ending is the first of the gap.
            let separator = if text.ends_with('\n') {
                gap.get(1..).unwrap_or_default()
            } else {
                gap
            };
            format!("{text}{}{tail}", separator.replace('\n', ending))
        }
    };
    let expected = keys(syntax, old.as_deref().unwrap_or_default())
        .into_iter()
        .chain(items.iter().flat_map(|item| keys(syntax, item)))
        .collect::<Vec<_>>();
    commit(store, path, old.as_deref(), &new_text, &expected, dry_run)?;
    let ids = new
        .into_iter()
        .filter_map(|(entries, _)| entries.id)
        .collect();
    Ok(Added { ids, text: added })
}

/// `srl edit --stdin`: replaces, for each line of `input`, one item object
/// per line, the item its `id` names. Every line is checked, and every
/// change made and verified, before any note is written. Gives the IDs of
/// the items, in the order of `input`.
pub fn edit(store: &Store, notes: &Notes, input: &str, dry_run: bool) -> Result<Vec<ItemId>> {
    refuse_problems(notes, false)?;
    let lines = read_input(notes, input)?;
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    for (line, object) in &lines {
        let invalid = |reason: String| Error::InvalidLine {
            line: *line,
            reason,
        };
        let id = object.entries.id.as_ref().ok_or_else(|| {
            invalid("it has no `id`; give the ID of the item it replaces".to_owned())
        })?;
        let item = notes
            .item(id)
            .ok_or_else(|| invalid(format!("no item has the ID {id}")))?;
        if !seen.insert(id) {
            return Err(invalid(format!("the item {id} is given twice")));
        }
        targets.push((*line, item, object));
    }
    let new = lines
        .iter()
        .map(|(_, object)| (object.entries.clone(), object.fields.clone()))
        .collect::<Vec<_>>();
    check_lines(&new, &lines, notes)?;
    let by_note = targets.into_iter().fold(
        BTreeMap::<&str, Vec<(usize, &Item, &Object)>>::new(),
        |mut by_note, target| {
            by_note.entry(target.1.path()).or_default().push(target);
            by_note
        },
    );
    let mut changed = Vec::new();
    for (path, edits) in by_note {
        let syntax = syntax::of(path);
        let (old, items) = read_parsed(store, path)?;
        // Replacing an item never adds or removes one, so the positions of
        // the others stay put, in any order.
        let mut text = old.clone();
        let mut replaced = Vec::new();
        for (line, item, object) in edits {
            let invalid = |reason: String| Error::InvalidLine { line, reason };
            let position = position_of(&items, item.id(), path)?;
            let (entries, fields) = (&object.entries, &object.fields);
            let edit = syntax
                .replace(&text, position, entries, fields)
                .map_err(invalid)?;
            text = apply(&text, &edit);
            let formatted = syntax.format(entries, fields).map_err(invalid)?;
            let (entries, fields) = keys(syntax, &formatted)
                .into_iter()
                .next()
                .ok_or_else(|| lost_item(path))?;
            replaced.push((position, entries, fields));
        }
        let expected = items
            .into_iter()
            .enumerate()
            .map(|(position, item)| {
                replaced
                    .iter()
                    .find(|(at, _, _)| *at == position)
                    .map_or((item.entries, item.fields), |(_, entries, fields)| {
                        (entries.clone(), fields.clone())
                    })
            })
            .collect::<Vec<_>>();
        verify(path, &text, &expected)?;
        changed.push((path, old, text));
    }
    if !dry_run {
        for (path, old, new) in changed {
            replace_file(store, path, Some(&old), &new)?;
        }
    }
    Ok(new
        .into_iter()
        .filter_map(|(entries, _)| entries.id)
        .collect())
}

/// The item objects of `input`, one per line, each with its line, counting
/// from 1; blank lines are skipped. A `see` must name a note of `notes`.
fn read_input(notes: &Notes, input: &str) -> Result<Vec<(usize, Object)>> {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            item_of(notes, line)
                .map(|object| (index + 1, object))
                .map_err(|reason| Error::InvalidLine {
                    line: index + 1,
                    reason,
                })
        })
        .collect()
}

/// The text of the note at `path` as it is now, and its items. Commands
/// that write refuse notes with errors, so a note that has some now
/// changed since it was read.
fn read_parsed(store: &Store, path: &str) -> Result<(String, Vec<ParsedItem>)> {
    let text = read_note(store, path)?;
    let parsed = syntax::of(path).parse(&text);
    if parsed.errors.is_empty() {
        Ok((text, parsed.items))
    } else {
        Err(Error::ChangedMeanwhile {
            path: path.to_owned(),
        })
    }
}

/// The position of the item `id` among `items`, those of the note at
/// `path` as it is now. An item that is not there any more was moved or
/// removed since the note was read.
fn position_of(items: &[ParsedItem], id: &ItemId, path: &str) -> Result<usize> {
    items
        .iter()
        .position(|item| item.entries.id.as_ref() == Some(id))
        .ok_or_else(|| Error::ChangedMeanwhile {
            path: path.to_owned(),
        })
}

/// The error for a change to an item the syntax doesn't find where it just
/// read it, which can only be a bug in `srl`.
fn lost_item(path: &str) -> Error {
    Error::CannotWrite {
        path: path.to_owned(),
        reason: "an item to change is not where srl read it (a bug in srl; please report it)"
            .to_owned(),
    }
}

/// New item IDs from `seed`, each used by no item of `notes`, not in
/// `taken`, and given only once.
fn fresh_ids(notes: &Notes, mut taken: HashSet<ItemId>, seed: u64) -> impl Iterator<Item = ItemId> {
    taken.extend(notes.items().iter().map(|item| item.id().clone()));
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

/// Checks the `new` items of the `lines` of standard input, as they would
/// be read in a note, all at once, so that each kind program is asked once
/// about all of them (see [`check_new`]). The first problem is an error
/// about its line, unless it is a kind program failing as a whole, which is
/// no line's fault.
fn check_lines(
    new: &[(Entries, Vec<Field>)],
    lines: &[(usize, Object)],
    notes: &Notes,
) -> Result<()> {
    let Some(problem) = check_new(new, notes.programs()).into_iter().next() else {
        return Ok(());
    };
    let Some((_, at)) = problem.place() else {
        return Err(problem);
    };
    let line = lines.get(at - 1).map_or(0, |(line, _)| *line);
    let reason = if let Error::Note { reason, .. } = &problem {
        reason.clone()
    } else {
        problem.to_string()
    };
    Err(Error::InvalidLine { line, reason })
}

/// What verification compares: each item's entries and fields, in order.
fn keys(syntax: &dyn Syntax, text: &str) -> Vec<(Entries, Vec<Field>)> {
    syntax
        .parse(text)
        .items
        .into_iter()
        .map(|item| (item.entries, item.fields))
        .collect()
}

/// Checks that `new` parses without errors into exactly the `expected`
/// items. A mismatch is a bug in `srl`, so nothing is written.
fn verify(path: &str, new: &str, expected: &[(Entries, Vec<Field>)]) -> Result<()> {
    let parsed = syntax::of(path).parse(new);
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

/// One line of `srl add --stdin` or `srl edit --stdin`: an item object
/// whose `see`, if any, names a note of `notes`. Errors are plain reasons.
fn item_of(notes: &Notes, line: &str) -> std::result::Result<Object, String> {
    let object = object::read(line)?;
    if let Some(see) = &object.entries.see {
        let see_nfc = nfc(see);
        if !notes.paths().iter().any(|known| nfc(known) == see_nfc) {
            return Err(format!(
                "`see` {see} doesn't name a note of the store; give a path relative to the \
                 store's folder"
            ));
        }
    }
    Ok(object)
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
            "```srl id:{one} see:\"my notes/b c.md\" draft\nQ: Two\n  lines\n\n  and more\nA: x\n```\n\n`````srl id:{two}\nQ: {{{{c1::y}}}}\n  ```\n  ````rust\n`````\n"
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
            "{\"Q\": \"q\", \"A\": \"a\", \"id\": \"ABC\"}",
            "{\"Q\": \"q\", \"A\": \"a\", \"id\": \"abc\"}\n{\"Q\": \"r\", \"A\": \"a\", \"id\": \"abc\"}",
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
    fn add_writes_json_lines_to_a_jsonl_note_and_keeps_given_ids() {
        let mut config = Config::default();
        config.notes.include = vec!["**/*.md".to_owned(), "**/*.jsonl".to_owned()];
        let (_tmp, store) = store(&[("a.jsonl", "{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}")]);
        let notes = Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        let input = "{\"id\": \"mine\", \"Q\": \"r\", \"A\": \"b\", \"kind\": \"qa\", \"see\": \"a.jsonl\"}\n{\"draft\": true, \"Q\": \"{{c1::s}}\"}\n";
        let added = add(&store, &config, &notes, "a.jsonl", input, 3, false).unwrap();
        assert_eq!(added.ids[0].as_str(), "mine");
        let two = &added.ids[1];
        assert_eq!(
            read(&store, "a.jsonl"),
            format!(
                "{{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}}\n{{\"id\":\"mine\",\"see\":\"a.jsonl\",\"Q\":\"r\",\"A\":\"b\"}}\n{{\"id\":\"{two}\",\"draft\":true,\"Q\":\"{{{{c1::s}}}}\"}}\n"
            )
        );
        let notes = Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        assert!(notes.problems().is_empty(), "{:?}", notes.problems());
        let taken = "{\"id\": \"old\", \"Q\": \"q\", \"A\": \"a\"}";
        assert!(add(&store, &config, &notes, "a.jsonl", taken, 3, false).is_err());
    }

    #[test]
    fn ids_and_accept_write_json_lines_again() {
        let mut config = Config::default();
        config.notes.include = vec!["*.jsonl".to_owned()];
        let text = "{\"Q\": \"q\", \"A\": \"a\", \"draft\": true}\r\n\r\n{\"id\": \"keep\", \"Q\": \"r\", \"A\": \"b\"}\r\n";
        let (_tmp, store) = store(&[("a.jsonl", text)]);
        let load = || Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        let written = assign_ids(&store, &load(), 7, false).unwrap();
        let id = &written[0].id;
        let notes = load();
        let drafts = notes.items().iter().collect::<Vec<_>>();
        accept(&store, &notes, &drafts, false).unwrap();
        assert_eq!(
            read(&store, "a.jsonl"),
            format!(
                "{{\"id\":\"{id}\",\"Q\":\"q\",\"A\":\"a\"}}\r\n\r\n{{\"id\": \"keep\", \"Q\": \"r\", \"A\": \"b\"}}\r\n"
            )
        );
    }

    #[test]
    fn edit_replaces_items_in_any_syntax_and_keeps_the_rest() {
        let mut config = Config::default();
        config.notes.include = vec!["*.md".to_owned(), "*.jsonl".to_owned()];
        let text = "Before.\n```srl id:aaa1\nQ: old\nA: old\n```\n\n````srl\n@ id:aaa2\nQ: old\nA: old\n\n@ id:aaa3\nQ: old\nA: old\n````\nAfter.\n";
        let lines = "{\"id\": \"bbb1\", \"Q\": \"old\", \"A\": \"old\"}\n";
        let (_tmp, store) = store(&[("a.md", text), ("b.jsonl", lines)]);
        let notes = Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        let input = "{\"id\": \"aaa2\", \"Q\": \"new\\n```\\ncode\\n```\", \"A\": \"a\", \"path\": \"x\", \"cards\": []}\n{\"id\": \"aaa1\", \"draft\": true, \"Q\": \"new\", \"A\": \"new\\n\\nmore\"}\n{\"id\": \"bbb1\", \"Q\": \"new\", \"A\": \"a\", \"see\": \"a.md\"}\n";
        let ids = edit(&store, &notes, input, false).unwrap();
        let ids = ids.iter().map(ItemId::as_str).collect::<Vec<_>>();
        assert_eq!(ids, ["aaa2", "aaa1", "bbb1"]);
        assert_eq!(
            read(&store, "a.md"),
            "Before.\n```srl id:aaa1 draft\nQ: new\nA: new\n\n  more\n```\n\n````srl\n@ id:aaa2\nQ: new\n  ```\n  code\n  ```\nA: a\n\n@ id:aaa3\nQ: old\nA: old\n````\nAfter.\n"
        );
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\":\"bbb1\",\"see\":\"a.md\",\"Q\":\"new\",\"A\":\"a\"}\n"
        );
        let notes = Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        let closing = "{\"id\": \"aaa3\", \"Q\": \"q\\n````\"}";
        assert!(edit(&store, &notes, closing, false).is_err());
    }

    #[test]
    fn several_edits_in_one_block_or_one_jsonl_note_each_find_their_item() {
        let mut config = Config::default();
        config.notes.include = vec!["*.md".to_owned(), "*.jsonl".to_owned()];
        let text = "```srl\n@ id:aaa1\nQ: one\nA: a\n\n@ id:aaa2\nQ: two\nA: a\n\n@ id:aaa3\nQ: three\nA: a\n```\n";
        let lines = "{\"id\": \"bbb1\", \"Q\": \"one\", \"A\": \"a\"}\n{\"id\": \"bbb2\", \"Q\": \"two\", \"A\": \"a\"}\n{\"id\": \"bbb3\", \"Q\": \"three\", \"A\": \"a\"}\n";
        let (_tmp, store) = store(&[("a.md", text), ("b.jsonl", lines)]);
        let notes = Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        // The first items given grow by a line, so that a later edit made
        // from the top down would miss its item.
        let input = "{\"id\": \"aaa1\", \"Q\": \"one\\nmore\", \"A\": \"a\"}\n{\"id\": \"bbb1\", \"Q\": \"1\", \"A\": \"a\"}\n{\"id\": \"aaa3\", \"Q\": \"three\\nmore\", \"A\": \"a\"}\n{\"id\": \"bbb3\", \"Q\": \"3\", \"A\": \"a\"}\n";
        edit(&store, &notes, input, false).unwrap();
        assert_eq!(
            read(&store, "a.md"),
            "```srl\n@ id:aaa1\nQ: one\n  more\nA: a\n\n@ id:aaa2\nQ: two\nA: a\n\n@ id:aaa3\nQ: three\n  more\nA: a\n```\n"
        );
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\":\"bbb1\",\"Q\":\"1\",\"A\":\"a\"}\n{\"id\": \"bbb2\", \"Q\": \"two\", \"A\": \"a\"}\n{\"id\":\"bbb3\",\"Q\":\"3\",\"A\":\"a\"}\n"
        );
    }

    #[test]
    fn writes_find_items_by_id_in_the_note_as_it_is_now() {
        let (_tmp, store) = store(&[
            ("a.md", "```srl id:aaa1\nQ: q\nA: a\n```\n"),
            ("b.md", "```srl id:bbb1\nQ: q\nA: a\n```\n"),
        ]);
        let notes = notes(&store);
        let input = "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}\n{\"id\": \"bbb1\", \"Q\": \"new\", \"A\": \"a\"}\n";
        // An item that moved since the notes were read is found where it
        // is now.
        fs::write(
            store.note_file("b.md"),
            "Prose.\n```srl id:bbb1\nQ: q\nA: a\n```\n",
        )
        .unwrap();
        edit(&store, &notes, input, true).unwrap();
        // An item that is gone, or a note that has an error now, stops the
        // command, and no note is written.
        for changed in ["Prose.\n", "```srl id:bbb1\nQ: q\nA: a\n```\n```srl\n"] {
            fs::write(store.note_file("b.md"), changed).unwrap();
            assert!(matches!(
                edit(&store, &notes, input, false),
                Err(Error::ChangedMeanwhile { .. })
            ));
            assert_eq!(read(&store, "a.md"), "```srl id:aaa1\nQ: q\nA: a\n```\n");
        }
        assert!(edit(&store, &notes, "\n\n", false).unwrap().is_empty());
    }

    #[test]
    fn ids_go_to_the_items_without_one_by_their_position() {
        let mut config = Config::default();
        config.notes.include = vec!["*.jsonl".to_owned()];
        // Two items alike in everything, but where they are, are told apart
        // by their position.
        let (_tmp, store) = store(&[(
            "a.jsonl",
            "{\"Q\": \"q\", \"A\": \"a\"}\n{\"Q\": \"q\", \"A\": \"a\"}\n",
        )]);
        let load = || Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        let written = assign_ids(&store, &load(), 7, false).unwrap();
        let lines = written.iter().map(|done| done.line).collect::<Vec<_>>();
        assert_eq!(lines, [1, 2]);
        assert_eq!(load().items().len(), 2);
        assert!(load().problems().is_empty());
    }

    #[test]
    fn add_to_a_jsonl_note_starts_a_new_line_and_keeps_its_line_endings() {
        let mut config = Config::default();
        config.notes.include = vec!["**/*.jsonl".to_owned()];
        let (_tmp, store) = store(&[
            ("a.jsonl", "{\"id\": \"old1\", \"Q\": \"q\", \"A\": \"a\"}"),
            (
                "b.jsonl",
                "{\"id\": \"old2\", \"Q\": \"q\", \"A\": \"a\"}\r\n",
            ),
        ]);
        let load = || Notes::load(&store, &config, &mut Index::open(&store)).unwrap();
        let item = "{\"id\": \"new1\", \"Q\": \"r\", \"A\": \"a\"}\n{\"id\": \"new2\", \"Q\": \"s\", \"A\": \"a\"}";
        let added = add(&store, &config, &load(), "a.jsonl", item, 3, false).unwrap();
        assert_eq!(
            added.text,
            "{\"id\":\"new1\",\"Q\":\"r\",\"A\":\"a\"}\n{\"id\":\"new2\",\"Q\":\"s\",\"A\":\"a\"}\n"
        );
        assert_eq!(
            read(&store, "a.jsonl"),
            format!(
                "{{\"id\": \"old1\", \"Q\": \"q\", \"A\": \"a\"}}\n{}",
                added.text
            )
        );
        let item = "{\"id\": \"new3\", \"Q\": \"t\", \"A\": \"a\"}";
        add(&store, &config, &load(), "b.jsonl", item, 3, false).unwrap();
        assert_eq!(
            read(&store, "b.jsonl"),
            "{\"id\": \"old2\", \"Q\": \"q\", \"A\": \"a\"}\r\n{\"id\":\"new3\",\"Q\":\"t\",\"A\":\"a\"}\r\n"
        );
        let item = "{\"id\": \"new4\", \"Q\": \"u\", \"A\": \"a\"}";
        add(&store, &config, &load(), "c/new.jsonl", item, 3, false).unwrap();
        assert_eq!(
            read(&store, "c/new.jsonl"),
            "{\"id\":\"new4\",\"Q\":\"u\",\"A\":\"a\"}\n"
        );
        assert!(load().problems().is_empty(), "{:?}", load().problems());
    }

    #[test]
    fn edit_checks_every_line_first() {
        let (_tmp, store) = store(&[("a.md", "```srl id:aaa1\nQ: q\nA: a\n```\n")]);
        let notes = notes(&store);
        for input in [
            "{\"Q\": \"q\", \"A\": \"a\"}",
            "{\"id\": \"zzz9\", \"Q\": \"q\", \"A\": \"a\"}",
            "{\"id\": \"aaa1\", \"Q\": \"q\"}",
            "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n{\"id\": \"aaa1\", \"Q\": \"r\", \"A\": \"a\"}",
            "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}\nnot json",
        ] {
            assert!(
                edit(&store, &notes, input, false).is_err(),
                "accepted {input:?}"
            );
        }
        assert_eq!(read(&store, "a.md"), "```srl id:aaa1\nQ: q\nA: a\n```\n");
        edit(
            &store,
            &notes,
            "{\"id\": \"aaa1\", \"Q\": \"new\", \"A\": \"a\"}",
            true,
        )
        .unwrap();
        assert_eq!(read(&store, "a.md"), "```srl id:aaa1\nQ: q\nA: a\n```\n");
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
