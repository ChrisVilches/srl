//! An item's context: the note shown after the answer of its cards (see
//! `docs/notes.md`, "Context").
//!
//! Which note it is depends only on `see`; what that note shows is up to
//! its own syntax (see [`syntax::Syntax::own_context`] and [`syntax::Syntax::context`]), so
//! an item in one syntax can name a note in another. That syntax also gives
//! the context's format ([`syntax::Syntax::context_format`]).

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

/// What the text of a context is, so that a frontend need not guess how to
/// render it: it follows the syntax of the note shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextFormat {
    /// A note of srl blocks, whatever its extension.
    Markdown,
    /// A JSONL note, as it is.
    Jsonl,
}

impl ContextFormat {
    /// Its name, as `context_format` gives it in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            ContextFormat::Markdown => "markdown",
            ContextFormat::Jsonl => "jsonl",
        }
    }
}

/// An item's context: the text shown, and its format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub text: String,
    pub format: ContextFormat,
}

/// The context of `item` in `mode`: what the note named by `see:` shows to
/// the items that name it, or else what the item's own note shows to its
/// own items, each as its syntax says, in that syntax's format. `None` in
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
        format: syntax.context_format(),
    }))
}
