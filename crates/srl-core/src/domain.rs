//! The domain: items, cards, selectors, scheduling and statistics, as
//! values (see `docs/rust.md`, "Layers"). Nothing here reads a file,
//! starts a process or knows the clock: it is given what it needs.

pub(crate) mod card;
pub(crate) mod item;
pub(crate) mod random;
pub(crate) mod schedule;
pub(crate) mod selector;
pub(crate) mod stats;
