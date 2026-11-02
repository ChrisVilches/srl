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
mod program;

use std::fmt;

use crate::config::Driver;
use crate::domain::context::ContextMode;
use crate::domain::note::object::Object;
use crate::{Config, Error, Result, Store};
use files::NoteFiles;
use jsonl::Jsonl;
use markdown::Markdown;
use program::Program;

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

    /// What the source is, for `srl sources`.
    fn describe(&self) -> Result<Description>;
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
    /// The line where the item starts, if the source can say: only for
    /// people.
    pub(crate) line: Option<usize>,
    /// The item as written, for `srl show`, if the source gives it.
    pub(crate) written: Option<String>,
    pub(crate) item: Object,
}

/// A problem a source found in a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) collection: String,
    pub(crate) line: Option<usize>,
    pub(crate) reason: String,
}

/// What a source says about itself, for `srl sources`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Description {
    /// What the source is, in a sentence.
    pub(crate) summary: String,
    /// The globs of the collections `srl add` may create in it, relative
    /// to its prefix: those a glob of `include` matches and none of
    /// `exclude` does.
    pub(crate) include: Vec<String>,
    pub(crate) exclude: Vec<String>,
}

/// A likely mistake a source found in a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) collection: String,
    pub(crate) line: Option<usize>,
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
    pub(crate) name: String,
    pub(crate) driver: Driver,
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
    /// The sources of `store`, as `[sources]` sets them, in the order of
    /// their names.
    pub(crate) fn new(store: &Store, config: &Config) -> Self {
        Self {
            mounted: config
                .sources
                .iter()
                .map(|(name, source)| {
                    let files = NoteFiles::new(store, source);
                    let driver: Box<dyn Source> = match &source.driver {
                        Driver::Markdown => Box::new(Markdown::new(files)),
                        Driver::Jsonl => Box::new(Jsonl::new(files)),
                        Driver::Program { command, settings } => {
                            Box::new(Program::new(store, name, command, settings))
                        }
                    };
                    Mounted {
                        name: name.clone(),
                        driver: source.driver.clone(),
                        prefix: source.prefix.clone(),
                        source: driver,
                    }
                })
                .collect(),
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
