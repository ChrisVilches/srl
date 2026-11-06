//! JSONL collections: one item object per line (see `docs/formats.md`,
//! "JSONL"). `srl` writes an item by writing its line again, as
//! [`object::written`] gives it, so a line it changes loses its spacing and
//! key order, and every other line is kept as it was.

use serde_json::Value;

use crate::domain::item::object::{self, Object};
use crate::domain::item::{Entries, Field};
use crate::source::files::{
    Parsed, ParsedItem, changed_meanwhile, keyed, line_ending, lines, position_of, verified,
};
use crate::source::{Change, Refusal};

/// Each line that is not blank is an item.
pub(super) fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed {
        items: Vec::new(),
        errors: Vec::new(),
    };
    for (line, content) in item_lines(text) {
        match object::read(content) {
            Ok(Object { entries, fields }) => parsed.items.push(ParsedItem {
                entries,
                fields,
                text: content.to_owned(),
            }),
            Err(reason) => parsed.errors.push(format!("line {line}: {reason}")),
        }
    }
    parsed
}

/// The JSONL file at `path`, `old`, with `changes` made to it and checked,
/// and the lines added. A changed item's line is written again; new items
/// are appended, one line each.
pub(super) fn changed(
    path: &str,
    old: Option<&str>,
    changes: &[(usize, &Change)],
) -> Result<(String, String), Refusal> {
    let parsed = parse(old.unwrap_or_default());
    if !parsed.errors.is_empty() {
        return Err(changed_meanwhile(path));
    }
    let mut expected = keyed(parsed);
    let mut text = old.unwrap_or_default().to_owned();
    let mut added = Vec::new();
    for &(_, change) in changes {
        match change {
            Change::Replace {
                handle,
                before,
                after,
                ..
            } => {
                if old.is_none() {
                    return Err(changed_meanwhile(path));
                }
                let position = position_of(&expected, handle, before, path)?;
                if before != after {
                    let line = item_lines(&text)
                        .nth(position)
                        .map(|(line, _)| line)
                        .ok_or_else(|| changed_meanwhile(path))?;
                    text = replace_line(&text, line, &written(&after.entries, &after.fields));
                }
                let slot = expected
                    .get_mut(position)
                    .expect("the position of an item should be among the file's items");
                *slot = (after.entries.clone(), after.fields.clone());
            }
            Change::Add { item, .. } => {
                expected.push((item.entries.clone(), item.fields.clone()));
                added.push(written(&item.entries, &item.fields));
            }
        }
    }
    let added = if added.is_empty() {
        String::new()
    } else {
        format!("{}\n", added.join("\n"))
    };
    let new = append(&text, &added);
    let parsed = parse(&new);
    verified(path, parsed.errors.is_empty() && keyed(parsed) == expected)?;
    Ok((new, added))
}

/// The lines of `text` that are not blank, with their numbers.
fn item_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    lines(text)
        .into_iter()
        .enumerate()
        .map(|(index, content)| (index + 1, content))
        .filter(|(_, content)| !content.trim().is_empty())
}

/// The line `srl` writes for an item of `entries` and `fields`.
fn written(entries: &Entries, fields: &[Field]) -> String {
    Value::Object(object::written(entries, fields)).to_string()
}

/// `text` with its line `line` (counting from 1) replaced by `content`,
/// keeping that line's ending and a byte order mark.
fn replace_line(text: &str, line: usize, content: &str) -> String {
    let raw = text.split_inclusive('\n').collect::<Vec<_>>();
    let before = raw.get(..line - 1).unwrap_or_default().concat();
    let old = raw.get(line - 1).copied().unwrap_or_default();
    let ending = old
        .get(old.trim_end_matches(['\n', '\r']).len()..)
        .unwrap_or_default();
    let bom = if line == 1 && text.starts_with('\u{feff}') {
        "\u{feff}"
    } else {
        ""
    };
    let after = raw.get(line..).unwrap_or_default().concat();
    format!("{before}{bom}{content}{ending}{after}")
}

/// `text` with the lines `added` appended on a line of their own, in the
/// line endings of `text`.
fn append(text: &str, added: &str) -> String {
    if added.is_empty() {
        return text.to_owned();
    }
    let ending = line_ending(text);
    let tail = added.replace('\n', ending);
    if text.is_empty() || text.ends_with('\n') {
        format!("{text}{tail}")
    } else {
        format!("{text}{ending}{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::item::ItemId;

    #[test]
    fn every_line_that_is_not_blank_is_an_item() {
        let text =
            "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}\r\n\n  \n{\"Q\": \"r\"}\nnot json\n";
        let parsed = parse(text);
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(
            parsed.items[0].text,
            "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}"
        );
        assert!(parsed.items[1].entries.id.is_none());
        assert_eq!(parsed.errors.len(), 1);
        assert!(
            parsed.errors[0].starts_with("line 5: "),
            "{:?}",
            parsed.errors
        );
    }

    #[test]
    fn edits_write_the_line_of_the_item_again() {
        let text = "{\"Q\": \"q\",  \"draft\": true}\r\n\n{\"id\": \"abc\", \"Q\": \"r\"}\r\n";
        let before = Object {
            entries: Entries {
                id: Some(ItemId::parse("abc").unwrap()),
                ..Entries::default()
            },
            fields: vec![Field {
                key: "Q".to_owned(),
                text: "r".to_owned(),
            }],
        };
        let mut after = before.clone();
        after.fields[0].text = "new".to_owned();
        let change = Change::Replace {
            collection: "a.jsonl".to_owned(),
            handle: "1".to_owned(),
            before,
            after,
        };
        let (new, added) = changed("a.jsonl", Some(text), &[(0, &change)]).unwrap();
        assert_eq!(
            new,
            "{\"Q\": \"q\",  \"draft\": true}\r\n\n{\"id\":\"abc\",\"Q\":\"new\"}\r\n"
        );
        assert_eq!(added, "");
    }
}
