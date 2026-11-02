//! `srl check` and `srl lint` (see `docs/cli.md`, "Checking").

use std::collections::HashSet;

use serde::Serialize;

use crate::kind::{has_near_deletion, has_revealing_hint};
use crate::source::{Finding, Read, Sources};
use crate::store::nfc;
use crate::{CardId, Catalog, Config, Error, Memories, Result, Store};

/// Every orphan: an ID with reviews in the log, not moved away, but no
/// card in the store. When it is the only orphan of an item that still
/// exists, and that item has exactly one card with no events, the notice
/// says how to move the history there (see `docs/scheduling.md`, "Moving
/// history").
pub fn orphans(notes: &Catalog, memories: &Memories) -> Vec<Error> {
    let cards = notes.cards().map(|(_, card)| card).collect::<HashSet<_>>();
    let mut orphans = memories
        .all()
        .filter(|(id, memory)| memory.seen.is_some() && !cards.contains(id))
        .map(|(id, _)| id.clone())
        .collect::<Vec<CardId>>();
    orphans.sort();
    orphans
        .iter()
        .map(|id| {
            let alone = orphans
                .iter()
                .filter(|other| other.item() == id.item())
                .count()
                == 1;
            let fresh = notes
                .item(id.item())
                .map(|item| {
                    item.cards()
                        .iter()
                        .filter(|card| !memories.get(card).touched)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let suggestion = match fresh.as_slice() {
                [card] if alone => {
                    format!("{card} is new. To keep the history: srl move {id} {card}")
                }
                _ => "its history stays in the log; give it to a card with `srl move` if you \
                      want to keep it"
                    .to_owned(),
            };
            Error::Orphan {
                id: id.to_string(),
                suggestion,
            }
        })
        .collect()
}

/// Something `srl lint` found: a likely mistake, never an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lint {
    /// The address of the collection it is in.
    pub address: String,
    pub line: usize,
    pub message: String,
}

/// `srl lint`: asks every source for likely mistakes, adds those of the
/// `qa` kind in the items they read, and gives them in
/// the order of their addresses and lines (see `docs/cli.md`,
/// "Checking"). These are guesses, so they are never errors.
pub fn lint(store: &Store, config: &Config) -> Result<Vec<Lint>> {
    let mut found = Vec::new();
    for mounted in Sources::new(store, config).all() {
        let findings = mounted
            .source
            .lint()?
            .into_iter()
            .chain(qa_lints(&mounted.source.read()?));
        found.extend(findings.map(|finding| Lint {
            address: mounted.address(&finding.collection),
            line: finding.line,
            message: finding.message,
        }));
    }
    found.sort_by_cached_key(|lint| (nfc(&lint.address), lint.address.clone(), lint.line));
    Ok(found)
}

/// The likely mistakes in the `Q` of a `qa` item, each once, in the order
/// of the lines of `Q` they are first found on.
fn question_suspicions(question: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    for line in question.lines() {
        let message = if has_near_deletion(line) {
            Some(
                "a line of Q looks like a cloze deletion but isn't one, so it is shown as \
                 written; write `{{c1::text}}`, with a lowercase `c` and two colons",
            )
        } else if has_revealing_hint(line) {
            Some(
                "a cloze deletion of Q has a hint that is the text it hides, so the prompt \
                 shows the answer; change the hint or remove it",
            )
        } else {
            None
        };
        if let Some(message) = message
            && !found.contains(&message)
        {
            found.push(message);
        }
    }
    found
}

/// The likely mistakes the `qa` kind finds in the items read from a
/// source, at each item's line: in the `Q` of its items whose kind is `qa`
/// (see `docs/cli.md`, "Checking").
fn qa_lints(read: &Read) -> Vec<Finding> {
    read.items
        .iter()
        .filter(|item| {
            item.item
                .entries
                .kind
                .as_deref()
                .is_none_or(|kind| kind == "qa")
        })
        .flat_map(|item| {
            let question = item
                .item
                .fields
                .iter()
                .find(|field| field.key == "Q")
                .map_or("", |field| field.text.as_str());
            question_suspicions(question)
                .into_iter()
                .map(|message| Finding {
                    collection: item.collection.clone(),
                    line: item.line,
                    message: message.to_owned(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ReadItem;

    fn read(text: &str) -> Read {
        let parsed = crate::source::parse("n.md", text);
        Read {
            items: parsed
                .items
                .into_iter()
                .map(|item| ReadItem {
                    collection: "n.md".to_owned(),
                    handle: String::new(),
                    line: item.line,
                    written: item.text,
                    item: crate::domain::note::object::Object {
                        entries: item.entries,
                        fields: item.fields,
                    },
                })
                .collect(),
            problems: Vec::new(),
        }
    }

    #[test]
    fn mistaken_deletions_in_questions_are_found_once_each_at_their_item() {
        let text = "```srl\n@ id:abc\nQ: {{C1::upper}}\n  {{c1:one colon}}\n  {{c1::Paris::Paris}}\nA: {{C1::in the answer}}\n@ id:abd kind:words\nQ: {{C1::other kind}}\n```\n";
        let found = qa_lints(&read(text));
        let lines = found.iter().map(|finding| finding.line).collect::<Vec<_>>();
        assert_eq!(lines, [2, 2]);
        assert!(found[0].message.contains("looks like a cloze deletion"));
        assert!(found[1].message.contains("hint that is the text it hides"));
    }
}
