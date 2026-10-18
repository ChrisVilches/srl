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
mod random;
mod selector;
mod store;
mod yaml;

pub use add::{Added, Flags, add};
pub use atom::{Atom, Atoms, Content, Id, Status, TagCount};
pub use card::{Card, Grading};
pub use check::{Fixed, Report, check};
pub use config::Config;
pub use drill::{Drill, DrillCards, Drills, Search};
pub use edit::{change_status, replace};
pub use error::{Error, Result};
pub use selector::Selector;
pub use store::{Lookup, Store};
