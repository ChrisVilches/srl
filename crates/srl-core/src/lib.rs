//! The core of `srl`: everything except the command line.
//!
//! This crate knows nothing about argument parsing, terminals or exit codes.
//! It takes its inputs explicitly (paths, environment values, text) so that
//! every part of it can be tested without a real process environment. The
//! `srl` binary crate is a thin layer that turns command-line arguments into
//! calls to this crate and prints the results.

mod app;
mod config;
mod domain;
mod error;
mod index;
mod kind;
mod log;
mod process;
mod source;
mod store;

pub use app::catalog::{Catalog, CollectionInfo, SourceDescription, collections, notes, sources};
pub use app::check::{Warning, orphans, warnings};
pub use app::next::next;
pub use app::review::{reset, review, suspend, unsuspend};
pub use app::write::{Added, Written, accept, add, assign_ids, edit};
pub use config::Config;
pub use domain::card::{Card, CardId, SUFFIX_SEPARATOR};
pub use domain::item::{Field, Item, ItemId};
pub use domain::schedule::{History, Memories, Memory, Place, Reviewed, Seen};
pub use domain::selector::{Selector, Status};
pub use domain::stats::{ScheduleCounts, Stats, stats};
pub use error::{Error, Result, Spot};
pub use index::Index;
pub use kind::{FieldDescription, Kind, KindDescription, kinds};
pub use log::{CardEvent, Entry, Event, Log, Review, Suspend};
#[cfg(unix)]
pub use process::stop_programs;
pub use store::{Lookup, Store};
