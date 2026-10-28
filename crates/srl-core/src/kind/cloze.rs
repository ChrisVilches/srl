//! Cloze deletions, written as in Anki (see `docs/cards.md`, "Cloze
//! deletions").

/// One cloze deletion: `{{cN::text}}` or `{{cN::text::hint}}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Deletion<'a> {
    pub(crate) number: u32,
    pub(crate) text: &'a str,
    pub(crate) hint: Option<&'a str>,
}

/// A piece of a field's text: plain text, or a deletion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Piece<'a> {
    Text(&'a str),
    Deletion(Deletion<'a>),
}

/// Where the next deletion starts in `text`, from byte `from` on: `{{c`, a
/// number and `::`. Returns where it starts, the number as written and
/// where its text starts. Any other `{{` is ordinary text.
fn next_start(text: &str, from: usize) -> Option<(usize, &str, usize)> {
    let mut search = from;
    loop {
        let start = search + text.get(search..)?.find("{{c")?;
        let after = start + 3;
        let rest = text.get(after..).unwrap_or_default();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let number = rest.get(..digits).unwrap_or_default();
        if digits > 0
            && rest
                .get(digits..)
                .is_some_and(|tail| tail.starts_with("::"))
        {
            return Some((start, number, after + digits + 2));
        }
        search = start + 1;
    }
}

/// Splits `text` into plain text and deletions. Errors are plain reasons.
pub(crate) fn pieces(text: &str) -> Result<Vec<Piece<'_>>, String> {
    let mut pieces = Vec::new();
    let mut plain = 0;
    while let Some((start, number, inner)) = next_start(text, plain) {
        if start > plain {
            pieces.push(Piece::Text(text.get(plain..start).unwrap_or_default()));
        }
        let written = format!("{{{{c{number}::");
        if number.starts_with('0') {
            return Err(format!(
                "the cloze deletion `{written}` has the number {number}; numbers start at 1 and \
                 have no leading zeros"
            ));
        }
        let number = number
            .parse::<u32>()
            .map_err(|error| format!("the cloze number in `{written}` is not usable ({error})"))?;
        let close = inner
            + text
                .get(inner..)
                .and_then(|rest| rest.find("}}"))
                .ok_or_else(|| format!("the cloze deletion `{written}` has no closing `}}}}`"))?;
        let content = text.get(inner..close).unwrap_or_default();
        if next_start(content, 0).is_some() {
            return Err(format!(
                "the cloze deletion `{written}` has another deletion inside it; deletions can't \
                 be nested"
            ));
        }
        let (deleted, hint) = match content.split_once("::") {
            Some((deleted, hint)) => (deleted, Some(hint).filter(|hint| !hint.is_empty())),
            None => (content, None),
        };
        if deleted.trim().is_empty() {
            return Err(format!(
                "the cloze deletion `{written}` hides no text; write the text after `::`"
            ));
        }
        pieces.push(Piece::Deletion(Deletion {
            number,
            text: deleted,
            hint,
        }));
        plain = close + 2;
    }
    if plain < text.len() {
        pieces.push(Piece::Text(text.get(plain..).unwrap_or_default()));
    }
    Ok(pieces)
}

/// Whether `text` has a deletion, written correctly or not.
pub(crate) fn has_deletion(text: &str) -> bool {
    next_start(text, 0).is_some()
}

/// Whether `text` has something that was probably meant to start a
/// deletion but doesn't: `{{C1::` or `{{c1:` (one colon).
pub(crate) fn has_near_deletion(text: &str) -> bool {
    text.match_indices("{{").any(|(start, _)| {
        let rest = text.get(start + 2..).unwrap_or_default();
        let (upper, rest) = match rest.strip_prefix('C') {
            Some(rest) => (true, rest),
            None => (false, rest.strip_prefix('c').unwrap_or_default()),
        };
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let tail = rest.get(digits..).unwrap_or_default();
        digits > 0
            && if upper {
                tail.starts_with("::")
            } else {
                tail.starts_with(':') && !tail.starts_with("::")
            }
    })
}

/// Whether a deletion of `text` has a hint that is its text, so that the
/// prompt shows what it hides. A text with errors has none.
pub(crate) fn has_revealing_hint(text: &str) -> bool {
    pieces(text).is_ok_and(|pieces| {
        pieces.iter().any(|piece| match piece {
            Piece::Deletion(deletion) => deletion.hint.map(str::trim) == Some(deletion.text.trim()),
            Piece::Text(_) => false,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deletion(number: u32, text: &str, hint: Option<&str>) -> Piece<'static> {
        Piece::Deletion(Deletion {
            number,
            text: Box::leak(text.to_owned().into_boxed_str()),
            hint: hint.map(|hint| &*Box::leak(hint.to_owned().into_boxed_str())),
        })
    }

    #[test]
    fn deletions_and_text_alternate() {
        assert_eq!(
            pieces("{{c1::周囲の反対}}を{{c2::彼女::who}}は").unwrap(),
            [
                deletion(1, "周囲の反対", None),
                Piece::Text("を"),
                deletion(2, "彼女", Some("who")),
                Piece::Text("は"),
            ]
        );
    }

    #[test]
    fn other_braces_are_text() {
        let text = "format!(\"{{}}\") and {{x}} and {{c::y}} and {{cx::z}}";
        assert_eq!(pieces(text).unwrap(), [Piece::Text(text)]);
        assert!(!has_deletion(text));
    }

    #[test]
    fn bad_deletions_are_errors() {
        for text in [
            "{{c1::open",
            "{{c0::zero}}",
            "{{c01::lead}}",
            "{{c1::}}",
            "{{c1::  }}",
            "{{c1::a {{c2::b}} }}",
            "{{c99999999999::big}}",
        ] {
            assert!(pieces(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn near_deletions_are_found() {
        for text in ["a {{C1::x}}", "{{c1:x}}", "{{c12:x}}"] {
            assert!(has_near_deletion(text), "missed {text}");
        }
        for text in [
            "{{c1::x}}",
            "{{c::x}}",
            "{{C::x}}",
            "{{cx:y}}",
            "{{}}",
            "{{c1",
        ] {
            assert!(!has_near_deletion(text), "found {text}");
        }
    }

    #[test]
    fn hints_that_are_the_text_are_found() {
        assert!(has_revealing_hint("{{c1::Paris::Paris}}"));
        assert!(has_revealing_hint("a {{c1::x}} {{c2::Paris:: Paris }}"));
        assert!(!has_revealing_hint("{{c1::Paris::city}}"));
        assert!(!has_revealing_hint("{{c1::Paris}}"));
        assert!(!has_revealing_hint("{{c1::open"));
    }

    #[test]
    fn the_hint_is_after_the_first_double_colon_as_in_anki() {
        assert_eq!(
            pieces("{{c1::std::move}}").unwrap(),
            [deletion(1, "std", Some("move"))]
        );
        assert_eq!(pieces("{{c1::x::}}").unwrap(), [deletion(1, "x", None)]);
    }
}
