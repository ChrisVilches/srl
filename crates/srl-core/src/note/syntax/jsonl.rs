//! The JSONL syntax: one item object per line (see `docs/notes.md`, "The
//! JSONL syntax"). `srl` writes an item by writing its line again, as
//! [`object::written`] gives it, so a line it changes loses its spacing and
//! key order.

use serde_json::Value;

use super::{Edit, Lines, Parsed, RawItem, Role};
use crate::note::object::{self, Object};
use crate::note::{Entries, Field, ItemId, lines};

/// Reads the items of a note: each line that is not blank is an item.
pub(super) fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed {
        items: Vec::new(),
        errors: Vec::new(),
        roles: Vec::new(),
    };
    for (index, content) in lines(text).into_iter().enumerate() {
        let line = index + 1;
        parsed.roles.push(Role::Prose);
        if content.trim().is_empty() {
            continue;
        }
        match object::read(content) {
            Ok(Object { entries, fields }) => parsed.items.push(RawItem {
                entries,
                line,
                end: line,
                spans: vec![
                    Lines {
                        first: line,
                        last: line
                    };
                    fields.len()
                ],
                fields,
                text: content.to_owned(),
            }),
            Err(reason) => parsed.errors.push((line, reason)),
        }
    }
    parsed
}

/// The item on line `line` of `text`, if there is one.
fn item_at(text: &str, line: usize) -> Option<Object> {
    let content = *lines(text).get(line.checked_sub(1)?)?;
    object::read(content).ok()
}

pub(super) fn with_id(text: &str, line: usize, id: &ItemId) -> Option<Edit> {
    let Object { entries, fields } = item_at(text, line)?;
    let entries = Entries {
        id: Some(id.clone()),
        ..entries
    };
    Some(Edit::one(line, format(&entries, &fields)))
}

pub(super) fn without_draft(text: &str, line: usize) -> Option<Edit> {
    let Object { entries, fields } = item_at(text, line).filter(|item| item.entries.draft)?;
    let entries = Entries {
        draft: false,
        ..entries
    };
    Some(Edit::one(line, format(&entries, &fields)))
}

pub(super) fn replace(
    text: &str,
    line: usize,
    entries: &Entries,
    fields: &[Field],
) -> Result<Edit, String> {
    item_at(text, line).ok_or_else(|| format!("no item is on line {line} any more"))?;
    Ok(Edit::one(line, format(entries, fields)))
}

/// An item as one line of JSON.
pub(super) fn format(entries: &Entries, fields: &[Field]) -> String {
    Value::Object(object::written(entries, fields)).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_that_is_not_blank_is_an_item() {
        let text =
            "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}\r\n\n  \n{\"Q\": \"r\"}\nnot json\n";
        let parsed = parse(text);
        let lines = parsed.items.iter().map(|item| item.line);
        assert_eq!(lines.collect::<Vec<_>>(), [1, 4]);
        assert_eq!(
            parsed.items[0].text,
            "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}"
        );
        assert!(parsed.items[1].entries.id.is_none());
        assert_eq!(parsed.errors.len(), 1);
        assert_eq!(parsed.errors[0].0, 5);
        assert_eq!(parsed.roles.len(), 5);
    }

    #[test]
    fn edits_write_the_line_again() {
        let text = "{\"Q\": \"q\",  \"draft\": true}\n{\"id\": \"abc\", \"Q\": \"r\"}\n";
        let id = ItemId::parse("xyz").unwrap();
        assert_eq!(
            with_id(text, 1, &id),
            Some(Edit::one(
                1,
                r#"{"id":"xyz","draft":true,"Q":"q"}"#.to_owned()
            ))
        );
        assert_eq!(
            without_draft(text, 1),
            Some(Edit::one(1, r#"{"Q":"q"}"#.to_owned()))
        );
        assert_eq!(without_draft(text, 2), None);
        assert_eq!(with_id(text, 3, &id), None);
        let fields = [Field {
            key: "Q".to_owned(),
            text: "new".to_owned(),
        }];
        let entries = Entries {
            id: Some(ItemId::parse("abc").unwrap()),
            ..Entries::default()
        };
        assert_eq!(
            replace(text, 2, &entries, &fields).unwrap(),
            Edit::one(2, r#"{"id":"abc","Q":"new"}"#.to_owned())
        );
    }
}
