//! The `filesystem` source: collections that are files of the store, each
//! in the format its extension names (see `docs/formats.md`). A file holds
//! nothing but items; the study material items name with `see` is not
//! read here (see `docs/items.md`, "Context").

mod json;
mod jsonl;
mod xml;

use std::path::Path;

use crate::Store;
use crate::source::files::{CollectionFiles, Parsed, add_read, by_collection};
use crate::source::{Change, Description, Read, Refusal, Source};

/// A format of collection files, named by the files' extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// One item object per line (see `docs/formats.md`, "JSONL").
    Jsonl,
    /// One array of item objects (see `docs/formats.md`, "JSON").
    Json,
    /// An `<items>` element of `<item>` elements (see `docs/formats.md`,
    /// "XML").
    Xml,
}

/// The extensions of the formats, as `srl` names them in messages.
const EXTENSIONS: &str = ".jsonl, .json and .xml";

impl Format {
    /// The format of the file at `path`, by its extension.
    fn of(path: &str) -> Option<Self> {
        match Path::new(path).extension()?.to_str()? {
            "jsonl" => Some(Self::Jsonl),
            "json" => Some(Self::Json),
            "xml" => Some(Self::Xml),
            _ => None,
        }
    }

    fn parse(self, text: &str) -> Parsed {
        match self {
            Self::Jsonl => jsonl::parse(text),
            Self::Json => json::parse(text),
            Self::Xml => xml::parse(text),
        }
    }

    /// The text of a file of this format at `path`, `old` (`None` if it
    /// doesn't exist yet), with `changes` made to it and checked, and the
    /// text of the items added. Each change comes with its place among all
    /// the changes.
    fn changed(
        self,
        path: &str,
        old: Option<&str>,
        changes: &[(usize, &Change)],
    ) -> Result<(String, String), Refusal> {
        match self {
            Self::Jsonl => jsonl::changed(path, old, changes),
            Self::Json => json::changed(path, old, changes),
            Self::Xml => xml::changed(path, old, changes),
        }
    }
}

/// The items of the file at `path` whose text is `text`, read in the
/// format its extension names.
pub(super) fn parse(path: &str, text: &str) -> Parsed {
    match Format::of(path) {
        Some(format) => format.parse(text),
        None => Parsed {
            items: Vec::new(),
            errors: vec![unknown_format()],
        },
    }
}

/// Why a file is no collection of any format.
fn unknown_format() -> String {
    format!(
        "srl reads collections in the formats {EXTENSIONS}, by their extension; leave other \
         files out of the source with `include` and `exclude`"
    )
}

/// The files of a `filesystem` source.
#[derive(Debug)]
pub(super) struct Filesystem {
    files: CollectionFiles,
}

impl Filesystem {
    pub(super) fn new(store: &Store, include: &[String], exclude: &[String]) -> Self {
        Self {
            files: CollectionFiles::new(store, include, exclude),
        }
    }
}

impl Source for Filesystem {
    fn collections(&self) -> crate::Result<Vec<String>> {
        self.files.paths()
    }

    fn read(&self) -> crate::Result<Read> {
        let mut read = Read::default();
        for path in self.files.paths()? {
            let parsed = match self.files.text(&path)? {
                Some(text) => parse(&path, &text),
                None => Parsed {
                    items: Vec::new(),
                    errors: vec!["it is not UTF-8, as every collection must be".to_owned()],
                },
            };
            add_read(&mut read, &path, parsed);
        }
        Ok(read)
    }

    fn describe(&self) -> crate::Result<Description> {
        Ok(self.files.describe(&format!(
            "Collections that are files of the store, in the formats {EXTENSIONS}, holding \
             nothing but items."
        )))
    }

    fn refuses(&self, collection: &str) -> crate::Result<Option<String>> {
        Ok(self.files.refuses(collection).or_else(|| {
            Format::of(collection)
                .is_none()
                .then(|| format!("its extension is none of {EXTENSIONS}"))
        }))
    }

    /// Every file is changed and checked before the first one is written.
    fn write(&self, changes: &[Change], dry_run: bool) -> Result<String, Refusal> {
        let mut appended = String::new();
        let mut written = Vec::new();
        for (path, changes) in by_collection(changes) {
            let format = Format::of(path).ok_or_else(|| Refusal::Change {
                at: changes.first().map_or(0, |(at, _)| *at),
                reason: unknown_format(),
            })?;
            let old = self.files.text_if_exists(path)?;
            let (new, added) = format.changed(path, old.as_deref(), &changes)?;
            appended.push_str(&added);
            written.push((path, old, new));
        }
        self.files.write_all(written, dry_run)?;
        Ok(appended)
    }
}
