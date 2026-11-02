//! A source program for `srl`'s tests (see `docs/sources.md`, "Source
//! programs"), so that they need no interpreter and run the same on every
//! platform. It keeps its items in a JSON file, and its settings, the keys
//! of its source in `[sources]`, decide what it does:
//!
//! - `data`: the file, relative to the folder it runs in:
//!   `{"next": 1, "collections": {"words": {"context": "…", "items":
//!   [{"handle": "h1", "item": {…}}]}}}`. A file that doesn't exist is a
//!   source without collections.
//! - `creates`: the globs `describe` gives; `["*"]` without it.
//! - `log`: each call appends a line to this file: its name and how many
//!   lines it got.
//! - `fail`: the call of this name fails as a whole, with `broken` on
//!   standard error.
//! - `raw_call` and `raw_output`: the call `raw_call` prints `raw_output`,
//!   as it is, in place of its answer.
//! - `refuse`: `write` answers every change with this error, and writes
//!   nothing.
//! - `lie`: `write` answers `ok` to every change, and writes nothing.
//!
//! An item's line is `null`, and its text as written is `null` too, so
//! `srl` shows its item object. A collection's context is its `context`,
//! in modes `prose` and `full`, to its own items and to those that name it.

use std::fs::{self, OpenOptions};
use std::io::{self, BufRead as _, Write as _};
use std::process::ExitCode;

use serde_json::{Map, Value, json};

#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a source program answers on standard output and standard error"
)]
fn main() -> ExitCode {
    let call = std::env::args().next_back().unwrap_or_default();
    let mut inputs = Vec::new();
    for line in io::stdin().lock().lines() {
        let parsed = line.map_err(|error| error.to_string()).and_then(|line| {
            serde_json::from_str::<Value>(&line).map_err(|error| error.to_string())
        });
        match parsed {
            Ok(Value::Object(input)) => inputs.push(input),
            Ok(other) => {
                eprintln!("an input line is not a JSON object: {other}");
                return ExitCode::FAILURE;
            }
            Err(error) => {
                eprintln!("cannot read an input line: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    let config = inputs
        .first()
        .and_then(|input| input.get("config"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let setting = |key: &str| config.get(key).and_then(Value::as_str);
    if let Some(log) = setting("log") {
        let logged = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .and_then(|mut file| writeln!(file, "{call} {}", inputs.len()));
        if let Err(error) = logged {
            eprintln!("cannot write the log {log}: {error}");
            return ExitCode::FAILURE;
        }
    }
    if setting("fail") == Some(call.as_str()) {
        eprintln!("broken");
        return ExitCode::FAILURE;
    }
    if setting("raw_call") == Some(call.as_str()) {
        print!("{}", setting("raw_output").unwrap_or_default());
        return ExitCode::SUCCESS;
    }
    match answer(&call, &config, &inputs) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!("{reason}");
            ExitCode::FAILURE
        }
    }
}

/// The value of `key` in `value`, or `null`.
fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&Value::Null)
}

/// The text of `key` in `input`, or nothing.
fn text<'a>(input: &'a Map<String, Value>, key: &str) -> &'a str {
    input.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The lines answering `call`, given `inputs`.
fn answer(
    call: &str,
    config: &Map<String, Value>,
    inputs: &[Map<String, Value>],
) -> Result<Vec<Value>, String> {
    let data_file = config
        .get("data")
        .and_then(Value::as_str)
        .ok_or("the setting `data` is missing; give the file the items are kept in")?;
    let mut data = match fs::read_to_string(data_file) {
        Ok(text) => serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            json!({"next": 1, "collections": {}})
        }
        Err(error) => return Err(format!("cannot read {data_file}: {error}")),
    };
    let collections = at(&data, "collections")
        .as_object()
        .cloned()
        .unwrap_or_default();
    match call {
        "describe" => Ok(vec![json!({
            "description": "Items in a JSON file, for srl's tests.",
            "creates": config.get("creates").cloned().unwrap_or(json!(["*"])),
        })]),
        "collections" => Ok(collections
            .keys()
            .map(|name| json!({"collection": name, "file": null}))
            .collect()),
        "read" => Ok(collections
            .iter()
            .flat_map(|(name, collection)| {
                at(collection, "items")
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |item| {
                        json!({"collection": name, "handle": at(&item, "handle"), "line": null,
                               "written": null, "item": at(&item, "item")})
                    })
            })
            .collect()),
        "context" => Ok(inputs
            .iter()
            .map(|input| {
                let context = collections
                    .get(text(input, "collection"))
                    .map_or(&Value::Null, |collection| at(collection, "context"));
                json!({"context": context})
            })
            .collect()),
        "write" => write(config, inputs, &mut data, data_file),
        _ => Err(format!("`{call}` is not a call")),
    }
}

/// Makes the changes of `inputs` to `data`, and writes it to `data_file`,
/// unless one is refused, it is a dry run, or the settings say otherwise.
fn write(
    config: &Map<String, Value>,
    inputs: &[Map<String, Value>],
    data: &mut Value,
    data_file: &str,
) -> Result<Vec<Value>, String> {
    if let Some(reason) = config.get("refuse").and_then(Value::as_str) {
        return Ok(inputs.iter().map(|_| json!({"error": reason})).collect());
    }
    let dry_run = inputs
        .first()
        .is_some_and(|input| input.get("dry_run") == Some(&json!(true)));
    let mut answers = Vec::new();
    let mut refused = false;
    for input in inputs {
        let name = text(input, "collection").to_owned();
        let next = data.get("next").and_then(Value::as_u64).unwrap_or(1);
        if text(input, "op") == "add" {
            data["next"] = json!(next + 1);
        }
        let collections = data
            .get_mut("collections")
            .and_then(Value::as_object_mut)
            .ok_or("`collections` is not an object")?;
        match text(input, "op") {
            "add" => {
                let collection = collections
                    .entry(name)
                    .or_insert(json!({"context": null, "items": []}));
                let items = collection
                    .get_mut("items")
                    .and_then(Value::as_array_mut)
                    .ok_or("a collection's `items` is not a list")?;
                let item = input.get("item").cloned().unwrap_or_default();
                items.push(json!({"handle": format!("h{next}"), "item": item}));
                answers.push(if dry_run {
                    json!({"preview": item.to_string()})
                } else {
                    json!({"ok": true})
                });
            }
            "replace" => {
                let found = collections
                    .get_mut(&name)
                    .and_then(|collection| collection.get_mut("items"))
                    .and_then(Value::as_array_mut)
                    .and_then(|items| {
                        items
                            .iter_mut()
                            .find(|item| item.get("handle") == input.get("handle"))
                    })
                    .filter(|item| item.get("item") == input.get("before"));
                if let Some(item) = found {
                    item["item"] = input.get("after").cloned().unwrap_or_default();
                    answers.push(json!({"ok": true}));
                } else {
                    refused = true;
                    answers.push(json!({"error": "the item changed since it was read"}));
                }
            }
            op => return Err(format!("`{op}` is not a change")),
        }
    }
    let lie = config.get("lie") == Some(&json!(true));
    if !dry_run && !refused && !lie {
        fs::write(data_file, data.to_string())
            .map_err(|error| format!("cannot write {data_file}: {error}"))?;
    }
    Ok(answers)
}
