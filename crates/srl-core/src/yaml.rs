//! Converting front-matter between YAML text and JSON values.
//!
//! Atoms are handled as JSON values everywhere in the core, because JSON is
//! what `--json` prints and what `srl add --stdin` reads. YAML only appears
//! when reading or writing an atom file. Parsing follows YAML 1.2, so `no`,
//! `yes`, `on` and `off` stay strings (see `docs/rust.md`, "Front-matter
//! editing").

use std::borrow::Cow;

use saphyr::{LoadableYamlNode, Mapping, Scalar, Yaml, YamlEmitter};
use serde_json::{Map, Number, Value};

/// Parses front-matter text into its keys and values, in file order.
///
/// Errors are plain reasons, to be put into a sentence by the caller.
pub(crate) fn parse_mapping(text: &str) -> Result<Map<String, Value>, String> {
    let docs = Yaml::load_from_str(text).map_err(|error| format!("invalid YAML: {error}"))?;
    match docs.as_slice() {
        [] => Ok(Map::new()),
        [doc] => match to_json(doc)? {
            Value::Object(map) => Ok(map),
            Value::Null => Ok(Map::new()),
            Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
                Err("the front-matter must be a mapping of keys to values".to_owned())
            }
        },
        [_, _, ..] => Err("the front-matter must be a single YAML document".to_owned()),
    }
}

/// Writes keys and values as YAML front-matter text, without the `---` lines.
/// The text ends with a newline. A key in `flow` whose value is a list of
/// text is written on one line, as `tags: [a, b]`.
pub(crate) fn render_mapping(map: &Map<String, Value>, flow: &[&str]) -> Result<String, String> {
    map.iter()
        .map(|(key, value)| {
            value
                .as_array()
                .filter(|_| flow.contains(&key.as_str()))
                .map_or_else(|| block_entry(key, value), |items| flow_entry(key, items))
        })
        .collect()
}

/// `key: value` as YAML in block style, and a newline.
fn block_entry(key: &str, value: &Value) -> Result<String, String> {
    let entry = Map::from_iter([(key.to_owned(), value.clone())]);
    Ok(format!("{}\n", emit(&Yaml::Mapping(to_mapping(&entry)?))?))
}

/// `key: [a, b]` and a newline.
fn flow_entry(key: &str, items: &[Value]) -> Result<String, String> {
    let items = items
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| format!("cannot write {item} in a one-line list"))
                .and_then(|text| emit(&string(text)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{}: [{}]\n", emit(&string(key))?, items.join(", ")))
}

/// A string as one YAML scalar on one line, quoted only where YAML needs it.
pub(crate) fn render_scalar(text: &str) -> Result<String, String> {
    let rendered = emit(&string(text))?;
    if rendered.contains('\n') {
        Err(format!("`{text}` cannot be written on one line"))
    } else {
        Ok(rendered)
    }
}

/// One YAML node as text, without the leading `---` line or a final newline.
fn emit(node: &Yaml<'_>) -> Result<String, String> {
    let mut text = String::new();
    let mut emitter = YamlEmitter::new(&mut text);
    emitter.multiline_strings(true);
    emitter
        .dump(node)
        .map_err(|error| format!("cannot write YAML: {error}"))?;
    text.strip_prefix("---\n")
        .map(str::to_owned)
        .ok_or_else(|| "the YAML writer did not start with `---`".to_owned())
}

fn to_json(yaml: &Yaml<'_>) -> Result<Value, String> {
    match yaml {
        Yaml::Value(scalar) => scalar_to_json(scalar),
        Yaml::Sequence(items) => items
            .iter()
            .map(to_json)
            .collect::<Result<_, _>>()
            .map(Value::Array),
        Yaml::Mapping(mapping) => mapping
            .iter()
            .map(|(key, value)| Ok((key_to_string(key)?, to_json(value)?)))
            .collect::<Result<_, String>>()
            .map(Value::Object),
        Yaml::Tagged(tag, _) => Err(format!("YAML tags such as `{tag}` are not supported")),
        Yaml::Alias(_) => Err("YAML aliases (`*name`) are not supported".to_owned()),
        Yaml::Representation(text, _, _) => Err(format!("cannot read the value `{text}`")),
        Yaml::BadValue => Err("a value is not valid YAML".to_owned()),
    }
}

fn scalar_to_json(scalar: &Scalar<'_>) -> Result<Value, String> {
    match scalar {
        Scalar::Null => Ok(Value::Null),
        Scalar::Boolean(value) => Ok(Value::Bool(*value)),
        Scalar::Integer(value) => Ok(Value::Number((*value).into())),
        Scalar::FloatingPoint(value) => Number::from_f64(value.into_inner())
            .map(Value::Number)
            .ok_or_else(|| format!("the number {value} cannot be represented in JSON")),
        Scalar::String(text) => Ok(Value::String(text.to_string())),
    }
}

fn key_to_string(key: &Yaml<'_>) -> Result<String, String> {
    match key {
        Yaml::Value(Scalar::String(text)) => Ok(text.to_string()),
        Yaml::Value(_)
        | Yaml::Sequence(_)
        | Yaml::Mapping(_)
        | Yaml::Tagged(..)
        | Yaml::Alias(_)
        | Yaml::Representation(..)
        | Yaml::BadValue => Err(format!(
            "every key must be text, but one is {key:?}; quote it"
        )),
    }
}

fn to_mapping(map: &Map<String, Value>) -> Result<Mapping<'static>, String> {
    map.iter()
        .map(|(key, value)| Ok((string(key), to_yaml(value)?)))
        .collect()
}

fn to_yaml(value: &Value) -> Result<Yaml<'static>, String> {
    match value {
        Value::Null => Ok(Yaml::Value(Scalar::Null)),
        Value::Bool(value) => Ok(Yaml::Value(Scalar::Boolean(*value))),
        Value::Number(number) => number_to_yaml(number),
        Value::String(text) => Ok(string(text)),
        Value::Array(items) => items
            .iter()
            .map(to_yaml)
            .collect::<Result<_, _>>()
            .map(Yaml::Sequence),
        Value::Object(map) => to_mapping(map).map(Yaml::Mapping),
    }
}

/// A number, through its text: JSON and YAML 1.2 write numbers the same way.
fn number_to_yaml(number: &Number) -> Result<Yaml<'static>, String> {
    match Yaml::value_from_str(&number.to_string()) {
        Yaml::Value(Scalar::Integer(value)) => Ok(Yaml::Value(Scalar::Integer(value))),
        Yaml::Value(Scalar::FloatingPoint(value)) => Ok(Yaml::Value(Scalar::FloatingPoint(value))),
        Yaml::Value(Scalar::Null | Scalar::Boolean(_) | Scalar::String(_))
        | Yaml::Sequence(_)
        | Yaml::Mapping(_)
        | Yaml::Tagged(..)
        | Yaml::Alias(_)
        | Yaml::Representation(..)
        | Yaml::BadValue => Err(format!("the number {number} cannot be written as YAML")),
    }
}

fn string(text: &str) -> Yaml<'static> {
    Yaml::Value(Scalar::String(Cow::Owned(text.to_owned())))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(text: &str) -> Value {
        Value::Object(parse_mapping(text).unwrap())
    }

    #[test]
    fn yaml_1_1_booleans_stay_strings() {
        assert_eq!(
            parse("cue: no\na: yes\nb: on\nc: off\nd: true\n"),
            json!({"cue": "no", "a": "yes", "b": "on", "c": "off", "d": true})
        );
    }

    #[test]
    fn key_order_is_kept() {
        let map = parse_mapping("z: 1\na: 2\nm: 3\n").unwrap();
        assert_eq!(map.keys().collect::<Vec<_>>(), ["z", "a", "m"]);
    }

    #[test]
    fn duplicate_keys_are_an_error() {
        assert!(parse_mapping("a: 1\na: 2\n").is_err());
    }

    #[test]
    fn non_text_keys_are_an_error() {
        assert!(parse_mapping("1: a\n").is_err());
    }

    #[test]
    fn a_scalar_document_is_an_error() {
        assert!(parse_mapping("just text\n").is_err());
    }

    #[test]
    fn empty_text_is_an_empty_mapping() {
        assert_eq!(parse(""), json!({}));
    }

    #[test]
    fn rendered_values_parse_back_the_same() {
        let value = json!({
            "cue": "no",
            "answer": "まぐろ — tuna",
            "tags": ["japanese/grammar", "a, b", "[x]"],
            "n": 3,
            "x": 1.5,
            "f": 1.0,
            "big": -9_000_000_000_i64,
            "quote": "it's \"quoted\": yes # not a comment",
            "lines": "first\nsecond\n",
            "empty": "",
            "nothing": null,
            "nested": [{"q": "Which particle?", "a": "を", "accept": ["を"]}],
            "numeric": "123",
            "flag": false,
        });
        let Value::Object(map) = &value else {
            panic!("the test value is an object")
        };
        let text = render_mapping(map, &["tags"]).unwrap();
        assert_eq!(parse(&text), value, "rendered as:\n{text}");
    }
}
