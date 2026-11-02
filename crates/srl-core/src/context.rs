//! An item's context: the collection shown after the answer of its cards
//! (see `docs/notes.md`, "Context").
//!
//! Which collection it is depends only on `see`; what that collection shows
//! is up to its own source, so an item of one source can name a collection
//! of another (see [`crate::Notes::contexts`]). How to render it is up to
//! whoever shows it, who can tell by the collection's address.

/// How the context is shown. Each frontend chooses it, with `--context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMode {
    /// The collection without its items.
    Prose,
    /// The collection with the text of its items, without the syntax around
    /// them.
    Full,
    /// Nothing.
    None,
}

/// An item's context: the text shown, and the address of the collection
/// it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub text: String,
    pub address: String,
}
