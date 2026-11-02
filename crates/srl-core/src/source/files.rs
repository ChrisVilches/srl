//! The source of the store's notes: files matched by `include` and not by
//! `exclude`, each in the item syntax its extension chooses (see
//! `docs/notes.md`).
//!
//! Every change edits lines, never data. Which lines, and what they become,
//! is the business of the note's item syntax ([`Syntax::with_id`],
//! [`Syntax::without_draft`], [`Syntax::replace`], [`Syntax::format`]);
//! this module makes the change safe (see `docs/rust.md`, "Note editing"):
//! the item to change must still be the one read, the new text is parsed
//! again and compared with the old before it is written, the file must not
//! have changed since it was read, and it is replaced atomically.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use globset::GlobSet;

use crate::check::question_suspicions;
use crate::config::Notes;

use crate::note::object::Object;
use crate::note::syntax::{self, Edit, Syntax};
use crate::note::{Entries, Field};
use crate::source::{
    Change, Collection, ContextRequest, Finding, Problem, Read, ReadItem, Refusal, Source,
};
use crate::store::nfc;
use crate::{Error, Result, Store};

/// An item as verification compares it: its entries and fields.
type Keyed = (Entries, Vec<Field>);

/// The notes of a store.
#[derive(Debug)]
pub(super) struct Files {
    store: Store,
    include: GlobSet,
    exclude: GlobSet,
}

impl Files {
    pub(super) fn new(store: &Store, notes: &Notes) -> Self {
        Self {
            store: store.clone(),
            include: notes.include_set(),
            exclude: notes.exclude_set(),
        }
    }

    /// The text of the note at `path`.
    fn text(&self, path: &str) -> Result<String> {
        let file = self.store.note_file(path);
        fs::read_to_string(&file).map_err(Error::io(file))
    }
}

impl Source for Files {
    fn collections(&self) -> Result<Vec<Collection>> {
        Ok(self
            .store
            .note_paths(&self.include, &self.exclude)?
            .into_iter()
            .map(|path| Collection {
                file: Some(path.clone()),
                name: path,
            })
            .collect())
    }

    fn read(&self) -> Result<Read> {
        let mut read = Read::default();
        for path in self.store.note_paths(&self.include, &self.exclude)? {
            let parsed = syntax::of(&path).parse(&self.text(&path)?);
            read.problems
                .extend(parsed.errors.into_iter().map(|(line, reason)| Problem {
                    collection: path.clone(),
                    line,
                    reason,
                }));
            read.items.extend(
                parsed
                    .items
                    .into_iter()
                    .enumerate()
                    .map(|(position, item)| ReadItem {
                        collection: path.clone(),
                        handle: position.to_string(),
                        line: item.line,
                        written: item.text,
                        item: Object {
                            entries: item.entries,
                            fields: item.fields,
                        },
                    }),
            );
        }
        Ok(read)
    }

    /// What a note shows as context is its syntax's business: to its own
    /// items, and to the items that name it with `see`.
    fn context(&self, requests: &[ContextRequest<'_>]) -> Result<Vec<Option<String>>> {
        requests
            .iter()
            .map(|request| {
                let syntax = syntax::of(request.collection);
                let text = self.text(request.collection)?;
                Ok(if request.own {
                    syntax.own_context(&text, request.mode)
                } else {
                    syntax.context(&text, request.mode)
                })
            })
            .collect()
    }

    /// A new note needs a path that `include` matches and `exclude`
    /// doesn't (see `docs/cli.md`, "Adding items").
    fn refuses(&self, collection: &str) -> Option<String> {
        if !self.include.is_match(nfc(collection)) {
            Some("the path is not matched by `include` in .srl/config.toml".to_owned())
        } else if self.exclude.is_match(nfc(collection)) {
            Some("the path is excluded by `exclude` in .srl/config.toml".to_owned())
        } else {
            None
        }
    }

    /// Every note is changed and checked before the first one is written.
    fn write(&self, changes: &[Change], dry_run: bool) -> std::result::Result<String, Refusal> {
        let mut by_note = BTreeMap::<&str, Vec<(usize, &Change)>>::new();
        for (at, change) in changes.iter().enumerate() {
            let (Change::Add { collection, .. } | Change::Replace { collection, .. }) = change;
            by_note.entry(collection).or_default().push((at, change));
        }
        let mut appended = String::new();
        let mut written = Vec::new();
        for (path, changes) in by_note {
            let (old, new, added) = self.changed(path, &changes)?;
            appended.push_str(&added);
            written.push((path, old, new));
        }
        if !dry_run {
            for (path, old, new) in written {
                replace_file(&self.store, path, old.as_deref(), &new)?;
            }
        }
        Ok(appended)
    }

    /// The likely mistakes in every note, in the order of the notes and
    /// their lines: its syntax's own (see [`Syntax::lint`]), then the `qa`
    /// kind's, in the `Q` of its items, where its syntax says `Q` is (see
    /// [`Syntax::field_lines`]).
    fn lint(&self) -> Result<Vec<Finding>> {
        let mut found = Vec::new();
        for path in self.store.note_paths(&self.include, &self.exclude)? {
            let text = self.text(&path)?;
            found.extend(
                lint_text(&path, &text)
                    .into_iter()
                    .map(|(line, message)| Finding {
                        collection: path.clone(),
                        line,
                        message,
                    }),
            );
        }
        Ok(found)
    }
}

impl Files {
    /// The note at `path` as it is now, if it exists, with `changes` made
    /// to it and checked, and the text of the items added. Each change
    /// comes with its place among all the changes.
    fn changed(
        &self,
        path: &str,
        changes: &[(usize, &Change)],
    ) -> std::result::Result<(Option<String>, String, String), Refusal> {
        let syntax = syntax::of(path);
        let old = read_if_exists(&self.store.note_file(path))?;
        let parsed = syntax.parse(old.as_deref().unwrap_or_default());
        // Commands that write refuse notes with errors, so a note that has
        // some now changed since it was read.
        if !parsed.errors.is_empty() {
            return Err(changed_meanwhile(path));
        }
        let mut expected = parsed
            .items
            .into_iter()
            .map(|item| (item.entries, item.fields))
            .collect::<Vec<_>>();
        let mut text = old.clone().unwrap_or_default();
        let mut added = Vec::new();
        for &(at, change) in changes {
            let refused = |reason: String| Refusal::Change { at, reason };
            match change {
                Change::Replace {
                    handle,
                    before,
                    after,
                    ..
                } => {
                    if old.is_none() {
                        return Err(changed_meanwhile(path));
                    }
                    let position = position_of(&expected, handle, before, path)?;
                    let (edit, item) =
                        replacement(syntax, &text, position, before, after).map_err(refused)?;
                    if let Some(edit) = edit {
                        text = apply(&text, &edit);
                    }
                    let slot = expected
                        .get_mut(position)
                        .expect("the position of an item should be among the note's items");
                    *slot = item;
                }
                Change::Add { item, .. } => {
                    let formatted = syntax
                        .format(&item.entries, &item.fields)
                        .map_err(refused)?;
                    expected.extend(keys(syntax, &formatted));
                    added.push(formatted);
                }
            }
        }
        let added = if added.is_empty() {
            String::new()
        } else {
            format!("{}\n", added.join(syntax.gap()))
        };
        let new = append(syntax, &text, &added);
        verify(path, &new, &expected)?;
        Ok((old, new, added))
    }
}

/// The position among `items`, the items of the note at `path` as it is
/// now, of the item named by `handle` and read as `before`: by its ID if it
/// had one, else by its position. An item that is not there any more, or
/// not as it was read, changed since the note was read.
fn position_of(
    items: &[Keyed],
    handle: &str,
    before: &Object,
    path: &str,
) -> std::result::Result<usize, Refusal> {
    let position = match &before.entries.id {
        Some(id) => items
            .iter()
            .position(|(entries, _)| entries.id.as_ref() == Some(id)),
        None => handle.parse().ok(),
    };
    position
        .filter(|position| {
            items.get(*position).is_some_and(|(entries, fields)| {
                *entries == before.entries && *fields == before.fields
            })
        })
        .ok_or_else(|| changed_meanwhile(path))
}

/// The edit that turns item `position` of `text`, read as `before`, into
/// `after`, if anything changes, and the item as it reads after the edit. A
/// change of `id` or `draft` alone changes the header only; any other
/// change replaces the item. Errors are plain reasons.
fn replacement(
    syntax: &dyn Syntax,
    text: &str,
    position: usize,
    before: &Object,
    after: &Object,
) -> std::result::Result<(Option<Edit>, Keyed), String> {
    let unchanged = (after.entries.clone(), after.fields.clone());
    if before == after {
        return Ok((None, unchanged));
    }
    let only = |entries: Entries| before.fields == after.fields && entries == after.entries;
    let undrafted = Entries {
        draft: false,
        ..before.entries.clone()
    };
    let header = match (&before.entries.id, &after.entries.id) {
        (None, Some(id))
            if only(Entries {
                id: Some(id.clone()),
                ..before.entries.clone()
            }) =>
        {
            Some(syntax.with_id(text, position, id))
        }
        _ if before.entries.draft && only(undrafted) => Some(syntax.without_draft(text, position)),
        _ => None,
    };
    let Some(header) = header else {
        let edit = syntax.replace(text, position, &after.entries, &after.fields)?;
        let formatted = syntax.format(&after.entries, &after.fields)?;
        let item = keys(syntax, &formatted).into_iter().next().ok_or_else(|| {
            "it doesn't read back as an item (a bug in srl; please report it)".to_owned()
        })?;
        return Ok((Some(edit), item));
    };
    let edit = header.ok_or_else(|| {
        "the item is not where srl read it (a bug in srl; please report it)".to_owned()
    })?;
    Ok((Some(edit), unchanged))
}

/// `text` with `added` appended: a gap of the syntax between them, and the
/// line endings of the note.
fn append(syntax: &dyn Syntax, text: &str, added: &str) -> String {
    if added.is_empty() {
        return text.to_owned();
    }
    let ending = line_ending(text);
    let gap = syntax.gap();
    let tail = added.replace('\n', ending);
    if text.is_empty() {
        return tail;
    }
    // The note's last line ending is the first of the gap.
    let separator = if text.ends_with('\n') {
        gap.get(1..).unwrap_or_default()
    } else {
        gap
    };
    format!("{text}{}{tail}", separator.replace('\n', ending))
}

fn changed_meanwhile(path: &str) -> Refusal {
    Refusal::Error(Error::ChangedMeanwhile {
        path: path.to_owned(),
    })
}

/// What verification compares: each item's entries and fields, in order.
fn keys(syntax: &dyn Syntax, text: &str) -> Vec<Keyed> {
    syntax
        .parse(text)
        .items
        .into_iter()
        .map(|item| (item.entries, item.fields))
        .collect()
}

/// Checks that `new` parses without errors into exactly the `expected`
/// items. A mismatch is a bug in `srl`, so nothing is written.
fn verify(path: &str, new: &str, expected: &[Keyed]) -> Result<()> {
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

/// The text of `file`, or `None` if it doesn't exist.
fn read_if_exists(file: &Path) -> Result<Option<String>> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io(file)(error)),
    }
}

/// Writes `new` to the note at `path`, if the file still holds `old` (or,
/// with `None`, still doesn't exist): to a temporary file in `.srl/tmp/`,
/// renamed over the note. The rename fails if the note's folder is on
/// another file system than `.srl/`.
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
    let tmp = store.tmp_dir();
    fs::create_dir_all(&tmp).map_err(Error::io(&tmp))?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(&tmp).map_err(|error| cannot(error.to_string()))?;
    temporary
        .write_all(new.as_bytes())
        .map_err(|error| cannot(error.to_string()))?;
    temporary
        .persist(&file)
        .map_err(|error| cannot(error.error.to_string()))?;
    Ok(())
}

/// The suspicious lines of the note at `path`, whose text is `text`, with
/// their numbers (from 1), in order: its syntax's own (see
/// [`Syntax::lint`]), then the `qa` kind's, in the `Q` of its items, where
/// its syntax says `Q` is (see [`Syntax::field_lines`]).
fn lint_text(path: &str, text: &str) -> Vec<(usize, String)> {
    let syntax = syntax::of(path);
    let mut found = syntax.lint(text);
    for item in syntax.parse(text).items {
        if item
            .entries
            .kind
            .as_deref()
            .is_some_and(|kind| kind != "qa")
        {
            continue;
        }
        if let Some(question) = item.fields.iter().position(|field| field.key == "Q") {
            found.extend(question_suspicions(
                syntax.field_lines(text, &item, question),
            ));
        }
    }
    found.sort_by_key(|(line, _)| *line);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found_lines(text: &str) -> Vec<usize> {
        lint_text("n.md", text)
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    #[test]
    fn mistaken_deletions_in_questions_are_found() {
        let text = "```srl\n@ id:abc\nQ: {{C1::upper}}\n  {{c1:one colon}}\n  {{c1::Paris::Paris}}\nA: {{C1::in the answer}}\n@ id:abd kind:words\nQ: {{C1::other kind}}\n```\n";
        assert_eq!(found_lines(text), [3, 4, 5]);
    }

    #[test]
    fn the_syntax_and_the_kind_are_both_linted_in_line_order() {
        let text = "Q: bare\n\n```srl id:abc\nQ: {{C1::upper}}\n```\n\n- ```srl\n";
        assert_eq!(found_lines(text), [1, 4, 7]);
    }

    #[test]
    fn a_note_changed_meanwhile_is_not_written() {
        let tmp = tempfile::TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&crate::Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..crate::Lookup::default()
        })
        .unwrap();
        fs::write(store.note_file("a.md"), "old").unwrap();
        assert!(matches!(
            replace_file(&store, "a.md", Some("something else"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert!(matches!(
            replace_file(&store, "b.md", Some("x"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert_eq!(fs::read_to_string(store.note_file("a.md")).unwrap(), "old");
    }

    #[test]
    fn in_a_jsonl_note_only_questions_are_linted_at_their_items_line() {
        let text = "{\"id\": \"abc\", \"Q\": \"{{c1::fine}}\"}\n\n{\"id\": \"abd\", \"Q\": \"{{C1::upper}}\", \"A\": \"Q: a\"}\n";
        let found = lint_text("n.jsonl", text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 3);
    }
}
