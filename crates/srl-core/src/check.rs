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
    // A block closed by an indented fence was most likely closed by the
    // end of a code block in one of its fields, too early.
    let mut early = Vec::new();
    for block in parsed.items.iter().map(|item| &item.block) {
        let closing = lines.get(block.close - 1).copied().unwrap_or_default();
        if closing.starts_with(' ') && !early.contains(&block.close) {
            early.push(block.close);
            found.push((
                block.close,
                format!(
                    "this indented fence closes the srl block of line {}, probably as the end of \
                     a code block in a field, so the rest of the item is not part of it; open \
                     the block with ~~~srl, or with a longer fence than the code blocks in its \
                     fields",
                    block.open
                ),
            ));
        }
    }
    // Outside items, one finding per paragraph: the lines after the first
    // suspicious one are usually the same mistake. The paragraph right
    // after a block closed too early is the rest of its item.
    let mut reported = false;
    for (index, (line, kind)) in lines.iter().zip(&parsed.lines).enumerate() {
        if *kind != Line::Text || line.trim().is_empty() {
            reported = false;
            continue;
        }
        if early.contains(&index) {
            reported = true;
        }
        if let Some(message) = suspicion(line).filter(|_| !reported) {
            found.push((index + 1, message.to_owned()));
            reported = true;
        }
    }
    for open in &parsed.code_blocks {
        let line = lines.get(open - 1).copied().unwrap_or_default();
        found.extend(block_suspicion(line.trim_start()).map(|message| (*open, message.to_owned())));
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
            .filter(|line| parse::block_start(line).is_some())
            .count();
        if hidden > 0 {
            found.push((
                open,
                format!(
                    "this code block is never closed, so the {hidden} srl block(s) after it are \
                     not read; close it with a line of the same fence"
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

/// Why `line`, outside items, looks like part of one.
fn suspicion(line: &str) -> Option<&'static str> {
    let bare = without_markers(line);
    if let Some(message) = block_suspicion(bare) {
        return Some(message);
    }
    if let Some(rest) = bare.strip_prefix("Q:").or_else(|| bare.strip_prefix("A:")) {
        return Some(if rest.is_empty() || rest.starts_with(' ') {
            "this looks like a field of an item outside an srl block; put the item in a block \
             that starts with a line ```srl and ends with a line ```"
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
    None
}

/// Why `bare`, a line without its indentation or markers that is not an srl
/// block's opening fence, looks like one: it would be one at column 0, or
/// its fence is followed by `srl` or `slr` in any case, then nothing, a
/// space or a character that can't continue a word.
fn block_suspicion(bare: &str) -> Option<&'static str> {
    if parse::block_start(bare).is_some() {
        return Some(
            "an srl block that doesn't start at column 0 is not read; move its fence to the \
             start of the line",
        );
    }
    let fence = bare.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let rest = bare.trim_start_matches(fence);
    if bare.len() - rest.len() < 3 {
        return None;
    }
    let rest = rest.trim_start_matches(' ');
    let word = rest.get(..3)?;
    let after = rest.get(3..)?.chars().next();
    let near = (word.eq_ignore_ascii_case("srl") || word.eq_ignore_ascii_case("slr"))
        && after.is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'));
    near.then_some(
        "this looks like a misspelled srl block, so the items in it are not read; open it with \
         ```srl, in lowercase, followed by a space or the end of the line",
    )
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
    fn lint_finds_items_outside_blocks_and_misplaced_blocks() {
        let text = "Q: bare\n\nA: bare\n\n- Q: in a list\n\n> A: quoted\n\n1. Q: numbered\n\n- ```srl\n\nSee {{c1::x}} here.\n\n```srl id:abc\nQ: {{c1::fine}}\n```\n\n```\nQ: in code\n```\nQuestion: not a field\n";
        assert_eq!(found_lines(text), [1, 3, 5, 7, 9, 11, 13]);
    }

    #[test]
    fn a_paragraph_is_reported_once() {
        assert_eq!(found_lines("Q: one\nA: two\nQ: three\n\nA: four\n"), [1, 5]);
        assert_eq!(found_lines("- ```srl\nQ: q\nA: a\n"), [1]);
    }

    #[test]
    fn misspelled_and_indented_blocks_are_found() {
        for line in [
            "```SRL",
            "```slr id:abc",
            "~~~ Srl",
            "```srl{id:abc}",
            "  ```srl",
            "- ```SRL",
            "> ```srl",
        ] {
            assert_eq!(found_lines(&format!("{line}\n")), [1], "{line}");
        }
        for line in ["```srlx", "```srl-notes", "```rust", "``srl", "```"] {
            assert!(found_lines(&format!("{line}\n")).is_empty(), "{line}");
        }
    }

    #[test]
    fn errors_are_left_to_check() {
        assert!(found_lines("```srl darft\nQ: q\nA: a\n```\n").is_empty());
        assert!(found_lines("```srl\nQ: q\nA: a\nmore\n```\n").is_empty());
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
    fn mistaken_deletions_in_questions_are_found() {
        let text = "```srl\n@ id:abc\nQ: {{C1::upper}}\n  {{c1:one colon}}\n  {{c1::Paris::Paris}}\nA: {{C1::in the answer}}\n@ id:abd kind:words\nQ: {{C1::other kind}}\n```\n";
        assert_eq!(found_lines(text), [3, 4, 5]);
    }

    #[test]
    fn a_block_closed_by_a_code_block_in_a_field_is_found() {
        // The A: left outside is the rest of the item, not another finding.
        let lost_answer =
            "```srl id:aaa1\nQ: Does this compile?\n  ```rust\n  let b = a;\n  ```\nA: No.\n```\n";
        assert_eq!(found_lines(lost_answer), [5]);
        let cut_cloze = "```srl id:bbb1\nQ: {{c1::x}} here\n  ```rust\n  code\n  ```\n  more of Q\n```\n\n```srl id:ccc1\nQ: q\nA: a\n```\n";
        assert_eq!(found_lines(cut_cloze), [5]);
        let shared =
            "```srl\n@ id:ddd1\nQ: {{c1::y}}\n  ```\n  code\n  ```\n@ id:eee1\nQ: z\nA: a\n```\n";
        assert_eq!(found_lines(shared), [4]);
        let longer = "````srl id:fff1\nQ: q\n  ```\n  code\n  ```\nA: a\n````\n";
        assert!(found_lines(longer).is_empty());
        let tildes = "~~~srl id:fff2\nQ: q\n  ```\n  code\n  ```\nA: a\n~~~\n";
        assert!(found_lines(tildes).is_empty());
    }

    #[test]
    fn blocks_in_an_unclosed_fence_are_found() {
        assert_eq!(found_lines("Prose.\n~~~\n```srl\nQ: q\n```\n"), [2]);
        assert!(found_lines("```\ncode\n").is_empty());
        assert!(found_lines("~~~\n```srl\nQ: q\n```\n~~~\n").is_empty());
    }

    #[test]
    fn markers_are_stripped() {
        assert_eq!(without_markers("  > - 12. Q: x"), "Q: x");
        assert_eq!(without_markers("-x"), "-x");
    }
}
