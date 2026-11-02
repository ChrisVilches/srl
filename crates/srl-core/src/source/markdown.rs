//! The `markdown` source: notes whose items are srl blocks (see
//! `docs/notes.md`, "The srl block syntax"). How items are written is
//! [`blocks`]'s business; this module reads and changes notes with it.
//!
//! Every change edits lines, never data: a change of `id` or `draft` alone
//! changes the header line, and any other change replaces the item. The
//! item to change must still be the one read, and the new text is parsed
//! again and compared with the old before it is written (see
//! `docs/rust.md`, "Note editing").

mod blocks;

use crate::Result;
use crate::check::question_suspicions;
use crate::note::Entries;
use crate::note::object::Object;
use crate::source::files::{
    Edit, Keyed, NoteFiles, add_read, append, apply, by_collection, changed_meanwhile, keyed,
    position_of, verified,
};
use crate::source::{Change, Collection, ContextRequest, Finding, Read, Refusal, Source};

/// The notes of a `markdown` source.
#[derive(Debug)]
pub(super) struct Markdown {
    files: NoteFiles,
}

impl Markdown {
    pub(super) fn new(files: NoteFiles) -> Self {
        Self { files }
    }

    /// The note at `path` as it is now, if it exists, with `changes` made
    /// to it and checked, and the text of the items added. Each change
    /// comes with its place among all the changes.
    fn changed(
        &self,
        path: &str,
        changes: &[(usize, &Change)],
    ) -> std::result::Result<(Option<String>, String, String), Refusal> {
        let old = self.files.text_if_exists(path)?;
        let parsed = blocks::parse(old.as_deref().unwrap_or_default());
        // Commands that write refuse notes with errors, so a note that has
        // some now changed since it was read.
        if !parsed.errors.is_empty() {
            return Err(changed_meanwhile(path));
        }
        let mut expected = keyed(parsed);
        let mut text = old.clone().unwrap_or_default();
        let mut added = Vec::new();
        for &(at, change) in changes {
            let refused = |reason: String| Refusal::Change { at, reason };
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
                    let (edit, item) =
                        replacement(&text, position, before, after).map_err(refused)?;
                    if let Some(edit) = edit {
                        text = apply(&text, &edit);
                    }
                    let slot = expected
                        .get_mut(position)
                        .expect("the position of an item should be among the note's items");
                    *slot = item;
                }
                Change::Add { item, .. } => {
                    let formatted = blocks::format(&item.entries, &item.fields).map_err(refused)?;
                    expected.extend(keyed(blocks::parse(&formatted)));
                    added.push(formatted);
                }
            }
        }
        let added = if added.is_empty() {
            String::new()
        } else {
            format!("{}\n", added.join(blocks::GAP))
        };
        let new = append(&text, &added, blocks::GAP);
        let parsed = blocks::parse(&new);
        verified(path, parsed.errors.is_empty() && keyed(parsed) == expected)?;
        Ok((old, new, added))
    }
}

impl Source for Markdown {
    fn collections(&self) -> Result<Vec<Collection>> {
        self.files.collections()
    }

    fn read(&self) -> Result<Read> {
        let mut read = Read::default();
        for path in self.files.paths()? {
            add_read(&mut read, &path, blocks::parse(&self.files.text(&path)?));
        }
        Ok(read)
    }

    /// A note shows the same to its own items and to the items that name
    /// it with `see`: itself, without its srl blocks in mode `prose`.
    fn context(&self, requests: &[ContextRequest<'_>]) -> Result<Vec<Option<String>>> {
        requests
            .iter()
            .map(|request| {
                Ok(blocks::context(
                    &self.files.text(request.collection)?,
                    request.mode,
                ))
            })
            .collect()
    }

    fn refuses(&self, collection: &str) -> Option<String> {
        self.files.refuses(collection)
    }

    /// Every note is changed and checked before the first one is written.
    fn write(&self, changes: &[Change], dry_run: bool) -> std::result::Result<String, Refusal> {
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

    /// The likely mistakes in every note, in the order of the notes and
    /// their lines: those of srl blocks, then the `qa` kind's, in the `Q` of
    /// its items, on the lines `Q` is written on.
    fn lint(&self) -> Result<Vec<Finding>> {
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

/// The edit that turns item `position` of `text`, read as `before`, into
/// `after`, if anything changes, and the item as it reads after the edit. A
/// change of `id` or `draft` alone changes the header only; any other
/// change replaces the item. Errors are plain reasons.
fn replacement(
    text: &str,
    position: usize,
    before: &Object,
    after: &Object,
) -> std::result::Result<(Option<Edit>, Keyed), String> {
    let unchanged = (after.entries.clone(), after.fields.clone());
    if before == after {
        return Ok((None, unchanged));
    }
    let only = |entries: Entries| before.fields == after.fields && entries == after.entries;
    let undrafted = Entries {
        draft: false,
        ..before.entries.clone()
    };
    let header = match (&before.entries.id, &after.entries.id) {
        (None, Some(id))
            if only(Entries {
                id: Some(id.clone()),
                ..before.entries.clone()
            }) =>
        {
            Some(blocks::with_id(text, position, id))
        }
        _ if before.entries.draft && only(undrafted) => Some(blocks::without_draft(text, position)),
        _ => None,
    };
    let Some(header) = header else {
        let edit = blocks::replace(text, position, &after.entries, &after.fields)?;
        let formatted = blocks::format(&after.entries, &after.fields)?;
        let item = keyed(blocks::parse(&formatted))
            .into_iter()
            .next()
            .ok_or_else(|| {
                "it doesn't read back as an item (a bug in srl; please report it)".to_owned()
            })?;
        return Ok((Some(edit), item));
    };
    let edit = header.ok_or_else(|| {
        "the item is not where srl read it (a bug in srl; please report it)".to_owned()
    })?;
    Ok((Some(edit), unchanged))
}

/// The suspicious lines of a note whose text is `text`, with their numbers
/// (from 1), in order: those of srl blocks, then the `qa` kind's, in the
/// `Q` of its items, on the lines `Q` is written on.
fn lint_text(text: &str) -> Vec<(usize, String)> {
    let mut found = blocks::lint(text);
    for item in blocks::parse(text).items {
        if item
            .entries
            .kind
            .as_deref()
            .is_some_and(|kind| kind != "qa")
        {
            continue;
        }
        if let Some(question) = item.fields.iter().position(|field| field.key == "Q") {
            found.extend(question_suspicions(blocks::lines_of_field(
                text, &item, question,
            )));
        }
    }
    found.sort_by_key(|(line, _)| *line);
    found
}

/// The items of a note of srl blocks whose text is `text`.
#[cfg(test)]
pub(super) fn parse(text: &str) -> crate::source::files::Parsed {
    blocks::parse(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found_lines(text: &str) -> Vec<usize> {
        lint_text(text).into_iter().map(|(line, _)| line).collect()
    }

    #[test]
    fn mistaken_deletions_in_questions_are_found() {
        let text = "```srl\n@ id:abc\nQ: {{C1::upper}}\n  {{c1:one colon}}\n  {{c1::Paris::Paris}}\nA: {{C1::in the answer}}\n@ id:abd kind:words\nQ: {{C1::other kind}}\n```\n";
        assert_eq!(found_lines(text), [3, 4, 5]);
    }

    #[test]
    fn the_syntax_and_the_kind_are_both_linted_in_line_order() {
        let text = "Q: bare\n\n```srl id:abc\nQ: {{C1::upper}}\n```\n\n- ```srl\n";
        assert_eq!(found_lines(text), [1, 4, 7]);
    }
}
