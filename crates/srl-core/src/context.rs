//! An item's context: the note shown after the answer of its cards (see
//! `docs/notes.md`, "Context").

use serde::{Deserialize, Serialize};

use crate::note::parse::{self, Line};
use crate::note::read_note;
use crate::{Item, Result, Store};

/// How the context is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContextMode {
    /// The note without its items.
    #[default]
    Prose,
    /// The note with its items, without their header lines.
    Full,
    /// Nothing.
    None,
}

/// The context of `item` in `mode`: the note named by `see:`, or the
/// item's own note. `None` in mode `none`, and when nothing but blank space
/// is left.
pub fn context(store: &Store, item: &Item, mode: ContextMode) -> Result<Option<String>> {
    if mode == ContextMode::None {
        return Ok(None);
    }
    let path = item.see().unwrap_or(item.path());
    Ok(of_text(&read_note(store, path)?, mode))
}

/// The context of a note whose text is `text`, in `mode`.
pub(crate) fn of_text(text: &str, mode: ContextMode) -> Option<String> {
    let parsed = parse::parse(text);
    let kept = parse::lines(text)
        .into_iter()
        .zip(parsed.lines)
        .filter(|(_, kind)| match kind {
            Line::Text | Line::Code => true,
            Line::Item => mode == ContextMode::Full,
            Line::FrontMatter | Line::Header => false,
        })
        .map(|(line, _)| line);
    let mut lines = Vec::new();
    for line in kept {
        let blank = line.trim().is_empty();
        let after_blank = lines.last().is_none_or(|last: &&str| last.is_empty());
        if blank && after_blank {
            continue;
        }
        lines.push(if blank { "" } else { line });
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    (!lines.is_empty()).then(|| format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\ntags: [x]\n---\n\n# Title\n\nProse.\n\n\n<!-- srl id:abc -->\nQ: q\n\nA: a\n\nMore.\n<!-- srl id:abd -->\nQ: {{c1::x}}\n\n";

    #[test]
    fn prose_leaves_out_items_and_front_matter() {
        assert_eq!(
            of_text(NOTE, ContextMode::Prose).as_deref(),
            Some("# Title\n\nProse.\n\nMore.\n")
        );
    }

    #[test]
    fn full_keeps_items_without_their_headers() {
        assert_eq!(
            of_text(NOTE, ContextMode::Full).as_deref(),
            Some("# Title\n\nProse.\n\nQ: q\n\nA: a\n\nMore.\nQ: {{c1::x}}\n")
        );
    }

    #[test]
    fn nothing_but_blank_space_is_no_context() {
        let only_items =
            "\n<!-- srl id:abc -->\nQ: q\nA: a\n\n  \n<!-- srl id:abd -->\nQ: r\nA: b\n";
        assert_eq!(of_text(only_items, ContextMode::Prose), None);
        assert!(of_text(only_items, ContextMode::Full).is_some());
        assert_eq!(of_text("", ContextMode::Full), None);
    }
}
