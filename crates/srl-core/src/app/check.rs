//! `srl check` and `srl lint` (see `docs/cli.md`, "Checking").

use std::collections::HashSet;

use serde::Serialize;

use crate::kind::{has_near_deletion, has_revealing_hint};
use crate::source::Sources;
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

/// `srl lint`: asks every source for likely mistakes, and gives them in
/// the order of their addresses and lines (see `docs/cli.md`,
/// "Checking"). These are guesses, so they are never errors.
pub fn lint(store: &Store, config: &Config) -> Result<Vec<Lint>> {
    let mut found = Vec::new();
    for mounted in Sources::new(store, config).all() {
        found.extend(mounted.source.lint()?.into_iter().map(|finding| Lint {
            address: mounted.address(&finding.collection),
            line: finding.line,
            message: finding.message,
        }));
    }
    found.sort_by_cached_key(|lint| (nfc(&lint.address), lint.address.clone(), lint.line));
    Ok(found)
}

/// The suspicious lines of the `Q` of a `qa` item, each with the line of
/// the note a message about it names.
pub(crate) fn question_suspicions(lines: Vec<(usize, String)>) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in lines {
        let line = line.as_str();
        let message = if has_near_deletion(line) {
            Some(
                "this looks like a cloze deletion but isn't one, so it is shown as written; \
                 write `{{c1::text}}`, with a lowercase `c` and two colons",
            )
        } else if has_revealing_hint(line) {
            Some(
                "this cloze deletion's hint is the text it hides, so the prompt shows the \
                 answer; change the hint or remove it",
            )
        } else {
            None
        };
        found.extend(message.map(|message| (number, message.to_owned())));
    }
    found
}
