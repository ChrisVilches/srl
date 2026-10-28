//! `srl check` and `srl lint` (see `docs/cli.md`, "Checking").

use std::collections::HashSet;

use serde::Serialize;

use crate::kind::{has_deletion, has_near_deletion, has_revealing_hint};
use crate::note::parse::{self, Line};
use crate::note::read_note;
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
        found.extend(lint_text(&text).into_iter().map(|(line, message)| Lint {
            path: path.clone(),
            line,
            message,
        }));
    }
    Ok(found)
}

/// The suspicious lines of one note, with their numbers (from 1), in
/// order.
fn lint_text(text: &str) -> Vec<(usize, String)> {
    let parsed = parse::parse(text);
    let lines = parse::lines(text);
    let mut found = Vec::new();
    // Outside items, one finding per paragraph: the lines after the first
    // suspicious one are usually the same mistake.
    let mut reported = false;
    for (index, (line, kind)) in lines.iter().zip(&parsed.lines).enumerate() {
        if *kind != Line::Text || line.trim().is_empty() {
            reported = false;
            continue;
        }
        // A line that starts like a header but isn't one is an error, which
        // `srl check` reports; the rest of its paragraph is its item.
        if parse::header_start(line).is_some() {
            reported = true;
            continue;
        }
        let after_item = index
            .checked_sub(1)
            .and_then(|previous| parsed.lines.get(previous))
            == Some(&Line::Item);
        if let Some(message) = suspicion(line, after_item).filter(|_| !reported) {
            found.push((index + 1, message.to_owned()));
            reported = true;
        }
    }
    for item in &parsed.items {
        if item.header.kind.as_deref().is_some_and(|kind| kind != "qa") {
            continue;
        }
        found.extend(question_suspicions(&lines, item.line, item.end));
    }
    if let Some(open) = parsed.unclosed_fence {
        let hidden = lines
            .iter()
            .skip(open)
            .filter(|line| parse::header_start(line).is_some())
            .count();
        if hidden > 0 {
            found.push((
                open,
                format!(
                    "this code block is never closed, so the {hidden} header(s) after it are not \
                     read; close it with a line of the same fence"
                ),
            ));
        }
    }
    found.sort_by_key(|(line, _)| *line);
    found
}

/// The suspicious lines of the `Q` of a `qa` item whose header is on line
/// `header` and whose last line is `end` (counting from 1).
fn question_suspicions(lines: &[&str], header: usize, end: usize) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut in_question = false;
    for (index, line) in lines.iter().enumerate().take(end).skip(header) {
        if let Some((key, _)) = parse::field(line) {
            in_question = key == "Q";
        }
        let message = if !in_question {
            None
        } else if has_near_deletion(line) {
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
        found.extend(message.map(|message| (index + 1, message.to_owned())));
    }
    found
}

/// Why `line`, outside items, looks like part of one. `after_item` says
/// whether it comes right after the last line of an item.
fn suspicion(line: &str, after_item: bool) -> Option<&'static str> {
    let bare = without_markers(line);
    if parse::header_start(bare).is_some() {
        return Some(
            "a header that isn't at column 0 is not read; move it to the start of the line",
        );
    }
    if is_near_header(bare) {
        return Some(
            "this looks like a misspelled header, so the item below it is not read; write \
             `<!-- srl -->`, in lowercase, at the start of the line",
        );
    }
    if let Some(rest) = bare.strip_prefix("Q:").or_else(|| bare.strip_prefix("A:")) {
        return Some(if rest.is_empty() || rest.starts_with(' ') {
            "this looks like a field of an item without a header; add `<!-- srl -->` on the line \
             before the question"
        } else {
            "this looks like a field, but a field's colon is followed by a space, as in `Q: …`"
        });
    }
    if let Some(rest) = bare.strip_prefix("q:")
        && (rest.is_empty() || rest.starts_with(' '))
    {
        return Some("this looks like a field, but field keys are uppercase: `Q:`, `A:`");
    }
    if ["Q：", "Ｑ:", "Ｑ："]
        .iter()
        .any(|prefix| bare.starts_with(prefix))
    {
        return Some(
            "this looks like a field written with full-width characters; write `Q: ` and `A: ` \
             in ASCII",
        );
    }
    if has_deletion(line) {
        return Some(
            "a cloze deletion outside an item is not studied; put it in the Q: of an item",
        );
    }
    if after_item && !line.starts_with('#') && !line.starts_with("<!--") {
        return Some(
            "this line right after an item is not part of it; to continue the field above, \
             start it with two spaces, and otherwise leave a blank line before it",
        );
    }
    None
}

/// Whether `bare` looks like a header with a mistake: an HTML comment,
/// however its dashes are written, whose first word is `srl` or `slr` in
/// any case.
fn is_near_header(bare: &str) -> bool {
    let dashes = ['-', '—', '–'];
    let Some(rest) = bare.strip_prefix("<!") else {
        return false;
    };
    let rest = rest.trim_start_matches(|c: char| c == ' ' || dashes.contains(&c));
    let length = rest.len()
        - rest
            .trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .len();
    let (Some(word), Some(after)) = (rest.get(..length), rest.get(length..)) else {
        return false;
    };
    (word.eq_ignore_ascii_case("srl") || word.eq_ignore_ascii_case("slr"))
        && (after.is_empty()
            || after.starts_with(' ')
            || after.trim_start_matches(dashes).starts_with('>'))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn found_lines(text: &str) -> Vec<usize> {
        lint_text(text).into_iter().map(|(line, _)| line).collect()
    }

    #[test]
    fn lint_finds_items_without_headers_and_misplaced_headers() {
        let text = "Q: bare\n\nA: bare\n\n- Q: in a list\n\n> A: quoted\n\n1. Q: numbered\n\n  <!-- srl -->\n\nSee {{c1::x}} here.\n\n<!-- srl id:abc -->\nQ: {{c1::fine}}\n\n```\nQ: in code\n```\nQuestion: not a field\n";
        assert_eq!(found_lines(text), [1, 3, 5, 7, 9, 11, 13]);
    }

    #[test]
    fn a_paragraph_is_reported_once() {
        assert_eq!(found_lines("Q: one\nA: two\nQ: three\n\nA: four\n"), [1, 5]);
        assert_eq!(found_lines("<!-- SRL -->\nQ: q\nA: a\n"), [1]);
    }

    #[test]
    fn misspelled_headers_are_found() {
        for line in [
            "<!-- SRL -->",
            "<!-- slr id:abc -->",
            "<!— srl —>",
            "<! -- srl -->",
            "<!--Srl-->",
            "- <!-- SRL -->",
        ] {
            assert_eq!(found_lines(&format!("{line}\n")), [1], "{line}");
        }
        for line in [
            "<!-- srlx -->",
            "<!-- srl-notes -->",
            "<!-- notes -->",
            "<!DOCTYPE html>",
        ] {
            assert!(found_lines(&format!("{line}\n")).is_empty(), "{line}");
        }
    }

    #[test]
    fn errors_are_left_to_check() {
        assert!(found_lines("<!-- srl -- >\nQ: q\nA: a\n").is_empty());
    }

    #[test]
    fn misspelled_fields_are_found() {
        for line in [
            "Q:no space",
            "q: lowercase",
            "Q：full width",
            "Ｑ: full width",
        ] {
            assert_eq!(found_lines(&format!("{line}\n")), [1], "{line}");
        }
        for line in ["a: an option", "A：dialogue", "q:x", "Qs: x"] {
            assert!(found_lines(&format!("{line}\n")).is_empty(), "{line}");
        }
    }

    #[test]
    fn text_right_after_an_item_is_found() {
        let item = "<!-- srl id:abc -->\nQ: q\nA: first\n";
        assert_eq!(found_lines(&format!("{item}second\nthird\n")), [4]);
        assert_eq!(
            found_lines(&format!("{item}  second\n\nprose\n")),
            Vec::<usize>::new()
        );
        assert!(found_lines(&format!("{item}# Heading\n")).is_empty());
        assert!(found_lines(&format!("{item}<!-- a comment -->\n")).is_empty());
        assert!(found_lines(&format!("{item}\nprose\n")).is_empty());
    }

    #[test]
    fn mistaken_deletions_in_questions_are_found() {
        let text = "<!-- srl id:abc -->\nQ: {{C1::upper}}\n  {{c1:one colon}}\n  {{c1::Paris::Paris}}\nA: {{C1::in the answer}}\n<!-- srl id:abd kind:words -->\nQ: {{C1::other kind}}\n";
        assert_eq!(found_lines(text), [2, 3, 4]);
    }

    #[test]
    fn headers_in_an_unclosed_fence_are_found() {
        assert_eq!(found_lines("Prose.\n```\n<!-- srl -->\nQ: q\nA: a\n"), [2]);
        assert!(found_lines("```\ncode\n").is_empty());
        assert!(found_lines("```\n<!-- srl -->\n```\n").is_empty());
    }

    #[test]
    fn markers_are_stripped() {
        assert_eq!(without_markers("  > - 12. Q: x"), "Q: x");
        assert_eq!(without_markers("-x"), "-x");
    }
}
