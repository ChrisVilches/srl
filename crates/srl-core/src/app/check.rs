//! What `srl check` finds besides the problems of the catalog: orphans
//! and likely mistakes (see `docs/cli.md`, "Checking").

use std::collections::HashSet;

use serde::Serialize;

use crate::kind::{Kind, has_near_deletion, has_revealing_hint};
use crate::{CardId, Catalog, Error, Memories};

/// Every orphan: an ID with reviews in the log but no card in the store
/// (see `docs/scheduling.md`, "Orphans").
pub fn orphans(catalog: &Catalog, memories: &Memories) -> Vec<Error> {
    let cards = catalog
        .cards()
        .map(|(_, card)| card)
        .collect::<HashSet<_>>();
    let mut orphans = memories
        .all()
        .filter(|(id, memory)| memory.seen.is_some() && !cards.contains(id))
        .map(|(id, _)| id.clone())
        .collect::<Vec<CardId>>();
    orphans.sort();
    orphans
        .iter()
        .map(|id| Error::Orphan { id: id.to_string() })
        .collect()
}

/// A likely mistake in an item, which `srl check` reports as a warning,
/// never an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Warning {
    /// The collection the item is in.
    pub collection: String,
    pub id: String,
    pub message: String,
}

/// The likely mistakes in the items of `catalog`, in their order: those
/// the `qa` kind finds in the `Q` of its items (see `docs/cli.md`,
/// "Checking"). These are guesses, so they are never errors.
pub fn warnings(catalog: &Catalog) -> Vec<Warning> {
    catalog
        .items()
        .iter()
        .filter(|item| matches!(item.kind(), Kind::Qa))
        .flat_map(|item| {
            let question = item
                .fields()
                .iter()
                .find(|field| field.key == "Q")
                .map_or("", |field| field.text.as_str());
            question_suspicions(question)
                .into_iter()
                .map(|message| Warning {
                    collection: item.collection().to_owned(),
                    id: item.id().to_string(),
                    message: message.to_owned(),
                })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mistaken_deletions_in_questions_are_found_once_each() {
        let found = question_suspicions(
            "{{C1::upper}}\n{{c1:one colon}}\n{{c1::Paris::Paris}}\n{{c1::fine}}",
        );
        assert_eq!(found.len(), 2);
        assert!(found[0].contains("looks like a cloze deletion"));
        assert!(found[1].contains("hint that is the text it hides"));
        assert_eq!(
            question_suspicions("{{c1::fine}} and plain text"),
            [] as [&str; 0]
        );
    }
}
