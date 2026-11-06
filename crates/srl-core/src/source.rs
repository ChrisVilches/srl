//! The source port: where items come from, and how they are written back
//! (see `docs/sources.md`, "What a source does", and `docs/rust.md`,
//! "Sources").
//!
//! A source is read whole, into the catalog, and everything else in `srl`
//! works on the catalog. A source is asked again only to change items.
//! Nothing outside a source's own module knows how it keeps its items: a
//! source names its collections and its items, and `srl` names them back
//! to it.

pub(crate) mod files;
mod filesystem;
mod program;

use std::fmt;

use crate::config::Driver;
use crate::domain::item::object::Object;
use crate::{Config, Error, Result, Store};
use filesystem::Filesystem;
use program::Program;

/// The operations every source answers.
pub(crate) trait Source: fmt::Debug {
    /// The name of every collection of the source, including those without
    /// items, in order.
    fn collections(&self) -> Result<Vec<String>>;

    /// Every item of the source, and every problem found in it. Only
    /// failing to read the source at all is an error.
    fn read(&self) -> Result<Read>;

    /// Why `collection`, which doesn't exist yet, can't be created by `srl
    /// add`, or `None` if it can.
    fn refuses(&self, collection: &str) -> Option<String>;

    /// Makes `changes`, or, with `dry_run`, only checks them. Gives the
    /// text of the items added, as they appear in their collections.
    fn write(&self, changes: &[Change], dry_run: bool) -> std::result::Result<String, Refusal>;

    /// What the source is, for `srl sources`.
    fn describe(&self) -> Result<Description>;
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
    /// The item as written, for `srl show`, if the source gives it.
    pub(crate) written: Option<String>,
    pub(crate) item: Object,
}

/// A problem a source found in a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) collection: String,
    pub(crate) reason: String,
}

/// What a source says about itself, for `srl sources`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Description {
    /// What the source is, in a sentence.
    pub(crate) summary: String,
    /// The globs of the collections `srl add` may create in it: those a
    /// glob of `include` matches and none of `exclude` does.
    pub(crate) include: Vec<String>,
    pub(crate) exclude: Vec<String>,
}

/// A change to the items of a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    /// Adds `item` to `collection`, creating it if it doesn't exist.
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

/// Every source of a store.
#[derive(Debug)]
pub(crate) struct Sources {
    named: Vec<Named>,
}

/// A source, with its name and driver.
#[derive(Debug)]
pub(crate) struct Named {
    pub(crate) name: String,
    pub(crate) driver: Driver,
    pub(crate) source: Box<dyn Source>,
}

impl Sources {
    /// The sources of `store`, as `[sources]` sets them, in the order of
    /// their names.
    pub(crate) fn new(store: &Store, config: &Config) -> Self {
        Self {
            named: config
                .sources
                .iter()
                .map(|(name, source)| {
                    let driver: Box<dyn Source> = match &source.driver {
                        Driver::Filesystem { include, exclude } => {
                            Box::new(Filesystem::new(store, include, exclude))
                        }
                        Driver::Program { command, settings } => {
                            Box::new(Program::new(store, name, command, settings))
                        }
                    };
                    Named {
                        name: name.clone(),
                        driver: source.driver.clone(),
                        source: driver,
                    }
                })
                .collect(),
        }
    }

    /// The sources, in order; an item names its source by its place here.
    pub(crate) fn all(&self) -> &[Named] {
        &self.named
    }

    /// The source that `address`, of a collection that doesn't exist yet,
    /// would be created in, or why it can't be created: exactly one source
    /// must accept it (see `docs/sources.md`, "Addresses").
    pub(crate) fn for_new(&self, address: &str) -> std::result::Result<usize, String> {
        let answers = self
            .named
            .iter()
            .enumerate()
            .map(|(index, named)| (index, named.source.refuses(address)))
            .collect::<Vec<_>>();
        let accepting = answers
            .iter()
            .filter(|(_, refusal)| refusal.is_none())
            .map(|(index, _)| *index)
            .collect::<Vec<_>>();
        match accepting.as_slice() {
            [index] => Ok(*index),
            [] => Err(answers
                .into_iter()
                .find_map(|(_, refusal)| refusal)
                .unwrap_or_else(|| "the store has no source to hold it".to_owned())),
            _ => Err("more than one source of the store would hold it".to_owned()),
        }
    }
}

/// The items of a collection file at `path` whose text is `text`, read in
/// the format its extension chooses: for tests that need items without a
/// store.
#[cfg(test)]
pub(crate) fn parse(path: &str, text: &str) -> files::Parsed {
    filesystem::parse(path, text)
}
