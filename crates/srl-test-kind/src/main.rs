//! A kind program for `srl`'s tests (see `docs/cards.md`, "Kind
//! programs"), so that they need no interpreter and run the same on every
//! platform. Like every kind program, it reads one JSON object per line up
//! to the end of standard input, and prints one line per input line. What it
//! does is decided by the fields of the items it gets:
//!
//! - `INVALID`: `validate` gives this error, about no field.
//! - `ERRORS`: printed as is for `validate`, in place of its answer.
//! - `GROUPS`: the item's cards, groups separated by `|` and suffixes by
//!   spaces, `_` for the single card with the item's ID. Without it, one
//!   card with the item's ID.
//! - `GROUPS_RAW`: printed as is for `groups`, in place of the groups.
//! - `FAIL_MAKE`: `make` gives an error for the suffixes listed (`_` for
//!   none).
//! - `SLEEP_MAKE`: `make` waits this many seconds first, once per call.
//! - `MAKE_RAW`: printed as is for `make`, in place of the card.
//! - `BREAK_VALIDATE`, `BREAK_GROUPS`, `BREAK_MAKE`: that call fails as a
//!   whole, with this text on standard error.
//! - `SHORT`: `validate` prints no line for this item.
//! - `LOG`: each call appends a line to this file, relative to the folder
//!   it runs in: its name and how many lines it got.
//!
//! The card made for suffix `S` has the prompt `S?`, the revealed prompt
//! `S!`, and an answer that tells what the program got: the study day, the
//! context and its format, and the name of the folder it ran in.
//!
//! `describe` has no item, so an argument before the call decides it
//! instead: `fail` makes it fail, and any other text is printed as is, in
//! place of the description of the fields above.

use std::fs::OpenOptions;
use std::io::{self, BufRead as _, Write as _};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a kind program answers on standard output and standard error"
)]
fn main() -> ExitCode {
    // srl adds the call last, after the arguments of `[kinds]`.
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let call = args.pop().unwrap_or_default();
    let describe = args.first().map(String::as_str);
    let mut inputs = Vec::new();
    for line in io::stdin().lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("cannot read standard input: {error}");
                return ExitCode::FAILURE;
            }
        };
        match serde_json::from_str::<Value>(&line) {
            Ok(input) => inputs.push(input),
            Err(error) => {
                eprintln!("a line of standard input is not JSON: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    match answer(&call, &inputs, describe) {
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

/// The text of field `key` of `input`, if it has it.
fn field<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get("fields")?.get(key)?.as_str()
}

/// The lines the call `call` prints for `inputs`, or why it fails as a
/// whole. `describe` is the argument that decides the `describe` call, if
/// there is one.
fn answer(call: &str, inputs: &[Value], describe: Option<&str>) -> Result<Vec<String>, String> {
    if let Some(path) = inputs.iter().find_map(|input| field(input, "LOG")) {
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| format!("LOG: {error}"))?;
        writeln!(log, "{call} {}", inputs.len()).map_err(|error| format!("LOG: {error}"))?;
    }
    let broken = match call {
        "validate" => Some("BREAK_VALIDATE"),
        "groups" => Some("BREAK_GROUPS"),
        "make" => Some("BREAK_MAKE"),
        _ => None,
    };
    if let Some(reason) = broken.and_then(|key| inputs.iter().find_map(|input| field(input, key))) {
        return Err(reason.to_owned());
    }
    match call {
        "describe" => Ok(vec![match describe {
            Some("fail") => return Err("describe fails on purpose".to_owned()),
            Some(raw) => raw.to_owned(),
            None => json!({
                "description": "A kind for srl's tests.",
                "fields": [
                    {"name": "GROUPS", "description": "The cards, groups separated by |."},
                    {"name": "FAIL_MAKE", "description": "The suffixes whose card fails."},
                ],
            })
            .to_string(),
        }]),
        "validate" => Ok(inputs
            .iter()
            .filter(|input| field(input, "SHORT").is_none())
            .map(|input| {
                if let Some(raw) = field(input, "ERRORS") {
                    return raw.to_owned();
                }
                let errors = field(input, "INVALID")
                    .map(|message| vec![json!({"field": null, "message": message})])
                    .unwrap_or_default();
                json!({ "errors": errors }).to_string()
            })
            .collect()),
        "groups" => Ok(inputs.iter().map(groups).collect()),
        "make" => {
            if let Some(seconds) = inputs.iter().find_map(|input| field(input, "SLEEP_MAKE")) {
                let seconds = seconds
                    .parse()
                    .map_err(|error| format!("SLEEP_MAKE: {error}"))?;
                thread::sleep(Duration::from_secs(seconds));
            }
            inputs.iter().map(make).collect()
        }
        other => Err(format!("unknown call `{other}`")),
    }
}

/// What `groups` prints for `input`.
fn groups(input: &Value) -> String {
    if let Some(raw) = field(input, "GROUPS_RAW") {
        return raw.to_owned();
    }
    let groups = field(input, "GROUPS")
        .unwrap_or("_")
        .split('|')
        .map(|group| {
            group
                .split_whitespace()
                .map(|suffix| if suffix == "_" { None } else { Some(suffix) })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    json!({ "groups": groups }).to_string()
}

/// What `make` prints for `input`, or why the call fails as a whole.
fn make(input: &Value) -> Result<String, String> {
    let suffix = input.get("suffix").and_then(Value::as_str).unwrap_or("_");
    if field(input, "FAIL_MAKE")
        .is_some_and(|failing| failing.split_whitespace().any(|s| s == suffix))
    {
        return Ok(json!({ "error": format!("card {suffix} fails on purpose") }).to_string());
    }
    if let Some(raw) = field(input, "MAKE_RAW") {
        return Ok(raw.to_owned());
    }
    let folder = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let get = |key: &str| input.get(key).unwrap_or(&Value::Null);
    Ok(json!({
        "prompt": format!("{suffix}?"),
        "prompt_revealed": format!("{suffix}!"),
        "answer": format!(
            "day {}; context {} ({}); in {folder}",
            get("day"),
            get("context"),
            get("context_format")
        ),
    })
    .to_string())
}
