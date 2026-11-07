//! XML collections: an `<items>` element of `<item>` elements, made for
//! writing items by hand (see `docs/formats.md`, "XML"). `srl` writes an
//! item by writing its element again, in place, and adds new ones before
//! `</items>`, so every other byte of the file is kept.

use std::ops::Range;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;

use crate::domain::item::object::{self, Object};
use crate::domain::item::{Entries, Field, is_field_key};
use crate::source::files::{
    Parsed, ParsedItem, changed_meanwhile, keyed, line_ending, position_of, verified,
};
use crate::source::{Change, Refusal};

/// A file read as XML: its items, each with where it is, and where new
/// items go.
#[derive(Debug, Default)]
struct Document {
    /// The items, in order, each with the bytes of its element.
    items: Vec<(Range<usize>, ParsedItem)>,
    /// Each problem, with its line.
    errors: Vec<String>,
    /// The `<items>` element, if the file has one.
    root: Option<Root>,
}

/// Where new items go in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Root {
    /// Before `</items>`, which starts at this byte.
    Closing(usize),
    /// In place of `<items/>`, these bytes.
    Empty(usize, usize),
}

/// The items of `text`.
pub(super) fn parse(text: &str) -> Parsed {
    let document = read(text);
    Parsed {
        items: document.items.into_iter().map(|(_, item)| item).collect(),
        errors: document.errors,
    }
}

/// Reads `text`. A file that is not well-formed, or breaks a rule of the
/// format outside an item, is one error, and has no items; an item that
/// breaks one is an error of its own, and the others are read.
fn read(text: &str) -> Document {
    let bom = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let body = text.get(bom..).unwrap_or_default();
    let mut document = Document::default();
    if body.trim().is_empty() {
        return document;
    }
    let mut reader = Reader::from_str(body);
    match read_root(&mut reader, body, bom, &mut document) {
        Ok(()) => document,
        Err((at, reason)) => Document {
            errors: vec![format!("line {}: {reason}", line_of(text, at + bom))],
            ..Document::default()
        },
    }
}

/// A problem at a byte of the file, and why.
type Failure = (usize, String);

/// The byte the reader is at.
fn at(reader: &Reader<&[u8]>) -> usize {
    usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX)
}

/// The next event of `reader`, with the byte it starts at: for text, the
/// first byte that is not blank space, which is what a message is about.
fn next<'a>(reader: &mut Reader<&'a [u8]>) -> Result<(usize, Event<'a>), Failure> {
    let start = at(reader);
    let event = reader.read_event().map_err(|error| {
        (
            usize::try_from(reader.error_position()).unwrap_or(start),
            format!("the file is not well-formed XML ({error})"),
        )
    })?;
    let blank = if let Event::Text(text) = &event {
        text.iter()
            .take_while(|byte| byte.is_ascii_whitespace())
            .count()
    } else {
        0
    };
    Ok((start + blank, event))
}

/// Reads the document around `<items>`, and the items in it.
fn read_root(
    reader: &mut Reader<&[u8]>,
    body: &str,
    bom: usize,
    document: &mut Document,
) -> Result<(), Failure> {
    loop {
        let (start, event) = next(reader)?;
        match event {
            Event::Eof => {
                if document.root.is_none() {
                    return Err((start, "the file has no `<items>` element".to_owned()));
                }
                return Ok(());
            }
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Text(text) if is_blank(&text) => {}
            Event::Start(element) if document.root.is_none() && name(&element) == "items" => {
                no_attributes(&element, start)?;
                read_items(reader, body, bom, document)?;
            }
            Event::Empty(element) if document.root.is_none() && name(&element) == "items" => {
                no_attributes(&element, start)?;
                document.root = Some(Root::Empty(start + bom, at(reader) + bom));
            }
            Event::Start(_)
            | Event::Empty(_)
            | Event::End(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::PI(_)
            | Event::DocType(_) => {
                return Err((
                    start,
                    "a file holds one `<items>` element, and nothing else but an XML \
                     declaration, comments and blank space"
                        .to_owned(),
                ));
            }
        }
    }
}

/// Reads the `<item>`s of `<items>`, up to its end.
fn read_items(
    reader: &mut Reader<&[u8]>,
    body: &str,
    bom: usize,
    document: &mut Document,
) -> Result<(), Failure> {
    loop {
        let (start, event) = next(reader)?;
        match event {
            Event::Comment(_) => {}
            Event::Text(text) if is_blank(&text) => {}
            Event::End(_) => {
                document.root = Some(Root::Closing(start + bom));
                return Ok(());
            }
            Event::Start(element) if name(&element) == "item" => {
                let read = read_item(reader, &element, false, start);
                add_item(document, body, bom, start..at(reader), read)?;
            }
            Event::Empty(element) if name(&element) == "item" => {
                let read = read_item(reader, &element, true, start);
                add_item(document, body, bom, start..at(reader), read)?;
            }
            Event::Start(_)
            | Event::Empty(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_)
            | Event::Eof => {
                return Err((
                    start,
                    "`<items>` holds `<item>` elements, and nothing else but comments and \
                     blank space"
                        .to_owned(),
                ));
            }
        }
    }
}

/// Adds the item read at `span` of `body`, or its error.
fn add_item(
    document: &mut Document,
    body: &str,
    bom: usize,
    span: Range<usize>,
    read: Result<Result<Object, Failure>, Failure>,
) -> Result<(), Failure> {
    match read? {
        Ok(Object { entries, fields }) => {
            document.items.push((
                span.start + bom..span.end + bom,
                ParsedItem { entries, fields },
            ));
        }
        Err((at, reason)) => {
            let line = line_of(body, at);
            document.errors.push(format!("line {line}: {reason}"));
        }
    }
    Ok(())
}

/// The item of `element`, an `<item>` starting at byte `start`, read up
/// to its end unless it is `empty`: the item, or why it is no item, with
/// the byte it is about. A file that is not well-formed is the outer error.
fn read_item(
    reader: &mut Reader<&[u8]>,
    element: &BytesStart<'_>,
    empty: bool,
    start: usize,
) -> Result<Result<Object, Failure>, Failure> {
    let (mut pairs, mut problem) = match entries(element) {
        Ok(pairs) => (pairs, None),
        Err(reason) => (Vec::new(), Some((start, reason))),
    };
    if !empty {
        loop {
            let (start, event) = next(reader)?;
            match event {
                Event::Comment(_) => {}
                Event::Text(text) if is_blank(&text) => {}
                Event::End(_) => break,
                Event::Start(field) => {
                    let key = name(&field);
                    let text = read_field(reader, &field)?;
                    match (field_problem(&field, &key), text) {
                        (Some(reason), _) | (None, Err(reason)) => {
                            problem.get_or_insert((start, reason));
                        }
                        (None, Ok(text)) => pairs.push((key, Value::from(text))),
                    }
                }
                Event::Empty(field) => {
                    let key = name(&field);
                    match field_problem(&field, &key) {
                        Some(reason) => {
                            problem.get_or_insert((start, reason));
                        }
                        None => pairs.push((key, Value::from(""))),
                    }
                }
                Event::Eof => {
                    return Err((start, "the file ends inside an `<item>`".to_owned()));
                }
                Event::Text(_)
                | Event::CData(_)
                | Event::Decl(_)
                | Event::PI(_)
                | Event::DocType(_) => {
                    problem.get_or_insert((
                        start,
                        "an `<item>` holds its fields, one element each, and nothing else but \
                         comments and blank space"
                            .to_owned(),
                    ));
                }
            }
        }
    }
    Ok(match problem {
        Some(problem) => Err(problem),
        None => object::from_pairs(pairs).map_err(|reason| (start, reason)),
    })
}

/// The header entries of `element`, an `<item>`, as the keys and values
/// of an item object. Errors are plain reasons.
fn entries(element: &BytesStart<'_>) -> Result<Vec<(String, Value)>, String> {
    let mut pairs = Vec::new();
    for attribute in element.attributes() {
        let attribute =
            attribute.map_err(|error| format!("an attribute is not valid ({error})"))?;
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let value = attribute
            .unescape_value()
            .map_err(|error| format!("the attribute `{key}` is not valid ({error})"))?
            .replace("\r\n", "\n");
        let value = match key.as_str() {
            "id" | "kind" | "see" => Value::from(value),
            "draft" => match value.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                _ => {
                    return Err(format!(
                        "the attribute `draft` must be \"true\" or \"false\", not \"{value}\""
                    ));
                }
            },
            _ => {
                return Err(format!(
                    "`{key}` is not a header entry; an `<item>`'s attributes are `id`, `kind`, \
                     `see` and `draft`"
                ));
            }
        };
        pairs.push((key, value));
    }
    Ok(pairs)
}

/// Why `element`, named `key`, can't be a field, if it can't.
fn field_problem(element: &BytesStart<'_>, key: &str) -> Option<String> {
    if !is_field_key(key) {
        Some(format!(
            "`<{key}>` is not a field: a field's name is uppercase letters, digits and _, \
             starting with a letter, such as `<Q>`; header entries are attributes of `<item>`"
        ))
    } else if element.attributes().next().is_some() {
        Some(format!(
            "the field `<{key}>` has attributes, which a field can't have"
        ))
    } else {
        None
    }
}

/// The text of a field whose `element` was just read, up to its end (see
/// `docs/formats.md`, "XML", rule 5), or why it has none. A file that is
/// not well-formed is the outer error.
fn read_field(
    reader: &mut Reader<&[u8]>,
    element: &BytesStart<'_>,
) -> Result<Result<String, String>, Failure> {
    let key = name(element);
    let mut text = String::new();
    let mut cdata = String::new();
    let mut has_cdata = false;
    let mut problem = None;
    let mut depth = 0_usize;
    loop {
        let (start, event) = next(reader)?;
        match event {
            Event::Eof => {
                return Err((start, format!("the file ends inside `<{key}>`")));
            }
            Event::End(_) if depth == 0 => break,
            Event::End(_) => depth -= 1,
            Event::Start(_) => {
                depth += 1;
                problem.get_or_insert(format!(
                    "the field `<{key}>` holds an element; a field holds text, written as \
                     CDATA if it has `<`"
                ));
            }
            Event::Empty(_) => {
                problem.get_or_insert(format!(
                    "the field `<{key}>` holds an element; a field holds text, written as \
                     CDATA if it has `<`"
                ));
            }
            Event::Text(part) => match part.unescape() {
                Ok(part) => text.push_str(&part),
                Err(error) => {
                    problem.get_or_insert(format!(
                        "the text of `<{key}>` has an escape that is not valid ({error})"
                    ));
                }
            },
            Event::CData(part) => {
                has_cdata = true;
                match part.decode() {
                    Ok(part) => cdata.push_str(&part),
                    Err(error) => {
                        problem.get_or_insert(format!(
                            "the CDATA of `<{key}>` is not valid ({error})"
                        ));
                    }
                }
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    if let Some(problem) = problem {
        return Ok(Err(problem));
    }
    Ok(if has_cdata {
        if text.trim().is_empty() {
            Ok(cdata.replace("\r\n", "\n"))
        } else {
            Err(format!(
                "the field `<{key}>` mixes CDATA with other text; write all of it as CDATA, \
                 or none"
            ))
        }
    } else {
        Ok(laid_out(&text.replace("\r\n", "\n")))
    })
}

/// A field's text, laid out (see `docs/formats.md`, "XML", rule 5): on
/// several lines, without a blank first and last line, and without the
/// indentation its other lines have in common.
fn laid_out(text: &str) -> String {
    if !text.contains('\n') {
        return text.to_owned();
    }
    let mut lines = text.split('\n').collect::<Vec<_>>();
    if lines.first().is_some_and(|line| is_blank_line(line)) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|line| is_blank_line(line)) {
        lines.pop();
    }
    // Spaces and tabs are one byte each, so lengths in bytes are safe to
    // cut at.
    let indentation = lines
        .iter()
        .filter(|line| !is_blank_line(line))
        .map(|line| {
            let length = line.len() - line.trim_start_matches([' ', '\t']).len();
            line.get(..length).unwrap_or_default()
        })
        .reduce(|common, indentation| {
            let length = common
                .bytes()
                .zip(indentation.bytes())
                .take_while(|(one, other)| one == other)
                .count();
            common.get(..length).unwrap_or_default()
        })
        .unwrap_or_default();
    lines
        .iter()
        .map(|line| {
            if is_blank_line(line) {
                ""
            } else {
                line.strip_prefix(indentation).unwrap_or(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `line` holds nothing but spaces and tabs.
fn is_blank_line(line: &str) -> bool {
    line.chars().all(|c| c == ' ' || c == '\t')
}

/// Whether a text event holds nothing but blank space.
fn is_blank(text: &quick_xml::events::BytesText<'_>) -> bool {
    text.iter().all(u8::is_ascii_whitespace)
}

/// The name of `element`, as written.
fn name(element: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(element.name().as_ref()).into_owned()
}

/// Fails if `element`, `<items>`, has attributes.
fn no_attributes(element: &BytesStart<'_>, at: usize) -> Result<(), Failure> {
    if element.attributes().next().is_some() {
        Err((at, "`<items>` has no attributes".to_owned()))
    } else {
        Ok(())
    }
}

/// The line, counting from 1, that byte `at` of `text` is on.
fn line_of(text: &str, at: usize) -> usize {
    text.as_bytes()
        .iter()
        .take(at)
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

/// The indentation of the line `at` is on, if nothing but blank space comes
/// before it there.
fn indentation_at(text: &str, at: usize) -> &str {
    let line_start = text
        .get(..at)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |newline| newline + 1);
    let before = text.get(line_start..at).unwrap_or_default();
    let before = before.strip_prefix('\u{feff}').unwrap_or(before);
    if is_blank_line(before) { before } else { "" }
}

/// The XML file at `path`, `old`, with `changes` made to it and checked,
/// and the items added, as written. Only the elements of the items changed
/// are written again, and new ones are added before `</items>`.
pub(super) fn changed(
    path: &str,
    old: Option<&str>,
    changes: &[(usize, &Change)],
) -> Result<(String, String), Refusal> {
    let text = old.unwrap_or_default();
    let document = read(text);
    if !document.errors.is_empty() {
        return Err(changed_meanwhile(path));
    }
    let spans = document
        .items
        .iter()
        .map(|(span, _)| span.clone())
        .collect::<Vec<_>>();
    let mut expected = keyed(Parsed {
        items: document.items.into_iter().map(|(_, item)| item).collect(),
        errors: Vec::new(),
    });
    let indentation = spans
        .first()
        .map_or("  ", |span| indentation_at(text, span.start));
    let indentation = if indentation.is_empty() {
        "  "
    } else {
        indentation
    };
    let step = indentation;
    let ending = line_ending(text);
    let mut edits = Vec::<(Range<usize>, String)>::new();
    let mut added = Vec::new();
    for &(at, change) in changes {
        match change {
            Change::Replace {
                handle,
                before,
                after,
                ..
            } => {
                if old.is_none() {
                    return Err(changed_meanwhile(path));
                }
                let position = position_of(&expected, handle, before, path)?;
                if before != after {
                    let span = spans
                        .get(position)
                        .expect("the position of an item should be among the file's items");
                    let indentation = indentation_at(text, span.start);
                    let element = element(&after.entries, &after.fields, indentation, step)
                        .map_err(|reason| Refusal::Change { at, reason })?;
                    edits.retain(|(range, _)| range != span);
                    edits.push((span.clone(), element.replace('\n', ending)));
                }
                *expected
                    .get_mut(position)
                    .expect("the position of an item should be among the file's items") =
                    (after.entries.clone(), after.fields.clone());
            }
            Change::Add { item, .. } => {
                let element = element(&item.entries, &item.fields, indentation, step)
                    .map_err(|reason| Refusal::Change { at, reason })?;
                added.push(element);
                expected.push((item.entries.clone(), item.fields.clone()));
            }
        }
    }
    let mut new = with_added(text, document.root, &added, indentation);
    // The items are before the place new ones go, so editing them last,
    // from the end, keeps every place right.
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, element) in edits {
        new.replace_range(range, &element);
    }
    let parsed = parse(&new);
    verified(path, parsed.errors.is_empty() && keyed(parsed) == expected)?;
    let added = added.iter().fold(String::new(), |mut all, element| {
        all.push_str(element);
        all.push('\n');
        all
    });
    Ok((new, added))
}

/// `text`, whose `<items>` is `root`, with the elements `added` before
/// `</items>`, each on lines of its own indented by `indentation`, in the
/// line ending of `text`; a file without `<items>` becomes one.
fn with_added(text: &str, root: Option<Root>, added: &[String], indentation: &str) -> String {
    let mut new = text.to_owned();
    if added.is_empty() {
        return new;
    }
    let ending = line_ending(text);
    let items = added
        .iter()
        .fold(String::new(), |mut all, element| {
            all.push_str(indentation);
            all.push_str(element);
            all.push('\n');
            all
        })
        .replace('\n', ending);
    match root {
        None => {
            let bom = if text.starts_with('\u{feff}') {
                "\u{feff}"
            } else {
                ""
            };
            new = format!("{bom}<items>{ending}{items}</items>{ending}");
        }
        Some(Root::Empty(start, end)) => {
            new.replace_range(start..end, &format!("<items>{ending}{items}</items>"));
        }
        Some(Root::Closing(close)) => {
            let line_start = new
                .get(..close)
                .and_then(|before| before.rfind('\n'))
                .map_or(0, |newline| newline + 1);
            if is_blank_line(new.get(line_start..close).unwrap_or_default()) {
                new.insert_str(line_start, &items);
            } else {
                new.insert_str(close, &format!("{ending}{items}"));
            }
        }
    }
    new
}

/// The `<item>` element `srl` writes for an item of `entries` and `fields`
/// whose line is indented by `indentation`, with its fields indented one
/// `step` more; the first line without the indentation. Its entries are
/// those that differ from their defaults, in the order of an item object.
/// Errors are plain reasons.
fn element(
    entries: &Entries,
    fields: &[Field],
    indentation: &str,
    step: &str,
) -> Result<String, String> {
    let mut element = String::from("<item");
    let attributes = [
        ("id", entries.id.as_ref().map(ToString::to_string)),
        ("kind", entries.kind.clone()),
        ("see", entries.see.clone()),
        ("draft", entries.draft.then(|| "true".to_owned())),
    ];
    for (key, value) in attributes {
        if let Some(value) = value {
            element.push(' ');
            element.push_str(key);
            element.push_str("=\"");
            element.push_str(&escape_attribute(&value));
            element.push('"');
        }
    }
    element.push_str(">\n");
    let inner = format!("{indentation}{step}");
    for field in fields {
        element.push_str(&inner);
        element.push_str(&field_element(field, &inner, step)?);
        element.push('\n');
    }
    element.push_str(indentation);
    element.push_str("</item>");
    Ok(element)
}

/// The element `srl` writes for `field`, whose line is indented by
/// `indentation`; the first line without it. Errors are plain reasons.
fn field_element(field: &Field, indentation: &str, step: &str) -> Result<String, String> {
    let (key, text) = (&field.key, &field.text);
    if text.contains('\r') {
        return Err(format!(
            "the text of {key} holds a carriage return, which XML reads as a line ending; keep \
             the item in a JSONL collection instead"
        ));
    }
    if text.is_empty() {
        return Ok(format!("<{key}></{key}>"));
    }
    if !text.contains(['<', '&']) && !text.contains("]]>") {
        let plain = if text.contains('\n') {
            let content = text
                .split('\n')
                .map(|line| {
                    if line.is_empty() {
                        String::new()
                    } else {
                        format!("{indentation}{step}{line}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("<{key}>\n{content}\n{indentation}</{key}>")
        } else {
            format!("<{key}>{text}</{key}>")
        };
        let inner = plain
            .strip_prefix(&format!("<{key}>"))
            .and_then(|rest| rest.strip_suffix(&format!("</{key}>")))
            .unwrap_or_default();
        if laid_out(inner) == *text {
            return Ok(plain);
        }
    }
    Ok(format!(
        "<{key}><![CDATA[{}]]></{key}>",
        text.replace("]]>", "]]]]><![CDATA[>")
    ))
}

/// `value` escaped for an attribute in double quotes.
fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
        .replace('\n', "&#10;")
        .replace('\t', "&#9;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::item::ItemId;

    fn item(text: &str) -> (Entries, Vec<(String, String)>) {
        let parsed = parse(text);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let item = parsed.items.into_iter().next().unwrap();
        let fields = item
            .fields
            .into_iter()
            .map(|field| (field.key, field.text))
            .collect();
        (item.entries, fields)
    }

    #[test]
    fn entries_are_attributes_and_fields_are_elements() {
        let text = "\u{feff}<?xml version=\"1.0\"?>\n<!-- A note. -->\n<items>\n  <item id=\"abc\" kind=\"cloze\" see=\"a &amp; b.md\" draft=\"true\">\n    <Q>One &lt; two</Q>\n    <A/>\n  </item>\n  <item kind=\"qa\" draft=\"false\"><Q>q</Q><A>a</A></item>\n</items>\n";
        let parsed = parse(text);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let first = &parsed.items[0];
        assert_eq!(first.entries.id, Some(ItemId::parse("abc").unwrap()));
        assert_eq!(first.entries.kind.as_deref(), Some("cloze"));
        assert_eq!(first.entries.see.as_deref(), Some("a & b.md"));
        assert!(first.entries.draft);
        assert_eq!(first.fields[0].text, "One < two");
        assert_eq!(first.fields[1].text, "");
        let second = &parsed.items[1];
        assert_eq!(second.entries, Entries::default());
    }

    #[test]
    fn a_field_of_several_lines_is_laid_out_and_cdata_is_taken_as_written() {
        let text = "<items>\r\n  <item>\r\n    <Q>\r\n      Does this compile?\r\n\r\n          let a = 1;\r\n    </Q>\r\n    <A><![CDATA[  a < b\r\n  ]]]]><![CDATA[>]]></A>\r\n    <B>  one line  </B>\r\n    <C>first\r\n      second</C>\r\n  </item>\r\n</items>";
        let (_, fields) = item(text);
        assert_eq!(fields[0].1, "Does this compile?\n\n    let a = 1;");
        assert_eq!(fields[1].1, "  a < b\n  ]]>");
        assert_eq!(fields[2].1, "  one line  ");
        assert_eq!(fields[3].1, "first\n      second");
        assert_eq!(laid_out("\n\t\ta\n\t  b\n\t"), "\ta\n  b");
    }

    #[test]
    fn what_breaks_the_rules_is_an_error_with_its_line() {
        for (text, line) in [
            ("", 0),
            ("<items>", 1),
            ("<things/>", 1),
            ("<items/><items/>", 1),
            ("<items>\n<thing/>\n</items>", 2),
            ("<items>\ntext\n</items>", 2),
            ("<items a=\"b\"/>", 1),
            ("plain text", 1),
        ] {
            let parsed = parse(text);
            if line == 0 {
                assert!(parsed.errors.is_empty() && parsed.items.is_empty());
                continue;
            }
            assert_eq!(parsed.errors.len(), 1, "{text:?}: {:?}", parsed.errors);
            assert!(
                parsed.errors[0].starts_with(&format!("line {line}: ")),
                "{text:?}: {:?}",
                parsed.errors
            );
        }
        // A file that ends inside an element is one error, not a hang.
        for text in [
            "<items>\n<item>\n<Q>q",
            "<items>\n<item>",
            "<items>\n<item><Q>",
        ] {
            assert_eq!(parse(text).errors.len(), 1, "{text:?}");
        }
        // An item that breaks a rule is an error of its own.
        let text = "<items>\n<item drfat=\"true\"><Q>q</Q></item>\n<item><q>q</q></item>\n<item><Q a=\"b\">q</Q></item>\n<item><Q>q<b/></Q></item>\n<item><Q>a<![CDATA[b]]></Q></item>\n<item draft=\"yes\"><Q>q</Q></item>\n<item><Q>q</Q><Q>r</Q></item>\n<item/>\n<item><Q>ok</Q><A>a</A></item>\n</items>\n";
        let parsed = parse(text);
        assert_eq!(parsed.items.len(), 1);
        let lines = parsed
            .errors
            .iter()
            .map(|error| error.split(':').next().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            lines,
            [
                "line 2", "line 3", "line 4", "line 5", "line 6", "line 7", "line 8", "line 9"
            ],
            "{:?}",
            parsed.errors
        );
    }

    fn field(key: &str, text: &str) -> Field {
        Field {
            key: key.to_owned(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn fields_are_written_as_they_read_back() {
        for text in [
            "one line",
            "  spaces around  ",
            "two\nlines",
            "\nblank first",
            "blank last\n",
            "  every line\n  indented",
            "a\n   \nb",
            "a < b & c",
            "has ]]> in it",
            "a\n\n    code",
        ] {
            let written = field_element(&field("Q", text), "    ", "  ").unwrap();
            let file = format!("<items>\n  <item>\n    {written}\n  </item>\n</items>\n");
            let (_, fields) = item(&file);
            assert_eq!(fields[0].1, text, "{written}");
        }
        assert_eq!(
            field_element(&field("A", "No.\n\nMore."), "    ", "  ").unwrap(),
            "<A>\n      No.\n\n      More.\n    </A>"
        );
        assert!(field_element(&field("A", "a\r\nb"), "", "  ").is_err());
    }

    fn object(id: &str, fields: &[(&str, &str)]) -> Object {
        Object {
            entries: Entries {
                id: Some(ItemId::parse(id).unwrap()),
                ..Entries::default()
            },
            fields: fields.iter().map(|(key, text)| field(key, text)).collect(),
        }
    }

    #[test]
    fn a_change_writes_only_the_element_of_its_item() {
        let text = "<?xml version=\"1.0\"?>\r\n<items>\r\n\t<!-- Kept. -->\r\n\t<item id=\"aaa1\"><Q>old</Q></item>\r\n\t<item id=\"aaa2\">\r\n\t\t<Q>keep</Q>\r\n\t</item>\r\n</items>\r\n";
        let before = object("aaa1", &[("Q", "old")]);
        let replace = Change::Replace {
            collection: "a.xml".to_owned(),
            handle: "0".to_owned(),
            before,
            after: object("aaa1", &[("Q", "new"), ("A", "two\nlines")]),
        };
        let add = Change::Add {
            collection: "a.xml".to_owned(),
            item: object("aaa3", &[("Q", "added")]),
        };
        let (new, added) = changed("a.xml", Some(text), &[(0, &replace), (1, &add)]).unwrap();
        assert_eq!(
            new,
            "<?xml version=\"1.0\"?>\r\n<items>\r\n\t<!-- Kept. -->\r\n\t<item id=\"aaa1\">\r\n\t\t<Q>new</Q>\r\n\t\t<A>\r\n\t\t\ttwo\r\n\t\t\tlines\r\n\t\t</A>\r\n\t</item>\r\n\t<item id=\"aaa2\">\r\n\t\t<Q>keep</Q>\r\n\t</item>\r\n\t<item id=\"aaa3\">\r\n\t\t<Q>added</Q>\r\n\t</item>\r\n</items>\r\n"
        );
        assert_eq!(added, "<item id=\"aaa3\">\n\t\t<Q>added</Q>\n\t</item>\n");
    }

    #[test]
    fn items_are_added_to_a_new_an_empty_or_a_one_line_file() {
        let add = Change::Add {
            collection: "a.xml".to_owned(),
            item: object("aaa1", &[("Q", "q")]),
        };
        let expected = "<items>\n  <item id=\"aaa1\">\n    <Q>q</Q>\n  </item>\n</items>\n";
        assert_eq!(changed("a.xml", None, &[(0, &add)]).unwrap().0, expected);
        assert_eq!(
            changed("a.xml", Some("\n"), &[(0, &add)]).unwrap().0,
            expected
        );
        assert_eq!(
            changed("a.xml", Some("<items/>\n"), &[(0, &add)])
                .unwrap()
                .0,
            expected
        );
        assert_eq!(
            changed("a.xml", Some("<items></items>"), &[(0, &add)])
                .unwrap()
                .0,
            "<items>\n  <item id=\"aaa1\">\n    <Q>q</Q>\n  </item>\n</items>"
        );
    }
}
