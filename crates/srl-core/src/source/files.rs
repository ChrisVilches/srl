//! What the sources whose collections are files of the store share,
//! without knowing any format: which files they read, reading them as
//! lines, and writing them safely (see `docs/rust.md`, "Note editing"): a
//! file must not have changed since it was read, and it is replaced
//! atomically, through `.srl/tmp/`.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use globset::GlobSet;

use crate::domain::note::object::Object;
use crate::domain::note::{Entries, Field};
use crate::source::{Change, Collection, Problem, Read, ReadItem, Refusal};
use crate::store::nfc;
use crate::{Error, Result, Store};

/// The files of a store a source reads: those `include` matches, `exclude`
/// doesn't, and `only` matches too when it is given.
#[derive(Debug)]
pub(super) struct NoteFiles {
    store: Store,
    include: GlobSet,
    exclude: GlobSet,
    only: Option<GlobSet>,
}

impl NoteFiles {
    pub(super) fn new(
        store: &Store,
        include: GlobSet,
        exclude: GlobSet,
        only: Option<GlobSet>,
    ) -> Self {
        Self {
            store: store.clone(),
            include,
            exclude,
            only,
        }
    }

    /// The paths of the files, in order (see [`Store::note_paths`]).
    pub(super) fn paths(&self) -> Result<Vec<String>> {
        let mut paths = self.store.note_paths(&self.include, &self.exclude)?;
        if let Some(only) = &self.only {
            paths.retain(|path| only.is_match(nfc(path)));
        }
        Ok(paths)
    }

    /// The files as collections: each named by its path.
    pub(super) fn collections(&self) -> Result<Vec<Collection>> {
        Ok(self
            .paths()?
            .into_iter()
            .map(|path| Collection {
                file: Some(path.clone()),
                name: path,
            })
            .collect())
    }

    /// The text of the file at `path`.
    pub(super) fn text(&self, path: &str) -> Result<String> {
        let file = self.store.note_file(path);
        fs::read_to_string(&file).map_err(Error::io(file))
    }

    /// The text of the file at `path`, or `None` if it doesn't exist.
    pub(super) fn text_if_exists(&self, path: &str) -> Result<Option<String>> {
        read_if_exists(&self.store.note_file(path))
    }

    /// Why a new file can't be at `path`: `include` doesn't match it, or
    /// `exclude` does (see `docs/cli.md`, "Adding items").
    pub(super) fn refuses(&self, path: &str) -> Option<String> {
        let path = nfc(path);
        if !self.include.is_match(&path) {
            Some("the path is not matched by `include` in .srl/config.toml".to_owned())
        } else if self.exclude.is_match(&path)
            || self.only.as_ref().is_some_and(|only| !only.is_match(&path))
        {
            Some("the path is excluded by `exclude` in .srl/config.toml".to_owned())
        } else {
            None
        }
    }

    /// Writes each of `written`, a file's path, its text when it was read
    /// (`None` if it didn't exist) and its new text, unless `dry_run`; see
    /// [`replace_file`].
    pub(super) fn write_all(
        &self,
        written: Vec<(&str, Option<String>, String)>,
        dry_run: bool,
    ) -> Result<()> {
        if !dry_run {
            for (path, old, new) in written {
                replace_file(&self.store, path, old.as_deref(), &new)?;
            }
        }
        Ok(())
    }
}

/// The lines of `text`, without their line endings (`\n` or `\r\n`) and
/// without a UTF-8 byte order mark. A final line ending doesn't start an
/// empty line. Lines are numbered from 1 in this order everywhere.
pub(crate) fn lines(text: &str) -> Vec<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

/// Where something is: from line `first` to line `last`, counting from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lines {
    pub(crate) first: usize,
    pub(crate) last: usize,
}

/// An item as a file's format reads it, before its kind turns it into
/// cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedItem {
    pub(crate) entries: Entries,
    pub(crate) fields: Vec<Field>,
    /// The line that names the item in messages and in `ADDRESS:LINE`:
    /// where it starts, as near as the format can say. Only for people.
    pub(crate) line: usize,
    /// Where each field is written, in the order of `fields`, for `srl
    /// lint`.
    pub(crate) spans: Vec<Lines>,
    /// The item written on its own, lines joined with `\n`: what `srl show`
    /// prints.
    pub(crate) text: String,
}

/// A file read for items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    /// The items, in the order they are written: an item's position here
    /// is its handle.
    pub(crate) items: Vec<ParsedItem>,
    /// Each problem, with its line.
    pub(crate) errors: Vec<(usize, String)>,
}

/// An item as verification compares it: its entries and fields.
pub(super) type Keyed = (Entries, Vec<Field>);

/// The items of `parsed`, as verification compares them.
pub(super) fn keyed(parsed: Parsed) -> Vec<Keyed> {
    parsed
        .items
        .into_iter()
        .map(|item| (item.entries, item.fields))
        .collect()
}

/// A change to a file: lines `first` to `last` (counting from 1) replaced
/// by `lines`. The line endings and a byte order mark are left to
/// [`apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    pub(crate) first: usize,
    pub(crate) last: usize,
    pub(crate) lines: Vec<String>,
}

impl Edit {
    /// The edit that replaces line `line` with `content`.
    pub(crate) fn one(line: usize, content: String) -> Self {
        Self {
            first: line,
            last: line,
            lines: vec![content],
        }
    }
}

/// `text`, unless it holds nothing but blank space, which is no context
/// (see `docs/notes.md`, "Context").
pub(crate) fn unless_blank(text: String) -> Option<String> {
    (!text.trim().is_empty()).then_some(text)
}

/// The position among `items`, the items of the file at `path` as it is
/// now, of the item named by `handle` and read as `before`: by its ID if it
/// had one, else by its position. An item that is not there any more, or
/// not as it was read, changed since the file was read.
pub(super) fn position_of(
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

/// The refusal for a file that changed since it was read.
pub(super) fn changed_meanwhile(path: &str) -> Refusal {
    Refusal::Error(Error::ChangedMeanwhile {
        path: path.to_owned(),
    })
}

/// Fails unless the changed file at `path` read back as intended: a
/// mismatch is a bug in `srl`, so nothing is written.
pub(super) fn verified(path: &str, as_intended: bool) -> Result<()> {
    if as_intended {
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

/// `text` with `added` appended after `gap`, in the line endings of
/// `text`.
pub(super) fn append(text: &str, added: &str, gap: &str) -> String {
    if added.is_empty() {
        return text.to_owned();
    }
    let ending = line_ending(text);
    let tail = added.replace('\n', ending);
    if text.is_empty() {
        return tail;
    }
    // The file's last line ending is the first of the gap.
    let separator = if text.ends_with('\n') {
        gap.get(1..).unwrap_or_default()
    } else {
        gap
    };
    format!("{text}{}{tail}", separator.replace('\n', ending))
}

/// The line ending of a file: that of its first line, or `\n`.
fn line_ending(text: &str) -> &'static str {
    match text.split_once('\n') {
        Some((first, _)) if first.ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

/// `text` with `edit` made: its lines joined with the file's line ending,
/// the last one getting the ending of the last line replaced, and a byte
/// order mark kept.
pub(super) fn apply(text: &str, edit: &Edit) -> String {
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

/// Writes `new` to the file at `path`, if it still holds `old` (or, with
/// `None`, still doesn't exist): to a temporary file in `.srl/tmp/`,
/// renamed over the file. The rename fails if the file's folder is on
/// another file system than `.srl/`.
///
// TODO(platform): Not run on Windows yet. Windows refuses to rename over a
// file that another program holds open without sharing it; the error is
// then reported and the file left as it was. See docs/notes.md, "How srl
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

/// Adds the items and problems of `parsed`, the note at `path`, to `read`.
pub(super) fn add_read(read: &mut Read, path: &str, parsed: Parsed) {
    read.problems
        .extend(parsed.errors.into_iter().map(|(line, reason)| Problem {
            collection: path.to_owned(),
            line,
            reason,
        }));
    read.items.extend(
        parsed
            .items
            .into_iter()
            .enumerate()
            .map(|(position, item)| ReadItem {
                collection: path.to_owned(),
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

/// `changes` by the collection they are about, each with its place.
pub(super) fn by_collection(changes: &[Change]) -> BTreeMap<&str, Vec<(usize, &Change)>> {
    let mut by_note = BTreeMap::<&str, Vec<(usize, &Change)>>::new();
    for (at, change) in changes.iter().enumerate() {
        let (Change::Add { collection, .. } | Change::Replace { collection, .. }) = change;
        by_note.entry(collection).or_default().push((at, change));
    }
    by_note
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_changed_meanwhile_is_not_written() {
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
}
