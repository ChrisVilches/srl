//! The item syntax: reading a note line by line (see `docs/notes.md`, "The
//! item syntax").
//!
//! The parser reports every problem it finds, with its line, and carries on,
//! so that one run lists all of them. What a line is (text, front-matter,
//! code, a header, part of an item) is kept for each line, for the context
//! and for `srl lint`.

use crate::note::ItemId;

/// What a line of a note is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Line {
    /// Ordinary text of the note.
    Text,
    /// The YAML front-matter, fences included.
    FrontMatter,
    /// A fenced code block of the note, fences included.
    Code,
    /// The header of an item.
    Header,
    /// A field, continuation or blank line of an item.
    Item,
}

/// The entries of a header (see `docs/notes.md`, "Header entries").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    pub(crate) id: Option<ItemId>,
    /// The kind's name, if `kind:` was given. Which kinds exist is checked
    /// later.
    pub(crate) kind: Option<String>,
    pub(crate) see: Option<String>,
    pub(crate) draft: bool,
}

/// Where one entry of a header is, in bytes of its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// A header and where its parts are on its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeaderLine {
    pub(crate) header: Header,
    /// The byte just after `srl`, where `srl id` adds `id:ID`.
    pub(crate) after_srl: usize,
    /// Where `draft` is, if it is there.
    pub(crate) draft: Option<Span>,
}

/// One field of an item: its key and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub text: String,
}

/// An item as written, before its kind turns it into cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawItem {
    pub(crate) header: Header,
    /// The line of the header, counting from 1.
    pub(crate) line: usize,
    /// The last line of the item, counting from 1.
    pub(crate) end: usize,
    pub(crate) fields: Vec<Field>,
}

/// A parsed note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) items: Vec<RawItem>,
    /// Each problem, with its line (counting from 1).
    pub(crate) errors: Vec<(usize, String)>,
    /// What each line is.
    pub(crate) lines: Vec<Line>,
}

/// The lines of `text`, without their line endings (`\n` or `\r\n`) and
/// without a UTF-8 byte order mark. A final line ending doesn't start an
/// empty line.
pub(crate) fn lines(text: &str) -> Vec<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

/// Parses a note.
pub(crate) fn parse(text: &str) -> Parsed {
    let lines = lines(text);
    let mut kinds = vec![Line::Text; lines.len()];
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut index = front_matter(&lines).map_or(0, |end| {
        kinds
            .iter_mut()
            .take(end + 1)
            .for_each(|kind| *kind = Line::FrontMatter);
        end + 1
    });
    let mut fence: Option<(char, usize)> = None;
    while let Some(line) = lines.get(index).copied() {
        if let Some((character, length)) = fence {
            set(&mut kinds, index, Line::Code);
            if closes_fence(line, character, length) {
                fence = None;
            }
            index += 1;
            continue;
        }
        if let Some(opened) = opens_fence(line) {
            fence = Some(opened);
            set(&mut kinds, index, Line::Code);
            index += 1;
            continue;
        }
        let header = match header(line) {
            None => {
                index += 1;
                continue;
            }
            Some(Err(reason)) => {
                errors.push((index + 1, reason));
                index += 1;
                continue;
            }
            Some(Ok(header)) => header.header,
        };
        if lines.get(index + 1).and_then(|next| field(next)).is_none() {
            errors.push((
                index + 1,
                "this header is not followed by a field line such as `Q: …` on the next line; \
                 add the fields, or remove the header"
                    .to_owned(),
            ));
            index += 1;
            continue;
        }
        set(&mut kinds, index, Line::Header);
        let body = item_body(&lines, index + 1, &mut errors);
        for kind in kinds.iter_mut().take(body.last + 1).skip(index + 1) {
            *kind = Line::Item;
        }
        items.push(RawItem {
            header,
            line: index + 1,
            end: body.last + 1,
            fields: body.fields,
        });
        index = body.last + 1;
    }
    Parsed {
        items,
        errors,
        lines: kinds,
    }
}

fn set(kinds: &mut [Line], index: usize, kind: Line) {
    if let Some(slot) = kinds.get_mut(index) {
        *slot = kind;
    }
}

/// The index of the closing `---` of the front-matter, if the note starts
/// with one that is closed.
fn front_matter(lines: &[&str]) -> Option<usize> {
    if lines.first() != Some(&"---") {
        return None;
    }
    lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, line)| **line == "---")
        .map(|(index, _)| index)
}

/// The character and length of the fence `line` opens, if it opens one:
/// three or more backticks or tildes after at most three spaces.
fn opens_fence(line: &str) -> Option<(char, usize)> {
    let rest = line.trim_start_matches(' ');
    if line.len() - rest.len() > 3 {
        return None;
    }
    let character = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let length = rest.chars().take_while(|c| *c == character).count();
    (length >= 3).then_some((character, length))
}

/// Whether `line` closes a fence of `length` `character`s.
fn closes_fence(line: &str, character: char, length: usize) -> bool {
    let rest = line.trim_start_matches(' ');
    if line.len() - rest.len() > 3 {
        return false;
    }
    let run = rest.chars().take_while(|c| *c == character).count();
    run >= length && rest.chars().skip(run).all(char::is_whitespace)
}

/// The byte after `srl` if `line` starts like a header: `<!--`, any number
/// of spaces, `srl`, then a space, `-->` or the end of the line.
pub(crate) fn header_start(line: &str) -> Option<usize> {
    let rest = line.strip_prefix("<!--")?;
    let after = rest.trim_start_matches(' ').strip_prefix("srl")?;
    (after.is_empty() || after.starts_with(' ') || after.starts_with("-->"))
        .then(|| line.len() - after.len())
}

/// The header on `line`: `None` if the line doesn't start like one, an error
/// if it starts like one but isn't valid.
pub(crate) fn header(line: &str) -> Option<Result<HeaderLine, String>> {
    let after_srl = header_start(line)?;
    Some(read_header(line, after_srl))
}

fn read_header(line: &str, after_srl: usize) -> Result<HeaderLine, String> {
    let tail = line.get(after_srl..).unwrap_or_default();
    let inner = tail
        .trim_end_matches(' ')
        .strip_suffix("-->")
        .ok_or_else(|| {
            "this header doesn't end with `-->` on the same line, so a renderer would hide what \
             follows; close it with `-->`, with nothing after it"
                .to_owned()
        })?;
    let mut header = Header {
        id: None,
        kind: None,
        see: None,
        draft: false,
    };
    let mut draft = None;
    let mut seen = Vec::new();
    for (key, value, span) in entries(inner)? {
        let span = Span {
            start: span.start + after_srl,
            end: span.end + after_srl,
        };
        if seen.contains(&key) {
            return Err(format!("the header has `{key}` twice; keep one"));
        }
        seen.push(key);
        match (key, value) {
            ("draft", None) => {
                header.draft = true;
                draft = Some(span);
            }
            ("draft", Some(_)) => {
                return Err("`draft` is a flag and takes no value; write `draft`".to_owned());
            }
            (_, Some("")) => return Err(format!("`{key}:` has no value; give one or remove it")),
            ("id", Some(value)) => {
                header.id = Some(ItemId::parse(value).map_err(|error| error.to_string())?);
            }
            ("kind", Some(value)) => header.kind = Some(value.to_owned()),
            ("see", Some(value)) => header.see = Some(value.to_owned()),
            ("id" | "kind" | "see", None) => {
                return Err(format!("`{key}` needs a value, as in `{key}:…`"));
            }
            (other, _) => {
                return Err(format!(
                    "`{other}` is not a header entry; use id:, kind:, see: or draft"
                ));
            }
        }
    }
    Ok(HeaderLine {
        header,
        after_srl,
        draft,
    })
}

/// One entry of a header: its key, its value (none for a flag), and where
/// it is.
type Entry<'a> = (&'a str, Option<&'a str>, Span);

/// The entries of a header's inside: each `key:value` or flag, with where
/// it is. Values in double quotes may contain spaces.
fn entries(inner: &str) -> Result<Vec<Entry<'_>>, String> {
    let bytes = inner.as_bytes();
    let at = |index: usize| bytes.get(index).copied();
    let text = |start: usize, end: usize| inner.get(start..end).unwrap_or_default();
    let mut entries = Vec::new();
    let mut index = 0;
    loop {
        while at(index) == Some(b' ') {
            index += 1;
        }
        if index >= bytes.len() {
            return Ok(entries);
        }
        let start = index;
        while at(index).is_some_and(|byte| byte != b' ' && byte != b':') {
            index += 1;
        }
        let key = text(start, index);
        if key.is_empty() {
            return Err("the header has an entry without a name before `:`".to_owned());
        }
        let value = if at(index) == Some(b':') {
            index += 1;
            if at(index) == Some(b'"') {
                index += 1;
                let value_start = index;
                while at(index).is_some_and(|byte| byte != b'"') {
                    index += 1;
                }
                if at(index).is_none() {
                    return Err(format!("the quoted value of `{key}:` is never closed"));
                }
                let value = text(value_start, index);
                index += 1;
                if at(index).is_some_and(|byte| byte != b' ') {
                    return Err(format!(
                        "the closing `\"` of `{key}:` must be followed by a space"
                    ));
                }
                Some(value)
            } else {
                let value_start = index;
                while at(index).is_some_and(|byte| byte != b' ') {
                    index += 1;
                }
                Some(text(value_start, index))
            }
        } else {
            None
        };
        entries.push((key, value, Span { start, end: index }));
    }
}

/// The key and the rest of a field line: an uppercase key at column 0, a
/// colon, then a space or the end of the line.
pub(crate) fn field(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once(':')?;
    let mut characters = key.chars();
    let valid = characters.next().is_some_and(|c| c.is_ascii_uppercase())
        && characters.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !valid {
        return None;
    }
    if rest.is_empty() {
        Some((key, rest))
    } else {
        rest.strip_prefix(' ').map(|text| (key, text))
    }
}

/// Whether `key` can name a field.
pub(crate) fn is_field_key(key: &str) -> bool {
    field(&format!("{key}:")).is_some()
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// The fields of an item whose first field is on line `first` (an index),
/// and the index of its last line.
struct Body {
    fields: Vec<Field>,
    last: usize,
}

fn item_body(lines: &[&str], first: usize, errors: &mut Vec<(usize, String)>) -> Body {
    let mut fields: Vec<(String, Vec<String>)> = Vec::new();
    let mut last = first;
    let mut index = first;
    while let Some(line) = lines.get(index).copied() {
        if let Some((key, rest)) = field(line) {
            fields.push((key.to_owned(), vec![rest.to_owned()]));
            last = index;
            index += 1;
            continue;
        }
        if is_blank(line) {
            let next = (index..lines.len()).find(|next| {
                lines
                    .get(*next)
                    .is_some_and(|candidate| !is_blank(candidate))
            });
            let Some(next) = next else {
                break;
            };
            let next_line = lines.get(next).copied().unwrap_or_default();
            if field(next_line).is_none() && !next_line.starts_with("  ") {
                break;
            }
            if next_line.starts_with("  ")
                && let Some((_, text)) = fields.last_mut()
            {
                text.extend((index..next).map(|_| String::new()));
            }
            index = next;
            continue;
        }
        if let Some(continued) = line.strip_prefix("  ") {
            if let Some((_, text)) = fields.last_mut() {
                text.push(continued.to_owned());
            }
            last = index;
            index += 1;
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            errors.push((
                index + 1,
                "this line of an item starts with one space or a tab; continuation lines start \
                 with exactly two spaces"
                    .to_owned(),
            ));
            last = index;
            index += 1;
            continue;
        }
        break;
    }
    let fields = fields
        .into_iter()
        .map(|(key, mut text)| {
            if text.first().is_some_and(String::is_empty) {
                text.remove(0);
            }
            while text.last().is_some_and(|line| is_blank(line)) {
                text.pop();
            }
            Field {
                key,
                text: text.join("\n"),
            }
        })
        .collect();
    Body { fields, last }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_pairs(item: &RawItem) -> Vec<(&str, &str)> {
        item.fields
            .iter()
            .map(|field| (field.key.as_str(), field.text.as_str()))
            .collect()
    }

    #[test]
    fn a_simple_item_is_read() {
        let parsed = parse("Prose.\n<!-- srl id:x7k2m9 -->\nQ: What?\nA: That.\n\nMore prose.\n");
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let [item] = parsed.items.as_slice() else {
            panic!("expected one item: {:?}", parsed.items);
        };
        assert_eq!(item.header.id.as_ref().unwrap().as_str(), "x7k2m9");
        assert_eq!((item.line, item.end), (2, 4));
        assert_eq!(field_pairs(item), [("Q", "What?"), ("A", "That.")]);
        assert_eq!(
            parsed.lines,
            [
                Line::Text,
                Line::Header,
                Line::Item,
                Line::Item,
                Line::Text,
                Line::Text
            ]
        );
    }

    #[test]
    fn headers_may_have_any_spacing() {
        for line in [
            "<!--srl-->",
            "<!-- srl -->",
            "<!--   srl   id:abc   -->",
            "<!-- srl id:abc-->  ",
        ] {
            let parsed = parse(&format!("{line}\nQ: q\nA: a\n"));
            assert!(parsed.errors.is_empty(), "{line}: {:?}", parsed.errors);
            assert_eq!(parsed.items.len(), 1, "{line}");
        }
    }

    #[test]
    fn lines_that_only_look_alike_are_text() {
        for line in [
            " <!-- srl -->",
            "<!-- srlx -->",
            "<!-- notes -->",
            "<!-- srl-notes -->",
        ] {
            let parsed = parse(&format!("{line}\nQ: q\n"));
            assert!(parsed.errors.is_empty(), "{line}: {:?}", parsed.errors);
            assert!(parsed.items.is_empty(), "{line}");
        }
    }

    #[test]
    fn bad_headers_are_errors() {
        for line in [
            "<!-- srl",
            "<!-- srl id:abc",
            "<!-- srl --> text",
            "<!-- srl darft -->",
            "<!-- srl id:abc id:abd -->",
            "<!-- srl id: -->",
            "<!-- srl id -->",
            "<!-- srl draft:yes -->",
            "<!-- srl id:ABC -->",
            "<!-- srl see:\"a b -->",
            "<!-- srl see:\"a\"b -->",
        ] {
            let parsed = parse(&format!("{line}\nQ: q\nA: a\n"));
            assert_eq!(parsed.errors.len(), 1, "{line}: {:?}", parsed.errors);
            assert_eq!(parsed.errors[0].0, 1, "{line}");
        }
    }

    #[test]
    fn entries_are_read_with_quotes() {
        let line = header("<!-- srl draft see:\"my notes/a b.md\" kind:qa id:abc -->")
            .unwrap()
            .unwrap();
        assert_eq!(line.header.see.as_deref(), Some("my notes/a b.md"));
        assert_eq!(line.header.kind.as_deref(), Some("qa"));
        assert!(line.header.draft);
        assert_eq!(line.after_srl, 8);
        assert_eq!(line.draft, Some(Span { start: 9, end: 14 }));
    }

    #[test]
    fn a_header_without_fields_is_an_error() {
        for text in [
            "<!-- srl -->\n",
            "<!-- srl -->\n\nQ: q\n",
            "<!-- srl -->\nprose\n",
        ] {
            let parsed = parse(text);
            assert_eq!(parsed.errors.len(), 1, "{text:?}");
            assert!(parsed.items.is_empty());
        }
    }

    #[test]
    fn continuations_and_blank_lines_belong_to_fields() {
        let text = "<!-- srl -->\nQ: Does this compile?\n  ```rust\n    let a = 1;\n  ```\n\nA:\n  No.\n\n  Really.\n\nAfter.\n";
        let parsed = parse(text);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let item = &parsed.items[0];
        assert_eq!(
            field_pairs(item),
            [
                ("Q", "Does this compile?\n```rust\n  let a = 1;\n```"),
                ("A", "No.\n\nReally.")
            ]
        );
        assert_eq!(item.end, 10);
        assert_eq!(parsed.lines[10], Line::Text);
        assert_eq!(parsed.lines[11], Line::Text);
    }

    #[test]
    fn an_item_ends_at_the_next_header_or_text() {
        let parsed = parse("<!-- srl -->\nQ: a\nA: b\n<!-- srl -->\nQ: c\nA: d\nText\n");
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].end, 3);
        assert_eq!(parsed.items[1].line, 4);
        assert_eq!(parsed.items[1].end, 6);
    }

    #[test]
    fn one_space_or_a_tab_inside_an_item_is_an_error() {
        for line in [" x", "\tx", "   "] {
            let parsed = parse(&format!("<!-- srl -->\nQ: q\n{line}\nA: a\n"));
            let expected = usize::from(!line.trim().is_empty());
            assert_eq!(
                parsed.errors.len(),
                expected,
                "{line:?}: {:?}",
                parsed.errors
            );
            assert_eq!(parsed.items[0].fields.len(), 2, "{line:?}");
        }
    }

    #[test]
    fn code_blocks_and_front_matter_are_skipped() {
        let text = "---\ntitle: <!-- srl -->\n---\n```markdown\n<!-- srl -->\nQ: q\n```\n~~~~\n<!-- srl\n~~~\n~~~~\n";
        let parsed = parse(text);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(parsed.items.is_empty());
        assert_eq!(parsed.lines[1], Line::FrontMatter);
        assert_eq!(parsed.lines[4], Line::Code);
        assert_eq!(parsed.lines[8], Line::Code);
    }

    #[test]
    fn crlf_and_a_bom_are_read() {
        let parsed = parse("\u{feff}<!-- srl -->\r\nQ: q\r\n  more\r\nA: a\r\n");
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert_eq!(
            field_pairs(&parsed.items[0]),
            [("Q", "q\nmore"), ("A", "a")]
        );
    }

    #[test]
    fn field_lines_have_an_uppercase_key_and_a_space() {
        assert_eq!(field("Q: x"), Some(("Q", "x")));
        assert_eq!(field("A:"), Some(("A", "")));
        assert_eq!(field("VERB_2: 動く"), Some(("VERB_2", "動く")));
        for line in ["q: x", "Q:x", " Q: x", "Note: x", "2A: x", ": x"] {
            assert_eq!(field(line), None, "{line}");
        }
    }
}
