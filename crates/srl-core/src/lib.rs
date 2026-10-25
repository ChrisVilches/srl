//! The core of `srl`: everything except the command line.
//!
//! This crate knows nothing about argument parsing, terminals or exit codes.
//! It takes its inputs explicitly (paths, environment values, text) so that
//! every part of it can be tested without a real process environment. The
//! `srl` binary crate is a thin layer that turns command-line arguments into
//! calls to this crate and prints the results.

mod card;
mod check;
mod config;
mod context;
mod error;
mod index;
mod kind;
mod log;
mod next;
mod note;
mod random;
mod review;
mod schedule;
mod selector;
mod stats;
mod store;
mod write;

pub use card::{Card, CardId};
pub use check::{Lint, lint, orphans};
pub use config::Config;
pub use context::{ContextMode, context};
pub use error::{Error, Result};
pub use index::Index;
pub use kind::{FieldDescription, Kind, KindDescription, kinds};
pub use log::{By, CardEvent, Entry, Event, Log, Move, Review};
pub use next::next;
pub use note::{Field, Item, ItemId, Notes};
pub use review::{ReviewRequest, move_history, reset, review, suspend};
pub use schedule::{History, Memories, Memory, Reviewed, Seen, State};
pub use selector::{Selector, Status};
pub use stats::{StateCounts, Stats, stats};
pub use store::{Lookup, Store};
pub use write::{Added, Written, accept, add, assign_ids, replace};
