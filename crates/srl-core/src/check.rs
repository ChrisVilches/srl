//! `srl check` and `srl lint` (see `docs/cli.md`, "Checking").

use std::collections::HashSet;

use serde::Serialize;

use crate::kind::{has_near_deletion, has_revealing_hint};
use crate::note::syntax::Syntax;
use crate::note::{lines, read_note};
use crate::{CardId, Config, Error, Memories, Notes, Result, Store};

/// Every orphan: an ID with reviews in the log, not moved away, but no
/// card in the store. When it is the only orphan of an item that still
/// exists, and that item has exactly one card with no events, the notice
/// says how to move the history there (see `docs/scheduling.md`, "Moving
/// history").
pub fn orphans(notes: &Notes, memories: &Memories) -> Vec<Error> {
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
    pub path: String,
    pub line: usize,
    pub message: String,
}

/// `srl lint`: looks through every note for likely mistakes (see
/// `docs/cli.md`, "Checking"). These are guesses, so they are never errors.
pub fn lint(store: &Store, config: &Config) -> Result<Vec<Lint>> {
    let mut found = Vec::new();
    for path in store.note_paths(&config.notes.include_set(), &config.notes.exclude_set())? {
        let text = read_note(store, &path)?;
        let syntax = Syntax::of(&path);
        found.extend(
            lint_text(syntax, &text)
                .into_iter()
                .map(|(line, message)| Lint {
                    path: path.clone(),
                    line,
                    message,
                }),
        );
    }
    Ok(found)
}

/// The suspicious lines of one note, with their numbers (from 1), in
/// order: the syntax's own (see [`Syntax::lint`]), then the `qa` kind's,
/// in the `Q` of its items.
fn lint_text(syntax: Syntax, text: &str) -> Vec<(usize, String)> {
    let lines = lines(text);
    let mut found = syntax.lint(text);
    for item in syntax.parse(text).items {
        if item
            .entries
            .kind
            .as_deref()
            .is_some_and(|kind| kind != "qa")
        {
            continue;
        }
        let question = item
            .fields
            .iter()
            .zip(&item.spans)
            .find(|(field, _)| field.key == "Q");
        let Some((field, span)) = question else {
            continue;
        };
        found.extend(match syntax {
            // The lines of the note that `Q` is written on.
            Syntax::Blocks => question_suspicions(
                lines
                    .iter()
                    .enumerate()
                    .take(span.last)
                    .skip(span.first - 1)
                    .map(|(index, line)| (index + 1, *line)),
            ),
            // `Q`'s text, all on the item's line.
            Syntax::Jsonl => question_suspicions(field.text.lines().map(|line| (item.line, line))),
        });
    }
    found.sort_by_key(|(line, _)| *line);
    found
}

/// The suspicious lines of the `Q` of a `qa` item, each with the line of
/// the note it is on.
fn question_suspicions<'a>(lines: impl Iterator<Item = (usize, &'a str)>) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in lines {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn found_lines(text: &str) -> Vec<usize> {
        lint_text(Syntax::Blocks, text)
            .into_iter()
            .map(|(line, _)| line)
            .collect()
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

    #[test]
    fn in_a_jsonl_note_only_questions_are_linted_at_their_items_line() {
        let text = "{\"id\": \"abc\", \"Q\": \"{{c1::fine}}\"}\n\n{\"id\": \"abd\", \"Q\": \"{{C1::upper}}\", \"A\": \"Q: a\"}\n";
        let found = lint_text(Syntax::Jsonl, text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 3);
    }
}
