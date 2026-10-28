//! The item object: an item as one JSON object, its header entries and its
//! fields side by side (see `docs/notes.md`, "The item object"). It is a
//! line of a JSONL note, a line of `srl add --stdin` and `srl edit
//! --stdin`, and, with the keys only `srl` gives, what `srl show --json`
//! prints.

use std::fmt;

use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::{Map, Value};

use crate::note::{Entries, Field, ItemId, is_field_key};

/// The keys that `srl show --json` prints and nothing reads: they are
/// ignored wherever an item object is read.
pub(crate) const READ_ONLY: [&str; 5] = ["path", "line", "cards", "context", "context_format"];

/// An item object as read: its entries and its fields, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Object {
    pub(crate) entries: Entries,
    pub(crate) fields: Vec<Field>,
}

/// Reads `text` as an item object. `"kind": "qa"`, `"see": null` and
/// `"draft": false` mean the same as leaving the key out. Whether the kind
/// exists, the fields suit it and `see` names a note is checked later.
/// Errors are plain reasons.
pub(crate) fn read(text: &str) -> Result<Object, String> {
    let Pairs(pairs) = serde_json::from_str::<Pairs>(text)
        .map_err(|error| format!("it is not a JSON object ({error})"))?;
    let mut seen = Vec::new();
    let mut object = Object::default();
    for (key, value) in pairs {
        if seen.contains(&key) {
            return Err(format!("the key `{key}` appears twice; keep one"));
        }
        seen.push(key.clone());
        match key.as_str() {
            "id" => {
                let Value::String(id) = value else {
                    return Err(format!("the value of `id` must be text, not {value}"));
                };
                let id = ItemId::parse(&id).map_err(|error| error.to_string())?;
                object.entries.id = Some(id);
            }
            "kind" => {
                let text = text_of(&key, value)?;
                object.entries.kind = (text != "qa").then_some(text);
            }
            "see" => {
                if !value.is_null() {
                    object.entries.see = Some(text_of(&key, value)?);
                }
            }
            "draft" => {
                let Value::Bool(draft) = value else {
                    return Err(format!(
                        "the value of `draft` must be true or false, not {value}"
                    ));
                };
                object.entries.draft = draft;
            }
            key if READ_ONLY.contains(&key) => {}
            key if is_field_key(key) => {
                let Value::String(text) = value else {
                    return Err(format!("the value of `{key}` must be text, not {value}"));
                };
                object.fields.push(Field {
                    key: key.to_owned(),
                    text,
                });
            }
            key => {
                return Err(format!(
                    "`{key}` is neither a header entry (`id`, `kind`, `see`, `draft`), a field \
                     name (uppercase letters, digits and _, starting with a letter), nor a key \
                     only srl gives (`path`, `line`, `cards`, `context`, `context_format`)"
                ));
            }
        }
    }
    if object.fields.is_empty() {
        return Err("it has no fields".to_owned());
    }
    Ok(object)
}

/// The text value of `key`, which can't be empty.
fn text_of(key: &str, value: Value) -> Result<String, String> {
    let Value::String(text) = value else {
        return Err(format!("the value of `{key}` must be text, not {value}"));
    };
    if text.is_empty() {
        return Err(format!("the value of `{key}` can't be empty"));
    }
    Ok(text)
}

/// The item object of `entries` and `fields` as `srl` writes it into a
/// JSONL note: the entries that differ from their defaults, then the
/// fields.
pub(crate) fn written(entries: &Entries, fields: &[Field]) -> Map<String, Value> {
    let mut object = Map::new();
    if let Some(id) = &entries.id {
        object.insert("id".to_owned(), Value::from(id.as_str()));
    }
    if let Some(kind) = &entries.kind {
        object.insert("kind".to_owned(), Value::from(kind.as_str()));
    }
    if let Some(see) = &entries.see {
        object.insert("see".to_owned(), Value::from(see.as_str()));
    }
    if entries.draft {
        object.insert("draft".to_owned(), Value::Bool(true));
    }
    for field in fields {
        object.insert(field.key.clone(), Value::from(field.text.as_str()));
    }
    object
}

/// A JSON object's keys and values as written, repeated keys included,
/// which a [`Map`] would quietly merge.
struct Pairs(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Pairs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(PairsVisitor)
    }
}

struct PairsVisitor;

impl<'de> Visitor<'de> for PairsVisitor {
    type Value = Pairs;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pairs, A::Error> {
        let mut pairs = Vec::new();
        while let Some(pair) = map.next_entry::<String, Value>()? {
            pairs.push(pair);
        }
        Ok(Pairs(pairs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_and_fields_are_read_in_order() {
        let object = read(
            r#"{"id": "abc", "kind": "words", "see": "a b.md", "draft": true, "Q": "q", "A": "a", "path": "x", "line": 3, "cards": [], "context": null, "context_format": null}"#,
        )
        .unwrap();
        assert_eq!(object.entries.id.unwrap().as_str(), "abc");
        assert_eq!(object.entries.kind.as_deref(), Some("words"));
        assert_eq!(object.entries.see.as_deref(), Some("a b.md"));
        assert!(object.entries.draft);
        let keys = object.fields.iter().map(|field| field.key.as_str());
        assert_eq!(keys.collect::<Vec<_>>(), ["Q", "A"]);
    }

    #[test]
    fn defaults_mean_the_same_as_leaving_the_key_out() {
        assert_eq!(
            read(r#"{"kind": "qa", "see": null, "draft": false, "Q": "q"}"#).unwrap(),
            read(r#"{"Q": "q"}"#).unwrap()
        );
    }

    #[test]
    fn anything_else_is_refused() {
        for text in [
            "[1]",
            "{}",
            "not json",
            r#"{"see": "a.md"}"#,
            r#"{"Q": "q", "Q": "r"}"#,
            r#"{"Q": 1}"#,
            r#"{"q": "q"}"#,
            r#"{"Q": "q", "id": "ABC"}"#,
            r#"{"Q": "q", "id": 123}"#,
            r#"{"Q": "q", "kind": ""}"#,
            r#"{"Q": "q", "see": ""}"#,
            r#"{"Q": "q", "draft": "yes"}"#,
            r#"{"Q": "q", "drfat": true}"#,
        ] {
            assert!(read(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn only_entries_that_differ_from_their_defaults_are_written() {
        let object = read(r#"{"Q": "q", "see": null, "id": "abc", "draft": false}"#).unwrap();
        assert_eq!(
            Value::Object(written(&object.entries, &object.fields)).to_string(),
            r#"{"id":"abc","Q":"q"}"#
        );
    }
}
