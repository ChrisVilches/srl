//! What the formats of the `filesystem` source share, without knowing any
//! of them: which files it reads, reading them as lines, and writing them
//! safely (see `docs/rust.md`, "Collection editing"): a file must not have
//! changed since it was read, and it is replaced atomically, through
//! `.srl/tmp/`.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use globset::GlobSet;

use crate::config::globs;
use crate::domain::item::object::Object;
use crate::domain::item::{Entries, Field};
use crate::source::{Change, Description, Problem, Read, ReadItem, Refusal};
use crate::store::nfc;
use crate::{Error, Result, Store};

/// The files of a store a source reads: those `include` matches and
/// `exclude` doesn't.
#[derive(Debug)]
pub(super) struct CollectionFiles {
    store: Store,
    include: GlobSet,
    exclude: GlobSet,
    /// The globs as given, for [`CollectionFiles::describe`].
    globs: (Vec<String>, Vec<String>),
}

impl CollectionFiles {
    /// The files `include` matches and `exclude` doesn't. The globs were
    /// checked when the configuration was read.
    pub(super) fn new(store: &Store, include: &[String], exclude: &[String]) -> Self {
        Self {
            store: store.clone(),
            include: globs(include).expect("the include globs should have been checked when read"),
            exclude: globs(exclude).expect("the exclude globs should have been checked when read"),
            globs: (include.to_vec(), exclude.to_vec()),
        }
    }

    /// The source, described by `description`, with the files it reads as
    /// the collections `srl add` may create.
    pub(super) fn describe(&self, description: &str) -> Description {
        Description {
            summary: description.to_owned(),
            include: self.globs.0.clone(),
            exclude: self.globs.1.clone(),
        }
    }

    /// The paths of the files, in order (see [`Store::files`]).
    pub(super) fn paths(&self) -> Result<Vec<String>> {
        self.store.files(&self.include, &self.exclude)
    }

    /// The text of the file at `path`.
    pub(super) fn text(&self, path: &str) -> Result<String> {
        let file = self.store.file(path);
        fs::read_to_string(&file).map_err(Error::io(file))
    }

    /// The text of the file at `path`, or `None` if it doesn't exist.
    pub(super) fn text_if_exists(&self, path: &str) -> Result<Option<String>> {
        read_if_exists(&self.store.file(path))
    }

    /// Why a new file can't be at `path`: `include` doesn't match it, or
    /// `exclude` does (see `docs/cli.md`, "Adding items").
    pub(super) fn refuses(&self, path: &str) -> Option<String> {
        let path = nfc(path);
        if !self.include.is_match(&path) {
            Some("its `include` doesn't match the path".to_owned())
        } else if self.exclude.is_match(&path) {
            Some("its `exclude` matches the path".to_owned())
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

/// An item as a file's format reads it, before its kind turns it into
/// cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedItem {
    pub(crate) entries: Entries,
    pub(crate) fields: Vec<Field>,
}

/// A file read for items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    /// The items, in the order they are written: an item's position here
    /// is its handle.
    pub(crate) items: Vec<ParsedItem>,
    /// Each problem, saying where in the file it is when the format can.
    pub(crate) errors: Vec<String>,
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
            reason: "the changed collection did not read back as intended (a bug in srl; please \
                     report it)"
                .to_owned(),
        })
    }
}

/// The line ending of a file: that of its first line, or `\n`.
pub(super) fn line_ending(text: &str) -> &'static str {
    match text.split_once('\n') {
        Some((first, _)) if first.ends_with('\r') => "\r\n",
        _ => "\n",
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

/// Writes `new` to the file at `path`, if it still holds `old` (or, with
/// `None`, still doesn't exist): to a temporary file in `.srl/tmp/`,
/// renamed over the file. The rename fails if the file's folder is on
/// another file system than `.srl/`.
///
// TODO(platform): Not run on Windows yet. Windows refuses to rename over a
// file that another program holds open without sharing it; the error is
// then reported and the file left as it was. See docs/formats.md, "How srl
// writes to collections", and docs/rust.md, "Platform independence".
fn replace_file(store: &Store, path: &str, old: Option<&str>, new: &str) -> Result<()> {
    let file = store.file(path);
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

/// Adds the items and problems of `parsed`, the collection at `path`, to
/// `read`.
pub(super) fn add_read(read: &mut Read, path: &str, parsed: Parsed) {
    read.problems
        .extend(parsed.errors.into_iter().map(|reason| Problem {
            collection: path.to_owned(),
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
                item: Object {
                    entries: item.entries,
                    fields: item.fields,
                },
            }),
    );
}

/// `changes` by the collection they are about, each with its place.
pub(super) fn by_collection(changes: &[Change]) -> BTreeMap<&str, Vec<(usize, &Change)>> {
    let mut by_file = BTreeMap::<&str, Vec<(usize, &Change)>>::new();
    for (at, change) in changes.iter().enumerate() {
        let (Change::Add { collection, .. } | Change::Replace { collection, .. }) = change;
        by_file.entry(collection).or_default().push((at, change));
    }
    by_file
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
        fs::write(store.file("a.jsonl"), "old").unwrap();
        assert!(matches!(
            replace_file(&store, "a.jsonl", Some("something else"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert!(matches!(
            replace_file(&store, "b.jsonl", Some("x"), "new"),
            Err(Error::ChangedMeanwhile { .. })
        ));
        assert_eq!(fs::read_to_string(store.file("a.jsonl")).unwrap(), "old");
    }
}
