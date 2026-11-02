//! The JSONL syntax: one item object per line (see `docs/notes.md`, "The
//! JSONL syntax"). `srl` writes an item by writing its line again, as
//! [`object::written`] gives it, so a line it changes loses its spacing and
//! key order.

use serde_json::Value;

use super::{Edit, Lines, Parsed, ParsedItem, Syntax, unless_blank};
use crate::context::ContextMode;
use crate::note::object::{self, Object};
use crate::note::{Entries, Field, ItemId};

/// The JSONL syntax.
pub(super) struct Jsonl;

impl Syntax for Jsonl {
    /// Each line that is not blank is an item.
    fn parse(&self, text: &str) -> Parsed {
        let mut parsed = Parsed {
            items: Vec::new(),
            errors: Vec::new(),
        };
        for (line, content) in item_lines(text) {
            match object::read(content) {
                Ok(Object { entries, fields }) => parsed.items.push(ParsedItem {
                    entries,
                    spans: vec![
                        Lines {
                            first: line,
                            last: line
                        };
                        fields.len()
                    ],
                    fields,
                    line,
                    text: content.to_owned(),
                }),
                Err(reason) => parsed.errors.push((line, reason)),
            }
        }
        parsed
    }

    /// A JSONL note is all items, so it is no context of its own items.
    fn own_context(&self, _text: &str, _mode: ContextMode) -> Option<String> {
        None
    }

    /// The note as it is, in every mode: nothing is removed but a byte
    /// order mark, which is no part of a note's text.
    fn context(&self, text: &str, _mode: ContextMode) -> Option<String> {
        unless_blank(text.strip_prefix('\u{feff}').unwrap_or(text).to_owned())
    }

    fn with_id(&self, text: &str, item: usize, id: &ItemId) -> Option<Edit> {
        let (line, Object { entries, fields }) = item_at(text, item)?;
        let entries = Entries {
            id: Some(id.clone()),
            ..entries
        };
        Some(Edit::one(line, written(&entries, &fields)))
    }

    fn without_draft(&self, text: &str, item: usize) -> Option<Edit> {
        let (line, Object { entries, fields }) =
            item_at(text, item).filter(|(_, object)| object.entries.draft)?;
        let entries = Entries {
            draft: false,
            ..entries
        };
        Some(Edit::one(line, written(&entries, &fields)))
    }

    fn replace(
        &self,
        text: &str,
        item: usize,
        entries: &Entries,
        fields: &[Field],
    ) -> Result<Edit, String> {
        let (line, _) =
            item_at(text, item).ok_or_else(|| format!("the note has no item {item}"))?;
        Ok(Edit::one(line, written(entries, fields)))
    }

    /// An item as one line of JSON.
    fn format(&self, entries: &Entries, fields: &[Field]) -> Result<String, String> {
        Ok(written(entries, fields))
    }

    /// A line ending: JSON lines follow each other.
    fn gap(&self) -> &'static str {
        "\n"
    }
}

/// The lines of `text` that are not blank, with their numbers.
fn item_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    crate::note::lines(text)
        .into_iter()
        .enumerate()
        .map(|(index, content)| (index + 1, content))
        .filter(|(_, content)| !content.trim().is_empty())
}

/// Item `item` of `text`, with its line, if it is there and valid.
fn item_at(text: &str, item: usize) -> Option<(usize, Object)> {
    let (line, content) = item_lines(text).nth(item)?;
    Some((line, object::read(content).ok()?))
}

/// The line `srl` writes for an item of `entries` and `fields`.
fn written(entries: &Entries, fields: &[Field]) -> String {
    Value::Object(object::written(entries, fields)).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_that_is_not_blank_is_an_item() {
        let text =
            "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}\r\n\n  \n{\"Q\": \"r\"}\nnot json\n";
        let parsed = Jsonl.parse(text);
        let lines = parsed.items.iter().map(|item| item.line);
        assert_eq!(lines.collect::<Vec<_>>(), [1, 4]);
        assert_eq!(
            parsed.items[0].text,
            "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}"
        );
        assert!(parsed.items[1].entries.id.is_none());
        assert_eq!(parsed.errors.len(), 1);
        assert_eq!(parsed.errors[0].0, 5);
    }

    #[test]
    fn edits_write_the_line_of_the_item_again() {
        let text = "{\"Q\": \"q\",  \"draft\": true}\n\n{\"id\": \"abc\", \"Q\": \"r\"}\n";
        let id = ItemId::parse("xyz").unwrap();
        assert_eq!(
            Jsonl.with_id(text, 0, &id),
            Some(Edit::one(
                1,
                r#"{"id":"xyz","draft":true,"Q":"q"}"#.to_owned()
            ))
        );
        assert_eq!(
            Jsonl.without_draft(text, 0),
            Some(Edit::one(1, r#"{"Q":"q"}"#.to_owned()))
        );
        assert_eq!(Jsonl.without_draft(text, 1), None);
        assert_eq!(Jsonl.with_id(text, 2, &id), None);
        let fields = [Field {
            key: "Q".to_owned(),
            text: "new".to_owned(),
        }];
        let entries = Entries {
            id: Some(ItemId::parse("abc").unwrap()),
            ..Entries::default()
        };
        assert_eq!(
            Jsonl.replace(text, 1, &entries, &fields).unwrap(),
            Edit::one(3, r#"{"id":"abc","Q":"new"}"#.to_owned())
        );
        assert!(Jsonl.replace(text, 2, &entries, &fields).is_err());
    }

    #[test]
    fn a_jsonl_note_is_given_whole_and_never_to_its_own_items() {
        let text = "\n{\"Q\": \"q\"}\n\n";
        for mode in [ContextMode::Prose, ContextMode::Full] {
            assert_eq!(Jsonl.context(text, mode).as_deref(), Some(text));
            assert_eq!(Jsonl.own_context(text, mode), None);
            assert_eq!(Jsonl.context("\n  \n", mode), None);
            // A byte order mark is the one thing left out.
            let marked = "\u{feff}{\"Q\": \"q\"}\r\n";
            assert_eq!(
                Jsonl.context(marked, mode).as_deref(),
                Some("{\"Q\": \"q\"}\r\n")
            );
        }
    }

    #[test]
    fn a_fields_lines_are_named_by_the_items_line() {
        let text = "\n{\"Q\": \"a\\nb\", \"A\": \"c\"}\n";
        let item = &Jsonl.parse(text).items[0];
        assert_eq!(
            Jsonl.field_lines(text, item, 0),
            [(2, "a".to_owned()), (2, "b".to_owned())]
        );
    }
}
