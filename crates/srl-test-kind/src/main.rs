//! A kind program for `srl`'s tests (see `docs/cards.md`, "Kind
//! programs"), so that they need no interpreter and run the same on every
//! platform. What it does is decided by the fields of the item it gets:
//!
//! - `GROUPS`: its cards, groups separated by `|` and suffixes by spaces,
//!   `_` for the single card with the item's ID. Without it, one card with
//!   the item's ID.
//! - `IDS`: printed as is for `ids`, in place of the groups.
//! - `FAIL_IDS`: `ids` fails, with this text on standard error.
//! - `FAIL_CARD`: `card` fails for the suffixes listed (`_` for none).
//! - `SLEEP_CARD`: `card` waits this many seconds first.
//! - `CARD`: printed as is for `card`, in place of the card.
//!
//! `describe` has no item, so an argument before the call decides it
//! instead: `fail` makes it fail, and any other text is printed as is, in
//! place of the description of the fields above.
//!
//! The card made for suffix `S` has the prompt `S?`, the revealed prompt
//! `S!`, and an answer that tells what the program got: the study day, the
//! context, and the name of the folder it ran in.

use std::io::{self, Read as _};
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
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read standard input: {error}");
        return ExitCode::FAILURE;
    }
    let input = match serde_json::from_str::<Value>(&input) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("standard input is not JSON: {error}");
            return ExitCode::FAILURE;
        }
    };
    match answer(&call, &input, describe) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!("{reason}");
            ExitCode::FAILURE
        }
    }
}

/// What the call `call` prints for `input`, or why it fails. `describe` is
/// the argument that decides the `describe` call, if there is one.
fn answer(call: &str, input: &Value, describe: Option<&str>) -> Result<String, String> {
    let get = |key: &str| input.get(key).unwrap_or(&Value::Null);
    let field = |key: &str| get("fields").get(key).and_then(Value::as_str);
    match call {
        "ids" => {
            if let Some(reason) = field("FAIL_IDS") {
                return Err(reason.to_owned());
            }
            if let Some(raw) = field("IDS") {
                return Ok(raw.to_owned());
            }
            let groups = field("GROUPS")
                .unwrap_or("_")
                .split('|')
                .map(|group| {
                    let cards = group
                        .split_whitespace()
                        .map(|suffix| if suffix == "_" { None } else { Some(suffix) })
                        .collect::<Vec<_>>();
                    json!({ "cards": cards })
                })
                .collect::<Vec<_>>();
            Ok(json!({ "groups": groups }).to_string())
        }
        "card" => {
            let suffix = get("suffix").as_str().unwrap_or("_");
            if let Some(seconds) = field("SLEEP_CARD") {
                let seconds = seconds
                    .parse()
                    .map_err(|error| format!("SLEEP_CARD: {error}"))?;
                thread::sleep(Duration::from_secs(seconds));
            }
            if field("FAIL_CARD")
                .is_some_and(|failing| failing.split_whitespace().any(|s| s == suffix))
            {
                return Err(format!("card {suffix} fails on purpose"));
            }
            if let Some(raw) = field("CARD") {
                return Ok(raw.to_owned());
            }
            let folder = std::env::current_dir()
                .map_err(|error| error.to_string())?
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(json!({
                "prompt": format!("{suffix}?"),
                "prompt_revealed": format!("{suffix}!"),
                "answer": format!(
                    "day {}; context {}; in {folder}",
                    get("day"),
                    get("context")
                ),
            })
            .to_string())
        }
        "describe" => match describe {
            Some("fail") => Err("describe fails on purpose".to_owned()),
            Some(raw) => Ok(raw.to_owned()),
            None => Ok(json!({
                "description": "A kind for srl's tests.",
                "fields": [
                    {"name": "GROUPS", "description": "The cards, groups separated by |."},
                    {"name": "FAIL_CARD", "description": "The suffixes whose card fails."},
                ],
            })
            .to_string()),
        },
        other => Err(format!("unknown call `{other}`")),
    }
}
