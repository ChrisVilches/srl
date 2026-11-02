//! An item's context: the note shown after the answer of its cards (see
//! `docs/notes.md`, "Context").
//!
//! Which note it is depends only on `see`; what that note shows is up to
//! its own syntax (see [`syntax::Syntax::own_context`] and [`syntax::Syntax::context`]), so
//! an item in one syntax can name a note in another. How to render it is up
//! to whoever shows it, who can tell by the note's path.

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

/// An item's context: the text shown, and the address of the note it comes
/// from: its path, relative to the store's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub text: String,
    pub address: String,
}

/// The context of `item` in `mode`: what the note named by `see:` shows to
/// the items that name it, or else what the item's own note shows to its
/// own items, each as its syntax says, with that note's path. `None` in
/// mode `none`, without reading anything, and when there is nothing to
/// show.
pub fn context(store: &Store, item: &Item, mode: ContextMode) -> Result<Option<Context>> {
    if mode == ContextMode::None {
        return Ok(None);
    }
    let note = item.see().unwrap_or(item.path());
    let syntax = syntax::of(note);
    let text = read_note(store, note)?;
    let text = if item.see().is_some() {
        syntax.context(&text, mode)
    } else {
        syntax.own_context(&text, mode)
    };
    Ok(text.map(|text| Context {
        text,
        address: note.to_owned(),
    }))
}
