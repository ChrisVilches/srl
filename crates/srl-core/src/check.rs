//! `srl check` and `srl lint` (see `docs/cli.md`, "Checking").

use std::collections::HashSet;
use std::path;

use serde::Serialize;

use crate::note::parse::{self, Line};
use crate::note::read_note;
use crate::{CardId, Config, Error, Memories, Notes, Result, Store};

/// The longest full path, in UTF-16 units, that Windows allows without long
/// paths (260 including a final NUL).
const MAX_PATH: usize = 259;

/// The notes whose full path is longer than [`MAX_PATH`], as problems.
///
// TODO(platform): Not run on Windows yet. `srl` itself copes with longer
// paths there (Rust's standard library uses the long path form), so the
// limit is for other programs. See docs/cli.md, "Checking".
pub fn long_paths(store: &Store, notes: &Notes) -> Result<Vec<Error>> {
    let root = path::absolute(store.root()).map_err(Error::io(store.root()))?;
    // Lossy only for counting: a part that isn't valid Unicode becomes one
    // replacement character. Such names only occur on Unix, where the limit
    // doesn't apply anyway.
    let used = root.to_string_lossy().encode_utf16().count() + 1;
    Ok(notes
        .paths()
        .iter()
        .filter(|path| used + path.encode_utf16().count() > MAX_PATH)
        .map(|path| Error::PathOverLimit {
            path: path.clone(),
            limit: MAX_PATH,
        })
        .collect())
}

/// Every orphan: an ID with reviews in the log, not moved away, but no
/// card in the store. When its item still exists with exactly one card that
/// has no events, the notice says how to move the history there (see
/// `docs/scheduling.md`, "Moving history").
pub fn orphans(notes: &Notes, memories: &Memories) -> Vec<Error> {
    let cards = notes
        .cards()
        .map(|(_, card)| card.id())
        .collect::<HashSet<_>>();
    let mut orphans = memories
        .all()
        .filter(|(id, memory)| memory.seen.is_some() && !cards.contains(id))
        .map(|(id, _)| id.clone())
        .collect::<Vec<CardId>>();
    orphans.sort();
    orphans
        .into_iter()
        .map(|id| {
            let fresh = notes
                .item(id.item())
                .map(|item| {
                    item.cards()
                        .iter()
                        .filter(|card| !memories.get(card.id()).touched)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let suggestion = match fresh.as_slice() {
                [card] => format!(
                    "{} is new. To keep the history: srl move {id} {}",
                    card.id(),
                    card.id()
                ),
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

/// Something `srl lint` found: probably meant to be an item, but isn't one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lint {
    pub path: String,
    pub line: usize,
    pub message: String,
}

/// `srl lint`: looks through every note for text that was probably meant
/// to be an item (see `docs/cli.md`, "Checking"). These are guesses, so
/// they are never errors.
pub fn lint(store: &Store, config: &Config) -> Result<Vec<Lint>> {
    let mut found = Vec::new();
    for path in store.note_paths(&config.notes.exclude_set())? {
        let text = read_note(store, &path)?;
        found.extend(lint_text(&text).into_iter().map(|(line, message)| Lint {
            path: path.clone(),
            line,
            message: message.to_owned(),
        }));
    }
    Ok(found)
}

/// The suspicious lines of one note, with their numbers (from 1).
fn lint_text(text: &str) -> Vec<(usize, &'static str)> {
    let parsed = parse::parse(text);
    parse::lines(text)
        .into_iter()
        .zip(parsed.lines)
        .enumerate()
        .filter(|(_, (_, kind))| *kind == Line::Text)
        .filter_map(|(index, (line, _))| suspicion(line).map(|message| (index + 1, message)))
        .collect()
}

/// Why `line`, outside items, looks like part of one.
fn suspicion(line: &str) -> Option<&'static str> {
    let bare = without_markers(line);
    if parse::header_start(bare).is_some() && parse::header_start(line).is_none() {
        return Some(
            "a header that isn't at column 0 is not read; move it to the start of the line",
        );
    }
    if bare.starts_with("Q:") || bare.starts_with("A:") {
        return Some(
            "this looks like a field of an item without a header; add `<!-- srl -->` on the line \
             before the question",
        );
    }
    if line.contains("{{c") && has_cloze(line) {
        return Some(
            "a cloze deletion outside an item is not studied; put it in the Q: of an item",
        );
    }
    None
}

/// `line` without leading spaces, list markers (`-`, `*`, `+`, `1.`) and
/// quote markers (`>`).
fn without_markers(line: &str) -> &str {
    let mut rest = line.trim_start();
    loop {
        let next = rest
            .strip_prefix('>')
            .or_else(|| {
                ["- ", "* ", "+ "]
                    .iter()
                    .find_map(|marker| rest.strip_prefix(marker))
            })
            .or_else(|| {
                let digits =
                    rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                (digits > 0)
                    .then(|| rest.get(digits..))
                    .flatten()
                    .and_then(|tail| tail.strip_prefix(". "))
            });
        match next {
            Some(stripped) => rest = stripped.trim_start(),
            None => return rest,
        }
    }
}

/// Whether `line` has `{{c`, a number and `::`.
fn has_cloze(line: &str) -> bool {
    line.match_indices("{{c").any(|(start, _)| {
        let rest = line.get(start + 3..).unwrap_or_default();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        digits > 0
            && rest
                .get(digits..)
                .is_some_and(|tail| tail.starts_with("::"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_finds_items_without_headers_and_misplaced_headers() {
        let text = "Q: bare\nA: bare\n- Q: in a list\n> A: quoted\n1. Q: numbered\n  <!-- srl -->\nSee {{c1::x}} here.\n<!-- srl id:abc -->\nQ: {{c1::fine}}\n\n```\nQ: in code\n```\nQuestion: not a field\n";
        let found = lint_text(text)
            .into_iter()
            .map(|(line, _)| line)
            .collect::<Vec<_>>();
        assert_eq!(found, [1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn markers_are_stripped() {
        assert_eq!(without_markers("  > - 12. Q: x"), "Q: x");
        assert_eq!(without_markers("-x"), "-x");
    }
}
