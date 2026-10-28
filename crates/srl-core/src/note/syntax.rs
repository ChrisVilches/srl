//! The item syntaxes: how items are written in a note (see `docs/notes.md`,
//! "Item syntaxes"), and everything that depends on it.
//!
//! This module is the boundary between notes as text and items as data
//! (see `docs/rust.md`, "The item syntax"). The rest of `srl` knows nothing
//! of fences, `@` lines or JSON lines: it sees only what these functions
//! return. A note's [`Syntax`] comes from its path; each syntax is a module
//! with the same functions:
//!
//! - Reading: `parse` gives a note's items, with their entries, fields and
//!   lines, its errors, and the [`Role`] of each line, from which the context
//!   is made.
//! - Writing: `with_id`, `without_draft` and `replace` give the [`Edit`] that
//!   makes a change to a note, and `format` writes a new item. Checking an
//!   edit and writing the file is left to `write.rs`.
//! - Checking: `lint` finds likely mistakes in the syntax.
//!
//! Lines are numbered from 1, as [`crate::note::lines`] splits them.

mod blocks;
mod jsonl;

use std::path::Path;

use crate::note::{Entries, Field, ItemId};

/// How the items of a note are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Syntax {
    /// srl blocks, fenced code blocks of Markdown.
    Blocks,
    /// One JSON object per line.
    Jsonl,
}

/// What a line of a note is for its context (see `docs/notes.md`,
/// "Context").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// Text of the note, shown in every mode but `none`.
    Prose,
    /// Text of an item, shown in mode `full` only.
    Item,
    /// Never shown: front-matter, and the syntax around items.
    Hidden,
}

/// Where something is: from line `first` to line `last`, counting from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lines {
    pub(crate) first: usize,
    pub(crate) last: usize,
}

/// An item as written, before its kind turns it into cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawItem {
    pub(crate) entries: Entries,
    /// The line that names the item in messages and in `PATH:LINE`, and
    /// identifies it to `with_id`, `without_draft` and `replace`: its
    /// header.
    pub(crate) line: usize,
    /// The last line of the item.
    pub(crate) end: usize,
    pub(crate) fields: Vec<Field>,
    /// Where each field is written, in the order of `fields`.
    pub(crate) spans: Vec<Lines>,
    /// The item written on its own, lines joined with `\n`: what `srl show`
    /// prints.
    pub(crate) text: String,
}

/// A parsed note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) items: Vec<RawItem>,
    /// Each problem, with its line.
    pub(crate) errors: Vec<(usize, String)>,
    /// The role of each line, in order.
    pub(crate) roles: Vec<Role>,
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

impl Syntax {
    /// The syntax of the note at `path`: JSONL if its extension is `jsonl`,
    /// in lowercase, srl blocks otherwise.
    pub(crate) fn of(path: &str) -> Self {
        if Path::new(path)
            .extension()
            .is_some_and(|ext| ext == "jsonl")
        {
            Self::Jsonl
        } else {
            Self::Blocks
        }
    }

    /// Reads the items of a note.
    pub(crate) fn parse(self, text: &str) -> Parsed {
        match self {
            Self::Blocks => blocks::parse(text),
            Self::Jsonl => jsonl::parse(text),
        }
    }

    /// The edit that gives the item whose header is on line `line` of
    /// `text` the ID `id`, or `None` if no header is there.
    pub(crate) fn with_id(self, text: &str, line: usize, id: &ItemId) -> Option<Edit> {
        match self {
            Self::Blocks => blocks::with_id(text, line, id),
            Self::Jsonl => jsonl::with_id(text, line, id),
        }
    }

    /// The edit that removes `draft` from the item whose header is on line
    /// `line` of `text`, or `None` if no header with `draft` is there.
    pub(crate) fn without_draft(self, text: &str, line: usize) -> Option<Edit> {
        match self {
            Self::Blocks => blocks::without_draft(text, line),
            Self::Jsonl => jsonl::without_draft(text, line),
        }
    }

    /// The edit that replaces the item whose header is on line `line` of
    /// `text` with an item of `entries` and `fields` (see `docs/cli.md`,
    /// "Editing items"). Errors are plain reasons.
    pub(crate) fn replace(
        self,
        text: &str,
        line: usize,
        entries: &Entries,
        fields: &[Field],
    ) -> Result<Edit, String> {
        match self {
            Self::Blocks => blocks::replace(text, line, entries, fields),
            Self::Jsonl => jsonl::replace(text, line, entries, fields),
        }
    }

    /// A new item as `srl add` writes it. Errors are plain reasons.
    pub(crate) fn format(self, entries: &Entries, fields: &[Field]) -> Result<String, String> {
        match self {
            Self::Blocks => blocks::format(entries, fields),
            Self::Jsonl => Ok(jsonl::format(entries, fields)),
        }
    }

    /// What `srl add` puts between two items, and between a note and the
    /// first item it appends: a blank line between srl blocks, a line
    /// ending between JSON lines.
    pub(crate) fn gap(self) -> &'static str {
        match self {
            Self::Blocks => "\n\n",
            Self::Jsonl => "\n",
        }
    }

    /// The likely mistakes of the syntax in one note, with their lines, in
    /// no particular order (see `docs/cli.md`, "Checking"). These are
    /// guesses, never errors: errors are [`Syntax::parse`]'s.
    pub(crate) fn lint(self, text: &str) -> Vec<(usize, String)> {
        match self {
            Self::Blocks => blocks::lint(text),
            // Every line is an item or blank, and parsing checks them all.
            Self::Jsonl => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_decides_the_syntax() {
        assert_eq!(Syntax::of("a/b.jsonl"), Syntax::Jsonl);
        for path in ["a.md", "a.txt", "a.json", "a.jsonl.md", "jsonl", "a.JSONL"] {
            assert_eq!(Syntax::of(path), Syntax::Blocks, "{path}");
        }
    }
}
