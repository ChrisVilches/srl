//! An item's context: the note shown after the answer of its cards (see
//! `docs/notes.md`, "Context").
//!
//! Which note it is depends only on `see`; what that note shows is up to
//! its own syntax (see [`syntax::Syntax::own_context`] and [`syntax::Syntax::context`]), so
//! an item in one syntax can name a note in another.

use crate::note::read_note;
use crate::note::syntax;
use crate::{Item, Result, Store};

/// How the context is shown. Each frontend chooses it, with `--context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMode {
    /// The note without its items.
    Prose,
    /// The note with the text of its items, without the syntax around them.
    Full,
    /// Nothing.
    None,
}

/// The context of `item` in `mode`: what the note named by `see:` shows to
/// the items that name it, or else what the item's own note shows to its
/// own items, each as its syntax says. `None` in mode `none`, without
/// reading anything, and when there is nothing to show.
pub fn context(store: &Store, item: &Item, mode: ContextMode) -> Result<Option<String>> {
    if mode == ContextMode::None {
        return Ok(None);
    }
    Ok(match item.see() {
        Some(see) => syntax::of(see).context(&read_note(store, see)?, mode),
        None => syntax::of(item.path()).own_context(&read_note(store, item.path())?, mode),
    })
}
