//! A source program for `srl`'s tests (see `docs/sources.md`, "Source
//! programs"), so that they need no interpreter and run the same on every
//! platform. It keeps its items in a JSON file, and its settings, the keys
//! of its source in `[sources]`, decide what it does:
//!
//! - `data`: the file, relative to the folder it runs in:
//!   `{"next": 1, "collections": {"vocab/words": {"items": [{"handle": "h1",
//!   "item": {…}}]}}}`. A file that doesn't exist is a source without
//!   collections.
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
//! - `url`: in place of `data`, a web service at `http://host:port` that
//!   keeps the same JSON as one resource, `/data`: `GET` gives it with an
//!   `ETag`, and `PUT` with `If-Match` replaces it, refused with 412 when
//!   it changed. `token` is sent as a bearer token, and `cache` names a
//!   file that keeps the last answer, asked again with `If-None-Match`.
//!
//! An item's text as written is `null`, so `srl` shows its item object.

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
    let place = Place::of(config)?;
    let (mut data, version) = place.load()?;
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
            .map(|name| json!({"collection": name}))
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
                        json!({"collection": name, "handle": at(&item, "handle"),
                               "written": null, "item": at(&item, "item")})
                    })
            })
            .collect()),
        "write" => write(config, inputs, &mut data, &place, version.as_deref()),
        _ => Err(format!("`{call}` is not a call")),
    }
}

/// Makes the changes of `inputs` to `data`, read at `version`, and saves it,
/// unless one is refused, it is a dry run, or the settings say otherwise.
fn write(
    config: &Map<String, Value>,
    inputs: &[Map<String, Value>],
    data: &mut Value,
    place: &Place,
    version: Option<&str>,
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
        place.save(data, version)?;
    }
    Ok(answers)
}

/// Where the items are kept: a file, or a web service that keeps them as
/// one resource, `/data`.
enum Place {
    File(String),
    Service {
        /// `host:port`.
        address: String,
        token: Option<String>,
        /// A file that keeps the last answer and its entity tag, so that an
        /// unchanged resource is not sent again.
        cache: Option<String>,
    },
}

impl Place {
    /// The place the settings name: `url` (`http://host:port`), with
    /// `token` and `cache`, or else `data`.
    fn of(config: &Map<String, Value>) -> Result<Self, String> {
        let setting = |key: &str| config.get(key).and_then(Value::as_str).map(str::to_owned);
        if let Some(url) = setting("url") {
            let address = url
                .strip_prefix("http://")
                .ok_or_else(|| format!("`url` must start with http://, not {url}"))?;
            return Ok(Self::Service {
                address: address.trim_end_matches('/').to_owned(),
                token: setting("token"),
                cache: setting("cache"),
            });
        }
        setting("data").map(Self::File).ok_or_else(|| {
            "the setting `data` or `url` is missing; give where the items are kept".to_owned()
        })
    }

    /// The items, and the version they were read at, if the place has
    /// versions.
    fn load(&self) -> Result<(Value, Option<String>), String> {
        match self {
            Self::File(file) => match fs::read_to_string(file) {
                Ok(text) => Ok((
                    serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())?,
                    None,
                )),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    Ok((json!({"next": 1, "collections": {}}), None))
                }
                Err(error) => Err(format!("cannot read {file}: {error}")),
            },
            Self::Service {
                address,
                token,
                cache,
            } => {
                let cached = cache
                    .as_ref()
                    .and_then(|cache| fs::read_to_string(cache).ok())
                    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
                let etag = cached
                    .as_ref()
                    .and_then(|cached| cached.get("etag"))
                    .and_then(Value::as_str);
                let mut headers = authorization(token.as_deref());
                if let Some(etag) = etag {
                    headers.push(("If-None-Match".to_owned(), etag.to_owned()));
                }
                let (status, etag, body) = http(address, "GET", &headers, None)?;
                match status {
                    304 => {
                        let cached = cached.unwrap_or_default();
                        Ok((
                            at(&cached, "data").clone(),
                            at(&cached, "etag").as_str().map(str::to_owned),
                        ))
                    }
                    200 => {
                        let data = serde_json::from_str::<Value>(&body)
                            .map_err(|error| error.to_string())?;
                        if let (Some(cache), Some(etag)) = (cache, &etag) {
                            fs::write(cache, json!({"etag": etag, "data": data}).to_string())
                                .map_err(|error| {
                                    format!("cannot write the cache {cache}: {error}")
                                })?;
                        }
                        Ok((data, etag))
                    }
                    _ => Err(format!("the service answered {status}: {}", body.trim())),
                }
            }
        }
    }

    /// Saves `data`, read at `version`. The service refuses it if the
    /// items changed since.
    fn save(&self, data: &Value, version: Option<&str>) -> Result<(), String> {
        match self {
            Self::File(file) => fs::write(file, data.to_string())
                .map_err(|error| format!("cannot write {file}: {error}")),
            Self::Service { address, token, .. } => {
                let mut headers = authorization(token.as_deref());
                if let Some(version) = version {
                    headers.push(("If-Match".to_owned(), version.to_owned()));
                }
                let (status, _, body) = http(address, "PUT", &headers, Some(&data.to_string()))?;
                match status {
                    200 => Ok(()),
                    412 => Err("the items changed on the service since they were read".to_owned()),
                    _ => Err(format!("the service answered {status}: {}", body.trim())),
                }
            }
        }
    }
}

/// The header that carries `token`, if there is one.
fn authorization(token: Option<&str>) -> Vec<(String, String)> {
    token
        .map(|token| ("Authorization".to_owned(), format!("Bearer {token}")))
        .into_iter()
        .collect()
}

/// A request `method` to `/data` of the service at `address`, with
/// `headers` and `body`: the status, the entity tag and the body of the
/// answer.
fn http(
    address: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> Result<(u16, Option<String>, String), String> {
    let failed = |error: io::Error| format!("cannot reach the service at {address}: {error}");
    let mut stream = std::net::TcpStream::connect(address).map_err(failed)?;
    let body = body.unwrap_or_default();
    let mut request = format!(
        "{method} /data HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(&[name.as_str(), ": ", value.as_str(), "\r\n"].concat());
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).map_err(failed)?;
    let mut answer = String::new();
    io::Read::read_to_string(&mut stream, &mut answer).map_err(failed)?;
    let (head, body) = answer
        .split_once("\r\n\r\n")
        .ok_or("the service's answer has no end of headers")?;
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or("the service's answer has no status")?;
    let etag = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("etag"))
        .map(|(_, value)| value.trim().to_owned());
    Ok((status, etag, body.to_owned()))
}
