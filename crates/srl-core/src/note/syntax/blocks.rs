//! The srl block syntax: items in fenced code blocks of Markdown (see
//! `docs/notes.md`, "The srl block syntax"), the syntax of every note but a
//! JSONL note.
//!
//! The parser reports every problem it finds, with its line, and carries
//! on, so that one run lists all of them.

use super::{Edit, Lines, Parsed, ParsedItem, Syntax, unless_blank};
use crate::context::ContextMode;
use crate::kind::has_deletion;
use crate::note::{Entries, Field, ItemId, is_field_key, lines};

/// The srl block syntax.
pub(super) struct Blocks;

impl Syntax for Blocks {
    fn parse(&self, text: &str) -> Parsed {
        let reading = read(text);
        Parsed {
            items: reading.found.into_iter().map(|found| found.item).collect(),
            errors: reading.errors,
        }
    }

    /// The note, without its srl blocks in mode `prose` (see [`context`]).
    fn own_context(&self, text: &str, mode: ContextMode) -> Option<String> {
        context(text, mode)
    }

    /// The same as for its own items: the note, without its srl blocks in
    /// mode `prose` (see [`context`]).
    fn context(&self, text: &str, mode: ContextMode) -> Option<String> {
        context(text, mode)
    }

    /// Adds `id:ID` right after the `srl` of the item's opening fence or
    /// the `@` of its `@` line.
    fn with_id(&self, text: &str, item: usize, id: &ItemId) -> Option<Edit> {
        let line = read(text).found.get(item)?.item.line;
        let content = *lines(text).get(line - 1)?;
        let header = header(content)?.ok()?;
        let (before, after) = content.split_at(header.mark_end);
        Some(Edit::one(line, format!("{before} id:{id}{after}")))
    }

    /// Removes `draft` from the item's header, with the spaces before it.
    fn without_draft(&self, text: &str, item: usize) -> Option<Edit> {
        let line = read(text).found.get(item)?.item.line;
        let content = *lines(text).get(line - 1)?;
        let draft = header(content)?.ok()?.draft?;
        let before = content.get(..draft.start).unwrap_or_default();
        let after = content.get(draft.end..).unwrap_or_default();
        Some(Edit::one(
            line,
            format!("{}{after}", before.trim_end_matches(' ')),
        ))
    }

    /// An item that has its block to itself becomes a block of its own, as
    /// [`format()`] writes it; an item that starts with an `@` line keeps its
    /// place in its block, as an `@` line and its field lines.
    fn replace(
        &self,
        text: &str,
        item: usize,
        entries: &Entries,
        fields: &[Field],
    ) -> Result<Edit, String> {
        let reading = read(text);
        let target = reading
            .found
            .get(item)
            .ok_or_else(|| format!("the note has no item {item}"))?;
        if target.is_alone() {
            return Ok(Edit {
                first: target.block.open,
                last: target.block.close,
                lines: format(entries, fields)?
                    .split('\n')
                    .map(str::to_owned)
                    .collect(),
            });
        }
        let field_lines = field_lines(fields);
        if field_lines.iter().any(|line| target.block.closed_by(line)) {
            return Err(format!(
                "a line of it would close the block the item is in; open and close that block \
                 with a longer fence than {}",
                target.block.fence
            ));
        }
        let written = header_entries(entries)?;
        let header = if written.is_empty() {
            "@".to_owned()
        } else {
            format!("@ {written}")
        };
        Ok(Edit {
            first: target.item.line,
            last: target.end,
            lines: std::iter::once(header).chain(field_lines).collect(),
        })
    }

    fn format(&self, entries: &Entries, fields: &[Field]) -> Result<String, String> {
        format(entries, fields)
    }

    /// A blank line: each item is a block of its own.
    fn gap(&self) -> &'static str {
        "\n\n"
    }

    fn lint(&self, text: &str) -> Vec<(usize, String)> {
        lint(text)
    }

    /// The lines of the note the field is written on, as they are, each
    /// named by its own line.
    fn field_lines(&self, text: &str, item: &ParsedItem, field: usize) -> Vec<(usize, String)> {
        let Some(span) = item.spans.get(field) else {
            return Vec::new();
        };
        lines(text)
            .into_iter()
            .enumerate()
            .take(span.last)
            .skip(span.first - 1)
            .map(|(index, line)| (index + 1, line.to_owned()))
            .collect()
    }
}

/// What a line of a note is for its context (see `docs/notes.md`,
/// "Context").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// Text of the note, shown in every mode but `none`.
    Prose,
    /// Text of an item, shown in mode `full` only.
    Item,
    /// Never shown: front-matter, and the syntax around items.
    Hidden,
}

/// A note of srl blocks as a context, in `mode`: its prose, and in mode
/// `full` the field lines of its items, without front-matter, with runs of
/// blank lines made one and no blank lines at the start or the end.
fn context(text: &str, mode: ContextMode) -> Option<String> {
    let reading = read(text);
    let kept = lines(text)
        .into_iter()
        .zip(&reading.lines)
        .filter(|(line, kind)| match kind.role(line) {
            Role::Prose => true,
            Role::Item => mode == ContextMode::Full,
            Role::Hidden => false,
        })
        .map(|(line, _)| line);
    let mut lines = Vec::new();
    for line in kept {
        let blank = line.trim().is_empty();
        let after_blank = lines.last().is_none_or(|last: &&str| last.is_empty());
        if blank && after_blank {
            continue;
        }
        lines.push(if blank { "" } else { line });
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    unless_blank(format!("{}\n", lines.join("\n")))
}

/// A new item as `srl add` writes it: an srl block of its own, its entries
/// on the opening fence, then each field, with continuation lines for
/// multi-line text. The fence is three backticks, or one more than the
/// longest run of backticks a line of the text starts with. Errors are
/// plain reasons.
fn format(entries: &Entries, fields: &[Field]) -> Result<String, String> {
    let written = header_entries(entries)?;
    let longest = fields
        .iter()
        .flat_map(|field| field.text.lines())
        .map(|line| line.trim_start().chars().take_while(|c| *c == '`').count())
        .max()
        .unwrap_or_default();
    let fence = "`".repeat((longest + 1).max(3));
    let mut lines = vec![if written.is_empty() {
        format!("{fence}srl")
    } else {
        format!("{fence}srl {written}")
    }];
    lines.extend(field_lines(fields));
    lines.push(fence);
    Ok(lines.join("\n"))
}

/// `entries` as a header writes them, separated by spaces. Errors are
/// plain reasons.
fn header_entries(entries: &Entries) -> Result<String, String> {
    let mut written = Vec::new();
    if let Some(id) = &entries.id {
        written.push(format!("id:{id}"));
    }
    if let Some(kind) = &entries.kind {
        if kind.is_empty() || kind.contains([' ', '"']) {
            return Err(format!("`{kind}` can't be a kind"));
        }
        written.push(format!("kind:{kind}"));
    }
    if let Some(see) = &entries.see {
        if see.contains('"') {
            return Err("a `see:` path can't contain `\"`".to_owned());
        }
        written.push(if see.contains(' ') {
            format!("see:\"{see}\"")
        } else {
            format!("see:{see}")
        });
    }
    if entries.draft {
        written.push("draft".to_owned());
    }
    Ok(written.join(" "))
}

/// The field lines of `fields`: continuation lines for multi-line text,
/// without trailing blank lines.
fn field_lines(fields: &[Field]) -> Vec<String> {
    let mut lines = Vec::new();
    for field in fields {
        let text = field.text.replace("\r\n", "\n");
        let mut parts = text.split('\n').collect::<Vec<_>>();
        while parts.len() > 1 && parts.last().is_some_and(|line| line.trim().is_empty()) {
            parts.pop();
        }
        let (first, rest) = parts
            .split_first()
            .map_or(("", &[][..]), |(first, rest)| (*first, rest));
        lines.push(if first.is_empty() {
            format!("{}:", field.key)
        } else {
            format!("{}: {first}", field.key)
        });
        lines.extend(rest.iter().map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        }));
    }
    lines
}

/// The likely mistakes of the syntax in one note (see [`Syntax::lint`]).
fn lint(text: &str) -> Vec<(usize, String)> {
    let reading = read(text);
    let lines = lines(text);
    let mut found = Vec::new();
    // A block closed by an indented fence was most likely closed by the
    // end of a code block in one of its fields, too early.
    let mut early = Vec::new();
    for block in reading.found.iter().map(|found| &found.block) {
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
    for (index, (line, kind)) in lines.iter().zip(&reading.lines).enumerate() {
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
    for open in &reading.code_blocks {
        let line = lines.get(open - 1).copied().unwrap_or_default();
        found.extend(block_suspicion(line.trim_start()).map(|message| (*open, message.to_owned())));
    }
    if let Some(open) = reading.unclosed_fence {
        let hidden = lines
            .iter()
            .skip(open)
            .filter(|line| block_start(line).is_some())
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
    found
}

/// What a line of a note is, to the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
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

impl Line {
    /// The role of `line`, of this kind, in the context.
    fn role(self, line: &str) -> Role {
        match self {
            Self::Text | Self::Code => Role::Prose,
            Self::Item => Role::Item,
            Self::Block if line.trim().is_empty() => Role::Item,
            Self::Block | Self::FrontMatter | Self::Header => Role::Hidden,
        }
    }
}

/// An srl block: where it is, and the fence that opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Block {
    /// The backticks or tildes that open it: three or more of one of them.
    fence: String,
    /// The line of its opening fence, counting from 1.
    open: usize,
    /// The line of its closing fence, counting from 1.
    close: usize,
}

impl Block {
    /// Whether `line` would close the block.
    fn closed_by(&self, line: &str) -> bool {
        let character = self.fence.chars().next().unwrap_or('`');
        closes_fence(line, character, self.fence.chars().count())
    }
}

/// An item, with what the syntax knows about it besides.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Found {
    item: ParsedItem,
    /// The last line of the item.
    end: usize,
    /// The header's entries, as written.
    entries: String,
    /// The block the item is in.
    block: Block,
}

impl Found {
    /// Whether the item's header is its block's opening fence, which it
    /// then has to itself.
    fn is_alone(&self) -> bool {
        self.item.line == self.block.open
    }
}

/// What the parser finds in a note.
struct Reading {
    found: Vec<Found>,
    errors: Vec<(usize, String)>,
    /// What each line is.
    lines: Vec<Line>,
    /// The lines that open a fenced code block that is not an srl block.
    code_blocks: Vec<usize>,
    /// The line that opens a fenced code block never closed, which runs to
    /// the end of the note.
    unclosed_fence: Option<usize>,
}

/// Parses a note.
fn read(text: &str) -> Reading {
    let lines = lines(text);
    let mut parser = Parser {
        kinds: vec![Line::Text; lines.len()],
        lines,
        found: Vec::new(),
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
    Reading {
        found: parser.found,
        errors: parser.errors,
        lines: parser.kinds,
        code_blocks,
        unclosed_fence: fence.map(|(_, _, line)| line),
    }
}

/// The state of [`read`].
struct Parser<'a> {
    lines: Vec<&'a str>,
    kinds: Vec<Line>,
    found: Vec<Found>,
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
        let opening = if line.entries.is_empty() {
            format!("{}srl", block.fence)
        } else {
            format!("{}srl {}", block.fence, line.entries)
        };
        let text = std::iter::once(opening.as_str())
            .chain(
                self.lines
                    .get(header + 1..=body.last)
                    .unwrap_or_default()
                    .iter()
                    .copied(),
            )
            .chain(std::iter::once(block.fence.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        self.found.push(Found {
            item: ParsedItem {
                entries: line.entries_read,
                line: header + 1,
                fields: body.fields,
                spans: body.spans,
                text,
            },
            end: body.last + 1,
            entries: line.entries,
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
struct BlockStart {
    character: char,
    length: usize,
    /// The byte just after `srl`.
    mark_end: usize,
}

/// The opening fence on `line`, if it opens an srl block: at column 0, three
/// or more backticks or tildes, any number of spaces, `srl`, then a space or
/// the end of the line.
fn block_start(line: &str) -> Option<BlockStart> {
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

/// Where one entry of a header is, in bytes of its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bytes {
    start: usize,
    end: usize,
}

/// A header and where its parts are on its line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HeaderLine {
    entries_read: Entries,
    /// The entries, as written, without the spaces around them.
    entries: String,
    /// The byte just after `srl` or `@`, where `srl id` adds `id:ID`.
    mark_end: usize,
    /// Where `draft` is, if it is there.
    draft: Option<Bytes>,
}

/// The header on `line`, an srl block's opening fence or an `@` line: `None`
/// if it is neither, an error if it is one but isn't valid.
fn header(line: &str) -> Option<Result<HeaderLine, String>> {
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
    let mut read = Entries::default();
    let mut draft = None;
    let mut seen = Vec::new();
    for (key, value, span) in entries(inner)? {
        let span = Bytes {
            start: span.start + mark_end,
            end: span.end + mark_end,
        };
        if seen.contains(&key) {
            return Err(format!("the header has `{key}` twice; keep one"));
        }
        seen.push(key);
        match (key, value) {
            ("draft", None) => {
                read.draft = true;
                draft = Some(span);
            }
            ("draft", Some(_)) => {
                return Err("`draft` is a flag and takes no value; write `draft`".to_owned());
            }
            (_, Some("")) => return Err(format!("`{key}:` has no value; give one or remove it")),
            ("id", Some(value)) => {
                read.id = Some(ItemId::parse(value).map_err(|error| error.to_string())?);
            }
            ("kind", Some(value)) => read.kind = Some(value.to_owned()),
            ("see", Some(value)) => read.see = Some(value.to_owned()),
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
        entries_read: read,
        entries: inner.trim_start_matches(' ').to_owned(),
        mark_end,
        draft,
    })
}

/// One entry of a header: its key, its value (none for a flag), and where
/// it is.
type Entry<'a> = (&'a str, Option<&'a str>, Bytes);

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
        entries.push((key, value, Bytes { start, end: index }));
    }
}

/// The key and the rest of a field line: a field key at column 0, a colon,
/// then a space or the end of the line.
fn field(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once(':')?;
    if !is_field_key(key) {
        return None;
    }
    if rest.is_empty() {
        Some((key, rest))
    } else {
        rest.strip_prefix(' ').map(|text| (key, text))
    }
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// The fields of an item, where each is, and the index of its last line.
struct Body {
    fields: Vec<Field>,
    spans: Vec<Lines>,
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
    // Each field: its key, its lines of text, and the indexes of its first
    // and last lines.
    let mut fields: Vec<(String, Vec<String>, usize, usize)> = Vec::new();
    let mut last = first;
    let mut index = first;
    while let Some(line) = lines.get(index).copied().filter(|_| index < close) {
        if let Some((key, rest)) = field(line) {
            fields.push((key.to_owned(), vec![rest.to_owned()], index, index));
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
                && let Some((_, text, _, _)) = fields.last_mut()
            {
                text.extend((index..next).map(|_| String::new()));
            }
            index = next;
            continue;
        }
        if let Some(continued) = line.strip_prefix("  ") {
            if let Some((_, text, _, end)) = fields.last_mut() {
                text.push(continued.to_owned());
                *end = index;
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
    let (fields, spans) = fields
        .into_iter()
        .map(|(key, mut text, start, end)| {
            if text.first().is_some_and(String::is_empty) {
                text.remove(0);
            }
            while text.last().is_some_and(|line| is_blank(line)) {
                text.pop();
            }
            (
                Field {
                    key,
                    text: text.join("\n"),
                },
                Lines {
                    first: start + 1,
                    last: end + 1,
                },
            )
        })
        .unzip();
    Body {
        fields,
        spans,
        last,
    }
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
    if block_start(bare).is_some() {
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

    fn parse(text: &str) -> Parsed {
        Blocks.parse(text)
    }

    /// The role in the context of each line of `text`.
    fn roles(text: &str) -> Vec<Role> {
        read(text)
            .lines
            .iter()
            .zip(lines(text))
            .map(|(kind, line)| kind.role(line))
            .collect()
    }

    fn field_pairs(item: &ParsedItem) -> Vec<(&str, &str)> {
        item.fields
            .iter()
            .map(|field| (field.key.as_str(), field.text.as_str()))
            .collect()
    }

    fn found_lines(text: &str) -> Vec<usize> {
        let mut found = lint(text)
            .into_iter()
            .map(|(line, _)| line)
            .collect::<Vec<_>>();
        found.sort_unstable();
        found
    }

    #[test]
    fn a_block_with_one_item_is_read() {
        let text = "Prose.\n```srl id:x7k2m9\nQ: What?\nA: That.\n```\n\nMore prose.\n";
        let reading = read(text);
        assert!(reading.errors.is_empty(), "{:?}", reading.errors);
        let [found] = reading.found.as_slice() else {
            panic!("expected one item: {:?}", reading.found);
        };
        let item = &found.item;
        assert_eq!(item.entries.id.as_ref().unwrap().as_str(), "x7k2m9");
        assert_eq!((item.line, found.end), (2, 4));
        assert_eq!((found.block.open, found.block.close), (2, 5));
        assert!(found.is_alone());
        assert_eq!(field_pairs(item), [("Q", "What?"), ("A", "That.")]);
        assert_eq!(
            reading.lines,
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
        assert_eq!(
            roles(text),
            [
                Role::Prose,
                Role::Hidden,
                Role::Item,
                Role::Item,
                Role::Hidden,
                Role::Prose,
                Role::Prose
            ]
        );
    }

    #[test]
    fn a_block_with_at_lines_holds_several_items() {
        let text = "~~~~srl\n@ id:abc\nQ: a\nA: b\n\n@\nQ: c\n  more\n\n~~~~\n";
        let reading = read(text);
        assert!(reading.errors.is_empty(), "{:?}", reading.errors);
        let [one, two] = reading.found.as_slice() else {
            panic!("expected two items: {:?}", reading.found);
        };
        assert_eq!(
            (one.item.line, one.end, two.item.line, two.end),
            (2, 4, 6, 8)
        );
        assert_eq!(one.entries, "id:abc");
        assert!(two.item.entries.id.is_none() && !two.is_alone());
        assert_eq!(field_pairs(&two.item), [("Q", "c\nmore")]);
        assert_eq!(two.block.fence, "~~~~");
        assert_eq!(reading.lines[4], Line::Block);
        assert_eq!(reading.lines[8], Line::Block);
        assert_eq!(roles(text)[4], Role::Item);
        assert_eq!(one.item.text, "~~~~srl id:abc\nQ: a\nA: b\n~~~~");
        assert_eq!(two.item.text, "~~~~srl\nQ: c\n  more\n~~~~");
    }

    #[test]
    fn fields_know_their_lines() {
        let parsed = parse("```srl\nQ: a\n  b\n\n  c\n\nA: d\n```\n");
        assert_eq!(
            parsed.items[0].spans,
            [Lines { first: 2, last: 5 }, Lines { first: 7, last: 7 }]
        );
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
        assert_eq!(
            parse("~~~srl see:`a`\nQ: q\n~~~\n").errors,
            [] as [(usize, String); 0]
        );
    }

    #[test]
    fn entries_are_read_with_quotes() {
        let line = header("```srl draft see:\"my notes/a b.md\" kind:qa id:abc")
            .unwrap()
            .unwrap();
        assert_eq!(line.entries_read.see.as_deref(), Some("my notes/a b.md"));
        assert_eq!(line.entries_read.kind.as_deref(), Some("qa"));
        assert!(line.entries_read.draft);
        assert_eq!(line.mark_end, 6);
        assert_eq!(line.draft, Some(Bytes { start: 7, end: 12 }));
        let line = header("@ draft").unwrap().unwrap();
        assert_eq!(
            (line.mark_end, line.draft),
            (1, Some(Bytes { start: 2, end: 7 }))
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
    fn a_block_with_an_error_is_hidden_from_the_context() {
        assert_eq!(
            roles("```srl\nprose\n```\nAfter.\n"),
            [Role::Hidden, Role::Hidden, Role::Hidden, Role::Prose]
        );
    }

    #[test]
    fn continuations_and_blank_lines_belong_to_fields() {
        let text = "````srl\nQ: Does this compile?\n  ```rust\n    let a = 1;\n  ```\n\nA:\n  No.\n\n  Really.\n\n````\nAfter.\n";
        let reading = read(text);
        assert!(reading.errors.is_empty(), "{:?}", reading.errors);
        let item = &reading.found[0].item;
        assert_eq!(
            field_pairs(item),
            [
                ("Q", "Does this compile?\n```rust\n  let a = 1;\n```"),
                ("A", "No.\n\nReally.")
            ]
        );
        assert_eq!(reading.found[0].end, 10);
        assert_eq!(reading.lines[10], Line::Block);
        assert_eq!(reading.lines[12], Line::Text);
    }

    #[test]
    fn a_shorter_fence_in_a_field_closes_the_block() {
        let reading = read("```srl\nQ: q\n  ```rust\n  code\n  ```\nA: a\n```\n");
        assert_eq!(reading.found[0].block.close, 5);
        assert_eq!(reading.lines[5], Line::Text);
        assert_eq!(reading.unclosed_fence, Some(7));
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
        let reading = read(text);
        assert!(reading.errors.is_empty(), "{:?}", reading.errors);
        assert_eq!(reading.found, [] as [Found; 0]);
        assert_eq!(reading.lines[1], Line::FrontMatter);
        assert_eq!(reading.lines[4], Line::Code);
        assert_eq!(reading.lines[9], Line::Code);
        assert_eq!(reading.code_blocks, [4, 9]);
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

    #[test]
    fn edits_touch_the_header_line_only() {
        let text = "```srl\n@  draft\nQ: q\n@ id:abc draft see:a.md\nQ: r\n@ id:abd\nQ: s\n```\n";
        let id = ItemId::parse("xyz").unwrap();
        assert_eq!(
            Blocks.with_id(text, 0, &id),
            Some(Edit::one(2, "@ id:xyz  draft".to_owned()))
        );
        assert_eq!(
            Blocks.without_draft(text, 1),
            Some(Edit::one(4, "@ id:abc see:a.md".to_owned()))
        );
        assert_eq!(Blocks.with_id(text, 3, &id), None);
        assert_eq!(Blocks.without_draft(text, 2), None);
        assert_eq!(
            Blocks.with_id("Prose.\n```srl\nQ: q\n```\n", 0, &id),
            Some(Edit::one(2, "```srl id:xyz".to_owned()))
        );
    }

    #[test]
    fn a_notes_context_leaves_out_its_items_unless_full() {
        let text = "---\ntags: [x]\n---\n\n# Title\n\nProse.\n\n\n```srl id:abc\nQ: q\n\nA: a\n```\n\nMore.\n```srl\n@ id:abd\nQ: {{c1::x}}\n\n@ id:abe\nQ: y\n```\n\n";
        for context in [
            Blocks.context(text, ContextMode::Prose),
            Blocks.own_context(text, ContextMode::Prose),
        ] {
            assert_eq!(context.as_deref(), Some("# Title\n\nProse.\n\nMore.\n"));
        }
        for context in [
            Blocks.context(text, ContextMode::Full),
            Blocks.own_context(text, ContextMode::Full),
        ] {
            assert_eq!(
                context.as_deref(),
                Some("# Title\n\nProse.\n\nQ: q\n\nA: a\n\nMore.\nQ: {{c1::x}}\n\nQ: y\n")
            );
        }
    }

    #[test]
    fn nothing_but_blank_space_is_no_context() {
        let only_items = "\n```srl id:abc\nQ: q\nA: a\n```\n\n  \n```srl id:abd\nQ: r\nA: b\n```\n";
        assert_eq!(Blocks.context(only_items, ContextMode::Prose), None);
        assert!(Blocks.context(only_items, ContextMode::Full).is_some());
        assert_eq!(Blocks.context("", ContextMode::Full), None);
    }

    #[test]
    fn a_fields_lines_are_the_lines_of_the_note_it_is_written_on() {
        let text = "Prose.\n```srl\nQ: a\n  b\n\nA: c\n```\n";
        let item = &parse(text).items[0];
        assert_eq!(
            Blocks.field_lines(text, item, 0),
            [(3, "Q: a".to_owned()), (4, "  b".to_owned())]
        );
        assert_eq!(Blocks.field_lines(text, item, 1), [(6, "A: c".to_owned())]);
    }

    #[test]
    fn replacing_keeps_the_form_of_the_item() {
        let text = "```srl id:aaa\nQ: q\nA: a\n```\n````srl\n@ id:bbb\nQ: q\nA: a\n\n@ id:ccc\nQ: q\nA: a\n````\n";
        let entries = |id: &str, draft| Entries {
            id: Some(ItemId::parse(id).unwrap()),
            kind: Some("words".to_owned()),
            draft,
            ..Entries::default()
        };
        let fields = |q: &str| {
            [("Q", q), ("A", "a")].map(|(key, text)| Field {
                key: key.to_owned(),
                text: text.to_owned(),
            })
        };
        let replace =
            |item, entries, fields: &[Field]| Blocks.replace(text, item, &entries, fields);
        let alone = replace(0, entries("aaa", true), &fields("new")).unwrap();
        assert_eq!((alone.first, alone.last), (1, 4));
        assert_eq!(
            alone.lines,
            ["```srl id:aaa kind:words draft", "Q: new", "A: a", "```"]
        );
        let shared = replace(1, entries("bbb", false), &fields("new\n  more")).unwrap();
        assert_eq!((shared.first, shared.last), (6, 8));
        assert_eq!(
            shared.lines,
            ["@ id:bbb kind:words", "Q: new", "    more", "A: a"]
        );
        let closing = replace(2, entries("ccc", false), &fields("q\n````"));
        assert!(closing.is_err());
        assert!(replace(3, entries("aaa", false), &fields("q")).is_err());
    }

    #[test]
    fn new_items_get_a_fence_longer_than_their_code() {
        let entries = Entries {
            id: Some(ItemId::parse("abc").unwrap()),
            kind: Some("qa".to_owned()),
            see: Some("my notes/a.md".to_owned()),
            draft: true,
        };
        let fields = [Field {
            key: "Q".to_owned(),
            text: "q\n```\n````rust\n\n".to_owned(),
        }];
        assert_eq!(
            format(&entries, &fields).unwrap(),
            "`````srl id:abc kind:qa see:\"my notes/a.md\" draft\nQ: q\n  ```\n  ````rust\n`````"
        );
        let bad = Entries {
            kind: Some("a b".to_owned()),
            ..Entries::default()
        };
        assert!(format(&bad, &fields).is_err());
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
    fn errors_are_left_to_parse() {
        assert_eq!(
            found_lines("```srl darft\nQ: q\nA: a\n```\n"),
            [] as [usize; 0]
        );
        assert_eq!(
            found_lines("```srl\nQ: q\nA: a\nmore\n```\n"),
            [] as [usize; 0]
        );
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
        assert_eq!(found_lines(longer), [] as [usize; 0]);
        let tildes = "~~~srl id:fff2\nQ: q\n  ```\n  code\n  ```\nA: a\n~~~\n";
        assert_eq!(found_lines(tildes), [] as [usize; 0]);
    }

    #[test]
    fn blocks_in_an_unclosed_fence_are_found() {
        assert_eq!(found_lines("Prose.\n~~~\n```srl\nQ: q\n```\n"), [2]);
        assert_eq!(found_lines("```\ncode\n"), [] as [usize; 0]);
        assert_eq!(
            found_lines("~~~\n```srl\nQ: q\n```\n~~~\n"),
            [] as [usize; 0]
        );
    }

    #[test]
    fn markers_are_stripped() {
        assert_eq!(without_markers("  > - 12. Q: x"), "Q: x");
        assert_eq!(without_markers("-x"), "-x");
    }
}
