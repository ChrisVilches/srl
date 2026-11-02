//! The `jsonl` source: notes of one item object per line (see
//! `docs/notes.md`, "The JSONL syntax"). `srl` writes an item by writing its
//! line again, as [`object::written`] gives it, so a line it changes loses
//! its spacing and key order.

use serde_json::Value;

use crate::check::question_suspicions;
use crate::context::ContextMode;
use crate::note::object::{self, Object};
use crate::note::{Entries, Field};
use crate::source::files::{
    Edit, Lines, NoteFiles, Parsed, ParsedItem, add_read, append, apply, by_collection,
    changed_meanwhile, keyed, lines, position_of, unless_blank, verified,
};
use crate::source::{Change, Collection, ContextRequest, Finding, Read, Refusal, Source};

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

/// Nothing in mode `prose`, which never shows items, and a JSONL note
/// is all items; in mode `full`, the note as it is: nothing is removed
/// but a byte order mark, which is no part of a note's text.
pub(super) fn context(text: &str, mode: ContextMode) -> Option<String> {
    match mode {
        ContextMode::Prose | ContextMode::None => None,
        ContextMode::Full => unless_blank(text.strip_prefix('\u{feff}').unwrap_or(text).to_owned()),
    }
}

/// The edit that writes item `item` of `text` again, as an item of
/// `entries` and `fields`. Errors are plain reasons.
pub(super) fn replace(
    text: &str,
    item: usize,
    entries: &Entries,
    fields: &[Field],
) -> Result<Edit, String> {
    let (line, _) = item_at(text, item).ok_or_else(|| format!("the note has no item {item}"))?;
    Ok(Edit::one(line, written(entries, fields)))
}

/// What `srl add` puts between two items: a line ending, since JSON lines
/// follow each other.
const GAP: &str = "\n";

/// The lines of field `field` of `item`, as `srl lint` looks at them:
/// the lines of the field's text, all named by the item's line, since
/// JSON writes a field on its item's line.
fn lines_of_field(item: &ParsedItem, field: usize) -> Vec<(usize, String)> {
    item.fields.get(field).map_or_else(Vec::new, |field| {
        field
            .text
            .lines()
            .map(|line| (item.line, line.to_owned()))
            .collect()
    })
}

/// The notes of a `jsonl` source.
#[derive(Debug)]
pub(super) struct Jsonl {
    files: NoteFiles,
}

impl Jsonl {
    pub(super) fn new(files: NoteFiles) -> Self {
        Self { files }
    }

    /// The note at `path` as it is now, if it exists, with `changes` made
    /// to it and checked, and the text of the items added. Each change
    /// comes with its place among all the changes. A changed item's line
    /// is written again.
    fn changed(
        &self,
        path: &str,
        changes: &[(usize, &Change)],
    ) -> Result<(Option<String>, String, String), Refusal> {
        let old = self.files.text_if_exists(path)?;
        let parsed = parse(old.as_deref().unwrap_or_default());
        if !parsed.errors.is_empty() {
            return Err(changed_meanwhile(path));
        }
        let mut expected = keyed(parsed);
        let mut text = old.clone().unwrap_or_default();
        let mut added = Vec::new();
        for &(at, change) in changes {
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
                        let edit = replace(&text, position, &after.entries, &after.fields)
                            .map_err(|reason| Refusal::Change { at, reason })?;
                        text = apply(&text, &edit);
                    }
                    let slot = expected
                        .get_mut(position)
                        .expect("the position of an item should be among the note's items");
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
            format!("{}\n", added.join(GAP))
        };
        let new = append(&text, &added, GAP);
        let parsed = parse(&new);
        verified(path, parsed.errors.is_empty() && keyed(parsed) == expected)?;
        Ok((old, new, added))
    }
}

impl Source for Jsonl {
    fn collections(&self) -> crate::Result<Vec<Collection>> {
        self.files.collections()
    }

    fn read(&self) -> crate::Result<Read> {
        let mut read = Read::default();
        for path in self.files.paths()? {
            add_read(&mut read, &path, parse(&self.files.text(&path)?));
        }
        Ok(read)
    }

    /// A JSONL note is no context of its own items, and is the context of
    /// the items that name it with `see` only in mode `full`, as it is.
    fn context(&self, requests: &[ContextRequest<'_>]) -> crate::Result<Vec<Option<String>>> {
        requests
            .iter()
            .map(|request| {
                if request.own {
                    return Ok(None);
                }
                Ok(context(&self.files.text(request.collection)?, request.mode))
            })
            .collect()
    }

    fn refuses(&self, collection: &str) -> Option<String> {
        self.files.refuses(collection)
    }

    /// Every note is changed and checked before the first one is written.
    fn write(&self, changes: &[Change], dry_run: bool) -> Result<String, Refusal> {
        let mut appended = String::new();
        let mut written = Vec::new();
        for (path, changes) in by_collection(changes) {
            let (old, new, added) = self.changed(path, &changes)?;
            appended.push_str(&added);
            written.push((path, old, new));
        }
        self.files.write_all(written, dry_run)?;
        Ok(appended)
    }

    /// A JSONL note holds nothing but items, so only the `Q` of its `qa`
    /// items is looked at, each finding at its item's line.
    fn lint(&self) -> crate::Result<Vec<Finding>> {
        let mut found = Vec::new();
        for path in self.files.paths()? {
            let text = self.files.text(&path)?;
            found.extend(lint_text(&text).into_iter().map(|(line, message)| Finding {
                collection: path.clone(),
                line,
                message,
            }));
        }
        Ok(found)
    }
}

/// The suspicious lines of a JSONL note whose text is `text`: the `qa`
/// kind's, in the `Q` of its items, each at its item's line.
fn lint_text(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for item in parse(text).items {
        if item
            .entries
            .kind
            .as_deref()
            .is_some_and(|kind| kind != "qa")
        {
            continue;
        }
        if let Some(question) = item.fields.iter().position(|field| field.key == "Q") {
            found.extend(question_suspicions(lines_of_field(&item, question)));
        }
    }
    found.sort_by_key(|(line, _)| *line);
    found
}

/// The lines of `text` that are not blank, with their numbers.
fn item_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    lines(text)
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
    use crate::note::ItemId;

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
    }

    #[test]
    fn edits_write_the_line_of_the_item_again() {
        let text = "{\"Q\": \"q\",  \"draft\": true}\n\n{\"id\": \"abc\", \"Q\": \"r\"}\n";
        let fields = [Field {
            key: "Q".to_owned(),
            text: "new".to_owned(),
        }];
        let entries = Entries {
            id: Some(ItemId::parse("abc").unwrap()),
            ..Entries::default()
        };
        assert_eq!(
            replace(text, 1, &entries, &fields).unwrap(),
            Edit::one(3, r#"{"id":"abc","Q":"new"}"#.to_owned())
        );
        assert!(replace(text, 2, &entries, &fields).is_err());
    }

    #[test]
    fn a_jsonl_note_is_given_whole_in_mode_full_only() {
        let text = "\n{\"Q\": \"q\"}\n\n";
        assert_eq!(context(text, ContextMode::Full).as_deref(), Some(text));
        assert_eq!(context(text, ContextMode::Prose), None);
        assert_eq!(context("\n  \n", ContextMode::Full), None);
        // A byte order mark is the one thing left out.
        let marked = "\u{feff}{\"Q\": \"q\"}\r\n";
        assert_eq!(
            context(marked, ContextMode::Full).as_deref(),
            Some("{\"Q\": \"q\"}\r\n")
        );
    }

    #[test]
    fn a_fields_lines_are_named_by_the_items_line() {
        let text = "\n{\"Q\": \"a\\nb\", \"A\": \"c\"}\n";
        let item = &parse(text).items[0];
        assert_eq!(
            lines_of_field(item, 0),
            [(2, "a".to_owned()), (2, "b".to_owned())]
        );
    }

    #[test]
    fn only_questions_are_linted_at_their_items_line() {
        let text = "{\"id\": \"abc\", \"Q\": \"{{c1::fine}}\"}\n\n{\"id\": \"abd\", \"Q\": \"{{C1::upper}}\", \"A\": \"Q: a\"}\n";
        let found = lint_text(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 3);
    }
}
