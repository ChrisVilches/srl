//! The application: what each command does with the domain and the ports
//! (see `docs/rust.md`, "Layers"). It reads the sources into the catalog,
//! decides, and asks the sources, kind programs and the log to change
//! things.

pub(crate) mod catalog;
pub(crate) mod check;
pub(crate) mod next;
pub(crate) mod review;
pub(crate) mod write;
