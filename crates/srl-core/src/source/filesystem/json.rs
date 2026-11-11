//! JSON collections: one array of item objects (see `docs/formats.md`,
//! "JSON"). `srl` writes the whole array again, two spaces of indentation,
//! each item not changed with the keys and values it was read with, so the
//! file's layout is not kept, but what it says is.

use std::fmt;

use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

use crate::domain::item::object::{self, Object, pairs_of};
use crate::source::files::{
    Keyed, Parsed, ParsedItem, changed_meanwhile, line_ending, position_of, verified, without,
};
use crate::source::{Change, Refusal};

/// The elements of the array, each with the item object it is, if it is
/// one.
fn read(text: &str) -> Result<Vec<Element>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str::<Vec<Element>>(text)
        .map_err(|error| format!("it is not a JSON array ({error})"))
}

/// Every element of the array is an item.
pub(super) fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed {
        items: Vec::new(),
        errors: Vec::new(),
    };
    match read(text) {
        Err(reason) => parsed.errors.push(reason),
        Ok(elements) => {
            for (index, element) in elements.into_iter().enumerate() {
                match element.item() {
                    Ok((Object { entries, fields }, _)) => {
                        parsed.items.push(ParsedItem { entries, fields });
                    }
                    Err(reason) => parsed.errors.push(format!("item {}: {reason}", index + 1)),
                }
            }
        }
    }
    parsed
}

/// The JSON file at `path`, `old`, with `changes` made to it and checked,
/// and the items added, as written. The array is written again whole,
/// without the items removed, unless nothing changes.
pub(super) fn changed(
    path: &str,
    old: Option<&str>,
    changes: &[(usize, &Change)],
) -> Result<(String, String), Refusal> {
    let text = old.unwrap_or_default();
    // A file with an error now changed since it was read without one.
    let elements = read(text).map_err(|_reason| changed_meanwhile(path))?;
    let mut expected = Vec::<Keyed>::new();
    let mut objects = Vec::new();
    for element in elements {
        let (object, written) = element.item().map_err(|_reason| changed_meanwhile(path))?;
        expected.push((object.entries, object.fields));
        objects.push(written);
    }
    let mut any = false;
    let mut added = Vec::new();
    // Positions among the items as read, removed once every other change
    // is made, so that positions stay those of the file read.
    let mut removed = Vec::new();
    for &(_, change) in changes {
        match change {
            Change::Remove { handle, before, .. } => {
                if old.is_none() {
                    return Err(changed_meanwhile(path));
                }
                any = true;
                removed.push(position_of(&expected, handle, before, path)?);
            }
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
                    any = true;
                    let written = object::written(&after.entries, &after.fields);
                    *objects
                        .get_mut(position)
                        .expect("the position of an item should be among the file's items") =
                        written;
                }
                *expected
                    .get_mut(position)
                    .expect("the position of an item should be among the file's items") =
                    (after.entries.clone(), after.fields.clone());
            }
            Change::Add { item, .. } => {
                any = true;
                let written = object::written(&item.entries, &item.fields);
                added.push(pretty(&written));
                objects.push(written);
                expected.push((item.entries.clone(), item.fields.clone()));
            }
        }
    }
    let expected = without(expected, &removed);
    let objects = without(objects, &removed);
    let new = if any || old.is_none() {
        written(text, &objects)
    } else {
        text.to_owned()
    };
    let parsed = parse(&new);
    verified(
        path,
        parsed.errors.is_empty() && crate::source::files::keyed(parsed) == expected,
    )?;
    let added = added.iter().fold(String::new(), |mut all, item| {
        all.push_str(item);
        all.push('\n');
        all
    });
    Ok((new, added))
}

/// The array of `objects`, as `srl` writes a JSON file that held `old`:
/// two spaces of indentation, in the line ending of `old`, with its byte
/// order mark, ending with a line ending.
fn written(old: &str, objects: &[Map<String, Value>]) -> String {
    let array = Value::Array(objects.iter().cloned().map(Value::Object).collect());
    let text = serde_json::to_string_pretty(&array).expect("JSON values should be written");
    let bom = if old.starts_with('\u{feff}') {
        "\u{feff}"
    } else {
        ""
    };
    format!("{bom}{text}\n").replace('\n', line_ending(old))
}

/// `object` with two spaces of indentation.
fn pretty(object: &Map<String, Value>) -> String {
    serde_json::to_string_pretty(object).expect("JSON values should be written")
}

/// An element of the array: the keys and values of an object, as written,
/// or something that is not an object.
struct Element(Option<Vec<(String, Value)>>);

impl Element {
    /// The item object this element is, and its keys and values, which
    /// [`object::from_pairs`] read without a repeated key; or why it is no
    /// item.
    fn item(self) -> Result<(Object, Map<String, Value>), String> {
        let pairs = self.0.ok_or("it is not a JSON object")?;
        let object = object::from_pairs(pairs.clone())?;
        Ok((object, pairs.into_iter().collect()))
    }
}

impl<'de> Deserialize<'de> for Element {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ElementVisitor)
    }
}

struct ElementVisitor;

impl<'de> Visitor<'de> for ElementVisitor {
    type Value = Element;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Element, A::Error> {
        pairs_of(map).map(|pairs| Element(Some(pairs)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Element, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Element(None))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Element, E> {
        Ok(Element(None))
    }

    fn visit_i64<E>(self, _: i64) -> Result<Element, E> {
        Ok(Element(None))
    }

    fn visit_u64<E>(self, _: u64) -> Result<Element, E> {
        Ok(Element(None))
    }

    fn visit_f64<E>(self, _: f64) -> Result<Element, E> {
        Ok(Element(None))
    }

    fn visit_str<E>(self, _: &str) -> Result<Element, E> {
        Ok(Element(None))
    }

    fn visit_unit<E>(self) -> Result<Element, E> {
        Ok(Element(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_element_of_the_array_is_an_item() {
        let text = "\u{feff}[\n  {\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"},\n  {\"Q\": \"r\", \"Q\": \"s\"},\n  [1],\n  {\"Q\": \"t\", \"kind\": \"qa\"}\n]";
        let parsed = parse(text);
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].fields[1].text, "a");
        assert_eq!(parsed.errors.len(), 2, "{:?}", parsed.errors);
        assert!(parsed.errors[0].starts_with("item 2: the key `Q` appears twice"));
        assert_eq!(parsed.errors[1], "item 3: it is not a JSON object");
        assert_eq!(parse("").errors, [] as [String; 0]);
        assert_eq!(parse(" \n").items, []);
        for bad in ["{}", "[", "{\"Q\": \"q\"}", "1"] {
            assert_eq!(parse(bad).errors.len(), 1, "{bad}");
        }
    }

    #[test]
    fn changes_write_the_array_again_keeping_unchanged_items_as_read() {
        let text = "[{\"id\": \"abc\", \"kind\": \"qa\", \"Q\": \"q\", \"A\": \"a\"},\r\n {\"id\": \"abd\", \"Q\": \"r\", \"A\": \"b\"}]";
        let parsed = parse(text);
        let before = Object {
            entries: parsed.items[1].entries.clone(),
            fields: parsed.items[1].fields.clone(),
        };
        let mut after = before.clone();
        after.entries.draft = true;
        let replace = Change::Replace {
            collection: "a.json".to_owned(),
            handle: "1".to_owned(),
            before: before.clone(),
            after,
        };
        let (new, added) = changed("a.json", Some(text), &[(0, &replace)]).unwrap();
        assert_eq!(
            new,
            "[\r\n  {\r\n    \"id\": \"abc\",\r\n    \"kind\": \"qa\",\r\n    \"Q\": \"q\",\r\n    \"A\": \"a\"\r\n  },\r\n  {\r\n    \"id\": \"abd\",\r\n    \"draft\": true,\r\n    \"Q\": \"r\",\r\n    \"A\": \"b\"\r\n  }\r\n]\r\n"
        );
        assert_eq!(added, "");
        // Changing nothing writes nothing.
        let same = Change::Replace {
            collection: "a.json".to_owned(),
            handle: "1".to_owned(),
            before: before.clone(),
            after: before.clone(),
        };
        assert_eq!(
            changed("a.json", Some(text), &[(0, &same)]).unwrap().0,
            text
        );
        // A new file is an array of its items.
        let add = Change::Add {
            collection: "b.json".to_owned(),
            item: before,
        };
        let (new, added) = changed("b.json", None, &[(0, &add)]).unwrap();
        assert_eq!(
            new,
            "[\n  {\n    \"id\": \"abd\",\n    \"Q\": \"r\",\n    \"A\": \"b\"\n  }\n]\n"
        );
        assert_eq!(
            added,
            "{\n  \"id\": \"abd\",\n  \"Q\": \"r\",\n  \"A\": \"b\"\n}\n"
        );
    }

    #[test]
    fn a_removed_item_leaves_the_array() {
        let text = "[{\"id\": \"aaa1\", \"Q\": \"q\"}, {\"id\": \"aaa2\", \"Q\": \"r\"}]";
        let item = parse(text).items.remove(1);
        let remove = Change::Remove {
            collection: "a.json".to_owned(),
            handle: "1".to_owned(),
            before: Object {
                entries: item.entries,
                fields: item.fields,
            },
        };
        let (new, _) = changed("a.json", Some(text), &[(0, &remove)]).unwrap();
        assert_eq!(
            new,
            "[\n  {\n    \"id\": \"aaa1\",\n    \"Q\": \"q\"\n  }\n]\n"
        );
    }
}
