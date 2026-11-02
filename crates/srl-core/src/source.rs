//! The source port: where items come from, and how they are written back
//! (see `docs/plans/sources.md`, "The Source port").
//!
//! A source is read whole, into the catalog, and everything else in `srl`
//! works on the catalog. A source is asked again only to show a context, to
//! change items and to look for likely mistakes. Nothing outside a source's
//! own module knows how it keeps its items: a source names its collections
//! and its items, and `srl` names them back to it.

pub(crate) mod files;
mod jsonl;
mod markdown;

use std::fmt;

use crate::config::globs;
use crate::domain::context::ContextMode;
use crate::domain::note::object::Object;
use crate::{Config, Error, Result, Store};
use files::NoteFiles;
use jsonl::Jsonl;
use markdown::Markdown;

/// The glob of the files of the `jsonl` source: those whose name ends with
/// `.jsonl` (see `docs/notes.md`, "Item syntaxes").
const JSONL: &str = "**/*.jsonl";

/// The operations every source answers.
pub(crate) trait Source: fmt::Debug {
    /// Every collection of the source, including those without items, in
    /// order.
    fn collections(&self) -> Result<Vec<Collection>>;

    /// Every item of the source, and every problem found in it. Only
    /// failing to read the source at all is an error.
    fn read(&self) -> Result<Read>;

    /// What each requested collection shows as context, in order: `None`
    /// when there is nothing to show. Never asked for mode `none`.
    fn context(&self, requests: &[ContextRequest<'_>]) -> Result<Vec<Option<String>>>;

    /// Why `collection`, which doesn't exist yet, can't be created by `srl
    /// add`, or `None` if it can.
    fn refuses(&self, collection: &str) -> Option<String>;

    /// Makes `changes`, or, with `dry_run`, only checks them. Gives the
    /// text of the items added, as they appear in their collections.
    fn write(&self, changes: &[Change], dry_run: bool) -> std::result::Result<String, Refusal>;

    /// The likely mistakes in the source (see `docs/cli.md`, "Checking").
    fn lint(&self) -> Result<Vec<Finding>>;
}

/// A collection of a source: a group of items, named by the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Collection {
    pub(crate) name: String,
    /// The file it is kept in, relative to the store's folder, if it is a
    /// file of its own.
    pub(crate) file: Option<String>,
}

/// What a source read: its items, and the problems it found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Read {
    pub(crate) items: Vec<ReadItem>,
    pub(crate) problems: Vec<Problem>,
}

/// An item as its source read it, before anything is checked but its
/// syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadItem {
    pub(crate) collection: String,
    /// What names the item to its source when it is changed.
    pub(crate) handle: String,
    /// The line where the item starts, only for people.
    pub(crate) line: usize,
    /// The item as written, for `srl show`.
    pub(crate) written: String,
    pub(crate) item: Object,
}

/// A problem a source found in a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) collection: String,
    pub(crate) line: usize,
    pub(crate) reason: String,
}

/// A likely mistake a source found in a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) collection: String,
    pub(crate) line: usize,
    pub(crate) message: String,
}

/// The context of a collection, wanted in `mode`, for its own items or
/// for items that name it with `see`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContextRequest<'a> {
    pub(crate) collection: &'a str,
    pub(crate) mode: ContextMode,
    pub(crate) own: bool,
}

/// A change to the items of a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    /// Appends `item` to `collection`, creating it if it doesn't exist.
    Add { collection: String, item: Object },
    /// Replaces the item of `collection` named by `handle`, which was read
    /// as `before`, with `after`. Refused if the item is no longer
    /// `before`.
    Replace {
        collection: String,
        handle: String,
        before: Object,
        after: Object,
    },
}

/// Why a source didn't make its changes: one of them, by its place, can't
/// be written, or anything else went wrong. Either way, nothing was
/// written.
#[derive(Debug)]
pub(crate) enum Refusal {
    Change { at: usize, reason: String },
    Error(Error),
}

impl From<Error> for Refusal {
    fn from(error: Error) -> Self {
        Self::Error(error)
    }
}

/// Every source of a store, each mounted at its prefix.
#[derive(Debug)]
pub(crate) struct Sources {
    mounted: Vec<Mounted>,
}

/// A source, with its prefix.
#[derive(Debug)]
pub(crate) struct Mounted {
    pub(crate) prefix: String,
    pub(crate) source: Box<dyn Source>,
}

impl Mounted {
    /// The address of `collection` of this source.
    pub(crate) fn address(&self, collection: &str) -> String {
        format!("{}{collection}", self.prefix)
    }
}

impl Sources {
    /// The sources of `store`, as `config` sets them: the notes of
    /// `[notes]`, those whose name ends with `.jsonl` in a `jsonl` source,
    /// and the others in a `markdown` source.
    pub(crate) fn new(store: &Store, config: &Config) -> Self {
        let notes = &config.notes;
        let jsonl = || globs(&[JSONL]).expect("the glob of JSONL notes should be valid");
        let not_jsonl = globs(
            &notes
                .exclude
                .iter()
                .map(String::as_str)
                .chain([JSONL])
                .collect::<Vec<_>>(),
        )
        .expect("the exclude globs should have been checked when read");
        let markdown = NoteFiles::new(store, notes.include_set(), not_jsonl, None);
        let jsonl = NoteFiles::new(
            store,
            notes.include_set(),
            notes.exclude_set(),
            Some(jsonl()),
        );
        Self {
            mounted: vec![
                Mounted {
                    prefix: String::new(),
                    source: Box::new(Markdown::new(markdown)),
                },
                Mounted {
                    prefix: String::new(),
                    source: Box::new(Jsonl::new(jsonl)),
                },
            ],
        }
    }

    /// The sources, in order; an item names its source by its place here.
    pub(crate) fn all(&self) -> &[Mounted] {
        &self.mounted
    }

    /// The source that `address`, of a collection that doesn't exist yet,
    /// would be created in, with the collection's name in it; or why it
    /// can't be created. Of the sources mounted deepest above the address,
    /// exactly one must accept it (see `docs/plans/sources.md`,
    /// "Addresses").
    pub(crate) fn for_new(&self, address: &str) -> std::result::Result<(usize, String), String> {
        let deepest = self
            .mounted
            .iter()
            .filter(|mounted| address.starts_with(&mounted.prefix))
            .map(|mounted| mounted.prefix.len())
            .max()
            .ok_or_else(|| "no source of the store holds it".to_owned())?;
        let collection = address.get(deepest..).unwrap_or_default();
        let answers = self
            .mounted
            .iter()
            .enumerate()
            .filter(|(_, mounted)| {
                mounted.prefix.len() == deepest && address.starts_with(&mounted.prefix)
            })
            .map(|(index, mounted)| (index, mounted.source.refuses(collection)))
            .collect::<Vec<_>>();
        let accepting = answers
            .iter()
            .filter(|(_, refusal)| refusal.is_none())
            .map(|(index, _)| *index)
            .collect::<Vec<_>>();
        match accepting.as_slice() {
            [index] => Ok((*index, collection.to_owned())),
            [] => Err(answers
                .into_iter()
                .find_map(|(_, refusal)| refusal)
                .expect("a source that accepts nothing should say why")),
            _ => Err("more than one source of the store would hold it".to_owned()),
        }
    }
}

/// The items of a note at `path` whose text is `text`, read by the source
/// its extension chooses: for tests that need items without a store.
#[cfg(test)]
pub(crate) fn parse(path: &str, text: &str) -> files::Parsed {
    if std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext == "jsonl")
    {
        jsonl::parse(text)
    } else {
        markdown::parse(text)
    }
}
