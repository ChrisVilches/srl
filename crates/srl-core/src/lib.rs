//! The core of `srl`: everything except the command line.
//!
//! This crate knows nothing about argument parsing, terminals or exit codes.
//! It takes its inputs explicitly (paths, environment values, text) so that
//! every part of it can be tested without a real process environment. The
//! `srl` binary crate is a thin layer that turns command-line arguments into
//! calls to this crate and prints the results.

mod add;
mod atom;
mod card;
mod check;
mod config;
mod drill;
mod edit;
mod error;
mod frontmatter;
mod index;
mod log;
mod next;
mod process;
mod random;
mod review;
mod schedule;
mod selector;
mod session;
mod stats;
mod store;
mod yaml;

pub use add::{Added, Flags, add};
pub use atom::{Atom, Atoms, Content, Id, Status, TagCount};
pub use card::Card;
pub use check::{Fixed, Report, check};
pub use config::Config;
pub use drill::{Drill, DrillCards, Drills, Search};
pub use edit::{change_status, replace};
pub use error::{Error, Result};
pub use index::Index;
pub use log::{By, CardUsed, Entry, Event, Log, Reset, Review};
pub use next::{NextOptions, next};
pub use review::{ReviewRequest, reset, review, review_ids};
pub use schedule::{History, Memories, Memory, Reviewed, State};
pub use selector::Selector;
pub use session::{SessionInfo, list_sessions, load_session, put_session, remove_session};
pub use stats::{StateCounts, Stats, stats};
pub use store::{Lookup, Store};
