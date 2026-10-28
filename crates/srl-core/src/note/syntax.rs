//! The item syntaxes: how items are written in a note (see `docs/notes.md`,
//! "Item syntaxes"), and everything that depends on it.
//!
//! This module is the boundary between notes as text and items as data
//! (see `docs/rust.md`, "The item syntax"). A note's syntax is the
//! implementation of [`Syntax`] that [`of`] picks for its path; the rest of
//! `srl` calls its methods and knows nothing of fences, `@` lines or JSON
//! lines, nor of which syntax it holds:
//!
//! - Reading: [`Syntax::parse`] gives a note's items as [`ParsedItem`]s,
//!   with their entries and fields, and its errors.
//! - Context: [`Syntax::own_context`] and [`Syntax::context`] give what a
//!   note shows as the context of its own items, and of the items that
//!   name it with `see`, and [`Syntax::context_format`] its format.
//! - Writing: [`Syntax::with_id`], [`Syntax::without_draft`] and
//!   [`Syntax::replace`] give the [`Edit`] that makes a change to an item,
//!   named by its position in the note, and [`Syntax::format`] writes a new
//!   item. Checking an edit and writing the file is left to `write.rs`.
//! - Checking: [`Syntax::lint`] finds likely mistakes in the syntax, and
//!   [`Syntax::field_lines`] says where a field's text is, for messages.
//!
//! Lines are numbered from 1, as [`crate::note::lines`] splits them. A line
//! is only ever shown to people, in `PATH:LINE`: an item is found again by
//! its position or its ID, never by its line (see `docs/rust.md`, "The item
//! syntax").

mod blocks;
mod jsonl;

use std::path::Path;

use crate::context::{ContextFormat, ContextMode};
use crate::note::{Entries, Field, ItemId};
use blocks::Blocks;
use jsonl::Jsonl;

/// The syntax of the note at `path`, chosen by its extension alone: JSONL
/// if it is `jsonl`, in lowercase, srl blocks otherwise.
pub(crate) fn of(path: &str) -> &'static dyn Syntax {
    if Path::new(path)
        .extension()
        .is_some_and(|ext| ext == "jsonl")
    {
        &Jsonl
    } else {
        &Blocks
    }
}

/// How the items of a note are written: one implementation per syntax.
///
/// An item is named by its position among the items [`Syntax::parse`]
/// gives, counting from 0, wherever the syntax writes it.
pub(crate) trait Syntax {
    /// Reads the items of a note, and its errors.
    fn parse(&self, text: &str) -> Parsed;

    /// What the note whose text is `text` shows as the context of its own
    /// items, those without `see`, in `mode` (never `none`, which shows
    /// nothing whatever the syntax). `None` when there is nothing to show.
    fn own_context(&self, text: &str, mode: ContextMode) -> Option<String>;

    /// What the note whose text is `text` shows as the context of the items
    /// that name it with `see`, wherever they are, in `mode` (never
    /// `none`). `None` when there is nothing to show.
    fn context(&self, text: &str, mode: ContextMode) -> Option<String>;

    /// The format of what [`Syntax::own_context`] and [`Syntax::context`]
    /// give, the same for both.
    fn context_format(&self) -> ContextFormat;

    /// The edit that gives item `item` of `text` the ID `id`, or `None` if
    /// there is no such item.
    fn with_id(&self, text: &str, item: usize, id: &ItemId) -> Option<Edit>;

    /// The edit that removes `draft` from item `item` of `text`, or `None`
    /// if there is no such item or it is not a draft.
    fn without_draft(&self, text: &str, item: usize) -> Option<Edit>;

    /// The edit that replaces item `item` of `text` with an item of
    /// `entries` and `fields` (see `docs/cli.md`, "Editing items"). Errors
    /// are plain reasons.
    fn replace(
        &self,
        text: &str,
        item: usize,
        entries: &Entries,
        fields: &[Field],
    ) -> Result<Edit, String>;

    /// A new item as `srl add` writes it. Errors are plain reasons.
    fn format(&self, entries: &Entries, fields: &[Field]) -> Result<String, String>;

    /// What `srl add` puts between two items, and between a note and the
    /// first item it appends.
    fn gap(&self) -> &'static str;

    /// The likely mistakes of the syntax in one note, with their lines, in
    /// no particular order (see `docs/cli.md`, "Checking"). These are
    /// guesses, never errors: errors are [`Syntax::parse`]'s. None by
    /// default.
    fn lint(&self, _text: &str) -> Vec<(usize, String)> {
        Vec::new()
    }

    /// The lines of field `field` of `item`, a parsed item of `text`, as
    /// `srl lint` looks at them, each with the line of the note a message
    /// about it names. By default, the lines of the field's text, all named
    /// by the item's line.
    fn field_lines(&self, _text: &str, item: &ParsedItem, field: usize) -> Vec<(usize, String)> {
        item.fields.get(field).map_or_else(Vec::new, |field| {
            field
                .text
                .lines()
                .map(|line| (item.line, line.to_owned()))
                .collect()
        })
    }
}

/// Where something is: from line `first` to line `last`, counting from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lines {
    pub(crate) first: usize,
    pub(crate) last: usize,
}

/// An item as a syntax reads it, before its kind turns it into cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedItem {
    pub(crate) entries: Entries,
    pub(crate) fields: Vec<Field>,
    /// The line that names the item in messages and in `PATH:LINE`: where
    /// it starts, as near as the syntax can say. Only for people.
    pub(crate) line: usize,
    /// Where each field is written, in the order of `fields`, for the
    /// syntax's own [`Syntax::field_lines`].
    pub(crate) spans: Vec<Lines>,
    /// The item written on its own, lines joined with `\n`: what `srl show`
    /// prints.
    pub(crate) text: String,
}

/// A parsed note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    /// The items, in the order they are written: an item's position here
    /// names it to the methods that change it.
    pub(crate) items: Vec<ParsedItem>,
    /// Each problem, with its line.
    pub(crate) errors: Vec<(usize, String)>,
}

/// A change to a note: lines `first` to `last` (counting from 1) replaced
/// by `lines`. The line endings and a byte order mark are left to whoever
/// applies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    pub(crate) first: usize,
    pub(crate) last: usize,
    pub(crate) lines: Vec<String>,
}

impl Edit {
    /// The edit that replaces line `line` with `content`.
    fn one(line: usize, content: String) -> Self {
        Self {
            first: line,
            last: line,
            lines: vec![content],
        }
    }
}

/// `text`, unless it holds nothing but blank space, which is no context
/// (see `docs/notes.md`, "Context").
fn unless_blank(text: String) -> Option<String> {
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_decides_the_syntax() {
        let line = "{\"id\": \"abc\", \"Q\": \"q\"}\n";
        assert_eq!(of("a/b.jsonl").parse(line).items.len(), 1);
        for path in ["a.md", "a.txt", "a.json", "a.jsonl.md", "jsonl", "a.JSONL"] {
            let parsed = of(path).parse(line);
            assert!(parsed.items.is_empty(), "{path}");
            assert!(parsed.errors.is_empty(), "{path}");
        }
    }
}
