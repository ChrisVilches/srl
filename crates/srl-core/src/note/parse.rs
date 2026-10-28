//! The item syntax: reading a note line by line (see `docs/notes.md`, "The
//! item syntax").
//!
//! The parser reports every problem it finds, with its line, and carries on,
//! so that one run lists all of them. What a line is (text, front-matter,
//! code, part of an srl block) is kept for each line, for the context and
//! for `srl lint`.

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
    /// A line of an srl block that is neither a header nor part of an item:
    /// its fences, and blank lines between items. Every line of a block
    /// with an error is one too.
    Block,
    /// The header of an item: an srl block's opening fence, or an `@` line.
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
    /// The entries, as written, without the spaces around them.
    pub(crate) entries: String,
    /// The byte just after `srl` or `@`, where `srl id` adds `id:ID`.
    pub(crate) mark_end: usize,
    /// Where `draft` is, if it is there.
    pub(crate) draft: Option<Span>,
}

/// One field of an item: its key and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub text: String,
}

/// An srl block: where it is, and the fence that opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Block {
    /// The backticks or tildes that open it: three or more of one of them.
    pub(crate) fence: String,
    /// The line of its opening fence, counting from 1.
    pub(crate) open: usize,
    /// The line of its closing fence, counting from 1.
    pub(crate) close: usize,
}

impl Block {
    /// Whether `line` would close the block.
    pub(crate) fn closed_by(&self, line: &str) -> bool {
        let character = self.fence.chars().next().unwrap_or('`');
        closes_fence(line, character, self.fence.chars().count())
    }
}

/// An item as written, before its kind turns it into cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawItem {
    pub(crate) header: Header,
    /// The header's entries, as written.
    pub(crate) entries: String,
    /// The line of the header, counting from 1: the block's opening fence
    /// when the item has the block to itself, otherwise its `@` line.
    pub(crate) line: usize,
    /// The last line of the item's fields, counting from 1.
    pub(crate) end: usize,
    pub(crate) fields: Vec<Field>,
    /// The block the item is in.
    pub(crate) block: Block,
}

impl RawItem {
    /// Whether the item's header is its block's opening fence, which it
    /// then has to itself.
    pub(crate) fn is_alone(&self) -> bool {
        self.line == self.block.open
    }

    /// The item as a block of its own, from the note's `lines`: an opening
    /// fence like its block's, with its entries, its field lines as
    /// written, and the closing fence.
    pub(crate) fn text(&self, lines: &[&str]) -> String {
        let fence = &self.block.fence;
        let opening = if self.entries.is_empty() {
            format!("{fence}srl")
        } else {
            format!("{fence}srl {}", self.entries)
        };
        std::iter::once(opening.as_str())
            .chain(
                lines
                    .get(self.line..self.end)
                    .unwrap_or_default()
                    .iter()
                    .copied(),
            )
            .chain(std::iter::once(fence.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A parsed note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) items: Vec<RawItem>,
    /// Each problem, with its line (counting from 1).
    pub(crate) errors: Vec<(usize, String)>,
    /// What each line is.
    pub(crate) lines: Vec<Line>,
    /// The lines (counting from 1) that open a fenced code block that is
    /// not an srl block.
    pub(crate) code_blocks: Vec<usize>,
    /// The line (counting from 1) that opens a fenced code block never
    /// closed, which runs to the end of the note.
    pub(crate) unclosed_fence: Option<usize>,
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
    let mut parser = Parser {
        kinds: vec![Line::Text; lines.len()],
        lines,
        items: Vec::new(),
        errors: Vec::new(),
    };
    let mut index = front_matter(&parser.lines).map_or(0, |end| {
        parser.mark(0, end, Line::FrontMatter);
        end + 1
    });
    let mut code_blocks = Vec::new();
    let mut fence: Option<(char, usize, usize)> = None;
    while let Some(line) = parser.lines.get(index).copied() {
        if let Some((character, length, _)) = fence {
            parser.mark(index, index, Line::Code);
            if closes_fence(line, character, length) {
                fence = None;
            }
            index += 1;
            continue;
        }
        if let Some(start) = block_start(line) {
            index = parser.block(index, &start);
            continue;
        }
        if let Some((character, length)) = opens_fence(line) {
            fence = Some((character, length, index + 1));
            code_blocks.push(index + 1);
            parser.mark(index, index, Line::Code);
        }
        index += 1;
    }
    Parsed {
        items: parser.items,
        errors: parser.errors,
        lines: parser.kinds,
        code_blocks,
        unclosed_fence: fence.map(|(_, _, line)| line),
    }
}

/// The state of [`parse`].
struct Parser<'a> {
    lines: Vec<&'a str>,
    kinds: Vec<Line>,
    items: Vec<RawItem>,
    errors: Vec<(usize, String)>,
}

impl Parser<'_> {
    /// Marks the lines `first` to `last` (indexes) as `kind`.
    fn mark(&mut self, first: usize, last: usize, kind: Line) {
        for slot in self.kinds.iter_mut().take(last + 1).skip(first) {
            *slot = kind;
        }
    }

    /// Reports `reason` about the line at `index`.
    fn error(&mut self, index: usize, reason: &str) {
        self.errors.push((index + 1, reason.to_owned()));
    }

    fn line(&self, index: usize) -> &str {
        self.lines.get(index).copied().unwrap_or_default()
    }

    /// Reads the srl block that `start` opens on the line at `open`, and
    /// returns the index of the line after it.
    fn block(&mut self, open: usize, start: &BlockStart) -> usize {
        let fence = start.character.to_string().repeat(start.length);
        let close = (open + 1..self.lines.len())
            .find(|index| closes_fence(self.line(*index), start.character, start.length));
        let Some(close) = close else {
            self.mark(open, self.lines.len().saturating_sub(1), Line::Block);
            self.error(
                open,
                &format!(
                    "this srl block is never closed, so a renderer would show the rest of the \
                     note as code; close it with a line `{fence}`"
                ),
            );
            return self.lines.len();
        };
        self.mark(open, close, Line::Block);
        let block = Block {
            fence,
            open: open + 1,
            close: close + 1,
        };
        let opening = match read_header(self.line(open), start.mark_end) {
            Ok(opening) => opening,
            Err(reason) => {
                self.error(open, &reason);
                return close + 1;
            }
        };
        let first = open + 1;
        if first == close {
            self.error(
                open,
                "this srl block holds no item; write its fields, or remove it",
            );
        } else if field(self.line(first)).is_some() {
            let last = self.item(open, opening, close, &block);
            let stray = (last + 1..close).find(|index| !is_blank(self.line(*index)));
            if let Some(stray) = stray {
                let reason = if self.line(stray).starts_with('@') {
                    "a block whose fields start right after its fence holds one item; to hold \
                     several, start each one, the first included, with an `@` line"
                } else {
                    STRAY
                };
                self.error(stray, reason);
            }
        } else if self.line(first).starts_with('@') {
            if opening.entries.is_empty() {
                self.listed(first, close, &block);
            } else {
                self.error(
                    open,
                    "the items of this block start with `@` lines, so its fence takes no \
                     entries; move them to the `@` line of the item they belong to",
                );
            }
        } else {
            self.error(
                first,
                "the first line of an srl block must be a field line such as `Q: …`, or an `@` \
                 line that starts an item",
            );
        }
        close + 1
    }

    /// Reads the items of a block from its line `first` (an index), the
    /// first `@` line, to its closing fence at `close`.
    fn listed(&mut self, first: usize, close: usize, block: &Block) {
        let mut index = first;
        while index < close {
            let line = self.line(index);
            if is_blank(line) {
                index += 1;
                continue;
            }
            let header = match at_header(line) {
                Some(Ok(header)) => header,
                Some(Err(reason)) => {
                    self.error(index, &reason);
                    index = self.next_at(index + 1, close);
                    continue;
                }
                None => {
                    self.error(index, STRAY);
                    index = self.next_at(index + 1, close);
                    continue;
                }
            };
            if index + 1 == close || field(self.line(index + 1)).is_none() {
                self.error(
                    index,
                    "this header is not followed by a field line such as `Q: …` on the next \
                     line; add the fields, or remove the header",
                );
                index = self.next_at(index + 1, close);
                continue;
            }
            index = self.item(index, header, close, block) + 1;
        }
    }

    /// The index of the first `@` line from `from` (an index), or `close`.
    fn next_at(&self, from: usize, close: usize) -> usize {
        (from..close)
            .find(|index| self.line(*index).starts_with('@'))
            .unwrap_or(close)
    }

    /// Reads the item whose header is at `header` (an index) and whose
    /// fields start on the next line, in a block closing at `close`, and
    /// returns the index of its last line.
    fn item(&mut self, header: usize, line: HeaderLine, close: usize, block: &Block) -> usize {
        let body = item_body(&self.lines, header + 1, close, &mut self.errors);
        self.mark(header, header, Line::Header);
        self.mark(header + 1, body.last, Line::Item);
        self.items.push(RawItem {
            header: line.header,
            entries: line.entries,
            line: header + 1,
            end: body.last + 1,
            fields: body.fields,
            block: block.clone(),
        });
        body.last
    }
}

/// The error about a line of an srl block that belongs to no item.
const STRAY: &str = "this line of an srl block is not part of an item; continuation lines start \
                     with exactly two spaces, and each item starts with a field line or an `@` \
                     line";

/// The index of the closing `---` of the front-matter, if the note starts
/// with one that is closed.
fn front_matter(lines: &[&str]) -> Option<usize> {
    if lines.first() != Some(&"---") {
        return None;
    }
    lines
        .iter()
        .skip(1)
        .position(|line| *line == "---")
        .map(|index| index + 1)
}

/// The character and length of the fence `line` opens, if it opens one:
/// three or more backticks or tildes after at most three spaces.
fn opens_fence(line: &str) -> Option<(char, usize)> {
    let rest = fence_indent(line)?;
    let character = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let length = rest.chars().take_while(|c| *c == character).count();
    (length >= 3).then_some((character, length))
}

/// Whether `line` closes a fence of `length` `character`s.
fn closes_fence(line: &str, character: char, length: usize) -> bool {
    fence_indent(line).is_some_and(|rest| {
        let run = rest.chars().take_while(|c| *c == character).count();
        run >= length && rest.chars().skip(run).all(char::is_whitespace)
    })
}

/// `line` without its leading spaces, if there are at most three, as a
/// fence may have.
fn fence_indent(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches(' ');
    (line.len() - rest.len() <= 3).then_some(rest)
}

/// The opening fence of an srl block.
pub(crate) struct BlockStart {
    character: char,
    length: usize,
    /// The byte just after `srl`.
    mark_end: usize,
}

/// The opening fence on `line`, if it opens an srl block: at column 0, three
/// or more backticks or tildes, any number of spaces, `srl`, then a space or
/// the end of the line.
pub(crate) fn block_start(line: &str) -> Option<BlockStart> {
    let character = line.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let length = line.chars().take_while(|c| *c == character).count();
    let after = line
        .get(length..)?
        .trim_start_matches(' ')
        .strip_prefix("srl")?;
    (length >= 3 && (after.is_empty() || after.starts_with(' '))).then(|| BlockStart {
        character,
        length,
        mark_end: line.len() - after.len(),
    })
}

/// The header on `line`, an srl block's opening fence or an `@` line: `None`
/// if it is neither, an error if it is one but isn't valid.
pub(crate) fn header(line: &str) -> Option<Result<HeaderLine, String>> {
    at_header(line).or_else(|| {
        let start = block_start(line)?;
        Some(read_header(line, start.mark_end))
    })
}

/// The header on an `@` line: `None` if `line` doesn't start with `@`.
fn at_header(line: &str) -> Option<Result<HeaderLine, String>> {
    let after = line.strip_prefix('@')?;
    if !(after.is_empty() || after.starts_with(' ')) {
        return Some(Err(
            "an `@` line is `@`, then a space and the item's entries, as in `@ id:x7k2m9`"
                .to_owned(),
        ));
    }
    Some(read_header(line, 1))
}

/// The header whose entries start at byte `mark_end` of `line`.
fn read_header(line: &str, mark_end: usize) -> Result<HeaderLine, String> {
    let inner = line
        .get(mark_end..)
        .unwrap_or_default()
        .trim_end_matches(' ');
    if line.starts_with('`') && inner.contains('`') {
        return Err(
            "a fence of backticks can't be followed by a backtick, or renderers don't read it \
             as a block; open the block with `~~~` instead"
                .to_owned(),
        );
    }
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
            start: span.start + mark_end,
            end: span.end + mark_end,
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
        entries: inner.trim_start_matches(' ').to_owned(),
        mark_end,
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

/// Reads the fields of an item from line `first` (an index), up to the
/// first line that is neither a field nor a continuation, or to `close`,
/// its block's closing fence.
fn item_body(
    lines: &[&str],
    first: usize,
    close: usize,
    errors: &mut Vec<(usize, String)>,
) -> Body {
    let mut fields: Vec<(String, Vec<String>)> = Vec::new();
    let mut last = first;
    let mut index = first;
    while let Some(line) = lines.get(index).copied().filter(|_| index < close) {
        if let Some((key, rest)) = field(line) {
            fields.push((key.to_owned(), vec![rest.to_owned()]));
            last = index;
            index += 1;
            continue;
        }
        if is_blank(line) {
            let next = (index..close).find(|next| {
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
    fn a_block_with_one_item_is_read() {
        let parsed = parse("Prose.\n```srl id:x7k2m9\nQ: What?\nA: That.\n```\n\nMore prose.\n");
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let [item] = parsed.items.as_slice() else {
            panic!("expected one item: {:?}", parsed.items);
        };
        assert_eq!(item.header.id.as_ref().unwrap().as_str(), "x7k2m9");
        assert_eq!((item.line, item.end), (2, 4));
        assert_eq!((item.block.open, item.block.close), (2, 5));
        assert!(item.is_alone());
        assert_eq!(field_pairs(item), [("Q", "What?"), ("A", "That.")]);
        assert_eq!(
            parsed.lines,
            [
                Line::Text,
                Line::Header,
                Line::Item,
                Line::Item,
                Line::Block,
                Line::Text,
                Line::Text
            ]
        );
    }

    #[test]
    fn a_block_with_at_lines_holds_several_items() {
        let text = "~~~~srl\n@ id:abc\nQ: a\nA: b\n\n@\nQ: c\n  more\n\n~~~~\n";
        let parsed = parse(text);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let [one, two] = parsed.items.as_slice() else {
            panic!("expected two items: {:?}", parsed.items);
        };
        assert_eq!((one.line, one.end, two.line, two.end), (2, 4, 6, 8));
        assert_eq!(one.entries, "id:abc");
        assert!(two.header.id.is_none() && !two.is_alone());
        assert_eq!(field_pairs(two), [("Q", "c\nmore")]);
        assert_eq!(two.block.fence, "~~~~");
        assert_eq!(parsed.lines[4], Line::Block);
        assert_eq!(parsed.lines[8], Line::Block);
        let lines = lines(text);
        assert_eq!(one.text(&lines), "~~~~srl id:abc\nQ: a\nA: b\n~~~~");
        assert_eq!(two.text(&lines), "~~~~srl\nQ: c\n  more\n~~~~");
    }

    #[test]
    fn fences_may_have_any_spacing() {
        for line in ["```srl", "```  srl", "````srl   id:abc  ", "~~~ srl draft"] {
            let fence = line.split(['s', ' ']).next().unwrap();
            let parsed = parse(&format!("{line}\nQ: q\nA: a\n{fence}\n"));
            assert!(parsed.errors.is_empty(), "{line}: {:?}", parsed.errors);
            assert_eq!(parsed.items.len(), 1, "{line}");
        }
    }

    #[test]
    fn other_fences_are_code() {
        for line in [
            " ```srl",
            "```srlx",
            "```rust",
            "``srl",
            "```SRL",
            "```srl{id:abc}",
        ] {
            let parsed = parse(&format!("{line}\nQ: q\n```\n"));
            assert!(parsed.errors.is_empty(), "{line}: {:?}", parsed.errors);
            assert!(parsed.items.is_empty(), "{line}");
        }
    }

    #[test]
    fn bad_headers_are_errors() {
        for line in [
            "```srl darft",
            "```srl id:abc id:abd",
            "```srl id:",
            "```srl id",
            "```srl draft:yes",
            "```srl id:ABC",
            "```srl see:\"a b",
            "```srl see:\"a\"b",
            "```srl see:`a`",
        ] {
            let parsed = parse(&format!("{line}\nQ: q\nA: a\n```\n"));
            assert_eq!(parsed.errors.len(), 1, "{line}: {:?}", parsed.errors);
            assert_eq!(parsed.errors[0].0, 1, "{line}");
            assert!(parsed.items.is_empty(), "{line}");
        }
        let parsed = parse("```srl\n@x\nQ: q\n@ darft\nQ: r\n@ id:abc\nQ: s\n```\n");
        assert_eq!(
            parsed
                .errors
                .iter()
                .map(|(line, _)| *line)
                .collect::<Vec<_>>(),
            [2, 4]
        );
        assert_eq!(parsed.items.len(), 1);
        assert!(parse("~~~srl see:`a`\nQ: q\n~~~\n").errors.is_empty());
    }

    #[test]
    fn entries_are_read_with_quotes() {
        let line = header("```srl draft see:\"my notes/a b.md\" kind:qa id:abc")
            .unwrap()
            .unwrap();
        assert_eq!(line.header.see.as_deref(), Some("my notes/a b.md"));
        assert_eq!(line.header.kind.as_deref(), Some("qa"));
        assert!(line.header.draft);
        assert_eq!(line.mark_end, 6);
        assert_eq!(line.draft, Some(Span { start: 7, end: 12 }));
        let line = header("@ draft").unwrap().unwrap();
        assert_eq!(
            (line.mark_end, line.draft),
            (1, Some(Span { start: 2, end: 7 }))
        );
    }

    #[test]
    fn badly_built_blocks_are_errors() {
        for (text, line) in [
            ("```srl\n```\n", 1),
            ("```srl\nQ: q\n", 1),
            ("```srl\n\nQ: q\n```\n", 2),
            ("```srl\nprose\n```\n", 2),
            ("```srl id:abc\n@\nQ: q\n```\n", 1),
            ("```srl\nQ: q\n@\nQ: r\n```\n", 3),
            ("```srl\nQ: q\nA: a\nmore\n```\n", 4),
            ("```srl\n@\n\nQ: q\n```\n", 2),
            ("```srl\n@\nQ: q\nstray\n@\nQ: r\n```\n", 4),
        ] {
            let parsed = parse(text);
            assert_eq!(
                parsed.errors.iter().map(|(at, _)| *at).collect::<Vec<_>>(),
                [line],
                "{text:?}: {:?}",
                parsed.errors
            );
        }
    }

    #[test]
    fn a_block_with_an_error_is_left_out_of_the_text() {
        let parsed = parse("```srl\nprose\n```\nAfter.\n");
        assert_eq!(
            parsed.lines,
            [Line::Block, Line::Block, Line::Block, Line::Text]
        );
    }

    #[test]
    fn continuations_and_blank_lines_belong_to_fields() {
        let text = "````srl\nQ: Does this compile?\n  ```rust\n    let a = 1;\n  ```\n\nA:\n  No.\n\n  Really.\n\n````\nAfter.\n";
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
        assert_eq!(parsed.lines[10], Line::Block);
        assert_eq!(parsed.lines[12], Line::Text);
    }

    #[test]
    fn a_shorter_fence_in_a_field_closes_the_block() {
        let parsed = parse("```srl\nQ: q\n  ```rust\n  code\n  ```\nA: a\n```\n");
        assert_eq!(parsed.items[0].block.close, 5);
        assert_eq!(parsed.lines[5], Line::Text);
        assert_eq!(parsed.unclosed_fence, Some(7));
    }

    #[test]
    fn one_space_or_a_tab_inside_an_item_is_an_error() {
        for line in [" x", "\tx", "   "] {
            let parsed = parse(&format!("```srl\nQ: q\n{line}\nA: a\n```\n"));
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
        let text =
            "---\ntitle: x\n---\n````markdown\n```srl\nQ: q\n```\n````\n~~~~\n```srl\n~~~\n~~~~\n";
        let parsed = parse(text);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(parsed.items.is_empty());
        assert_eq!(parsed.lines[1], Line::FrontMatter);
        assert_eq!(parsed.lines[4], Line::Code);
        assert_eq!(parsed.lines[9], Line::Code);
        assert_eq!(parsed.code_blocks, [4, 9]);
    }

    #[test]
    fn crlf_and_a_bom_are_read() {
        let parsed = parse("\u{feff}```srl\r\nQ: q\r\n  more\r\nA: a\r\n```\r\n");
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
