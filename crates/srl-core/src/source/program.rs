//! A source program: a program named in `[sources]` that keeps a source's
//! items its own way, and answers the calls of the source port over
//! standard input and output (see `docs/sources.md`, "Source programs").
//!
//! `srl` trusts it no more than a kind program: every line it prints is
//! read strictly, and its changes are verified by reading it again.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::{Map, Value};

use crate::config::globs;
use crate::domain::context::ContextMode;
use crate::domain::note::object::{self, Object};
use crate::process;
use crate::source::{
    Change, Collection, ContextRequest, Description, Finding, Problem, Read, ReadItem, Refusal,
    Source,
};
use crate::{Error, Result, Store};

/// A source program, as `[sources]` names it.
#[derive(Debug)]
pub(super) struct Program {
    /// The source's name, for messages.
    name: String,
    root: PathBuf,
    command: Vec<String>,
    settings: Map<String, Value>,
}

impl Program {
    pub(super) fn new(
        store: &Store,
        name: &str,
        command: &[String],
        settings: &Map<String, Value>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            root: store.root().to_path_buf(),
            command: command.to_vec(),
            settings: settings.clone(),
        }
    }

    /// The call `call`, with `inputs`, each given the source's settings as
    /// `config`: every line the program printed, each a JSON object.
    fn call(&self, call: &str, inputs: Vec<Map<String, Value>>) -> Result<Vec<Map<String, Value>>> {
        let failed = |reason: String| Error::SourceFailed {
            name: self.name.clone(),
            reason,
        };
        let (program, args) = self
            .command
            .split_first()
            .expect("a source program's command should have been checked to be non-empty");
        let file = process::find(&self.root, program)
            .map_err(|error| failed(format!("cannot find its program `{program}` ({error})")))?;
        let input = inputs
            .into_iter()
            .map(|mut input| {
                input.insert("config".to_owned(), Value::Object(self.settings.clone()));
                Value::Object(input).to_string() + "\n"
            })
            .collect::<String>();
        let stdout = process::run(&self.root, &file, args, call, input, None)
            .map_err(|failure| failed(failure.reason(&format!("its call `{call}`"))))?;
        stdout
            .lines()
            .map(|line| match serde_json::from_str::<Value>(line) {
                Ok(Value::Object(object)) => Ok(object),
                Ok(_) => Err(self.invalid(call, &format!("`{line}` is not a JSON object"))),
                Err(error) => Err(self.invalid(call, &format!("`{line}` is not JSON ({error})"))),
            })
            .collect()
    }

    /// [`Program::call`], for a call that answers each input line with one
    /// line.
    fn call_each(
        &self,
        call: &str,
        inputs: Vec<Map<String, Value>>,
    ) -> Result<Vec<Map<String, Value>>> {
        let count = inputs.len();
        let answers = self.call(call, inputs)?;
        if answers.len() == count {
            Ok(answers)
        } else {
            Err(self.invalid(
                call,
                &format!(
                    "it printed {} lines for {count} input lines; it must print one line per \
                     input line, in order",
                    answers.len()
                ),
            ))
        }
    }

    /// The error for output of `call` that doesn't follow the protocol.
    fn invalid(&self, call: &str, reason: &str) -> Error {
        Error::SourceFailed {
            name: self.name.clone(),
            reason: format!("its call `{call}` printed invalid output: {reason}"),
        }
    }

    /// The items the program has now, as verification compares them: each
    /// collection with its item, in a fixed order.
    fn snapshot(&self) -> Result<Vec<(String, String)>> {
        let read = self.read()?;
        let mut items = read
            .items
            .into_iter()
            .map(|item| (item.collection, as_line(&item.item).to_string()))
            .collect::<Vec<_>>();
        items.sort();
        Ok(items)
    }
}

impl Source for Program {
    fn collections(&self) -> Result<Vec<Collection>> {
        let call = "collections";
        self.call(call, vec![Map::new()])?
            .into_iter()
            .map(|line| {
                let reason = |reason: String| self.invalid(call, &reason);
                only(&line, &["collection", "file"]).map_err(reason)?;
                Ok(Collection {
                    name: collection(&line).map_err(reason)?,
                    file: optional_text(&line, "file").map_err(reason)?,
                })
            })
            .collect()
    }

    fn read(&self) -> Result<Read> {
        let call = "read";
        let mut read = Read::default();
        let mut handles = HashSet::new();
        for line in self.call(call, vec![Map::new()])? {
            let reason = |reason: String| self.invalid(call, &reason);
            if line.contains_key("error") {
                only(&line, &["collection", "line", "error"]).map_err(reason)?;
                read.problems.push(Problem {
                    collection: collection(&line).map_err(reason)?,
                    line: line_number(&line).map_err(reason)?,
                    reason: text(&line, "error").map_err(reason)?,
                });
                continue;
            }
            only(&line, &["collection", "handle", "line", "written", "item"]).map_err(reason)?;
            let handle = text(&line, "handle").map_err(reason)?;
            if !handles.insert(handle.clone()) {
                return Err(reason(format!("the handle `{handle}` is given twice")));
            }
            let item = line
                .get("item")
                .ok_or_else(|| reason("a line has no `item`".to_owned()))?;
            let item = object::read(&item.to_string())
                .map_err(|why| reason(format!("an `item` is not an item object: {why}")))?;
            read.items.push(ReadItem {
                collection: collection(&line).map_err(reason)?,
                handle,
                line: line_number(&line).map_err(reason)?,
                written: optional_text(&line, "written").map_err(reason)?,
                item,
            });
        }
        Ok(read)
    }

    fn context(&self, requests: &[ContextRequest<'_>]) -> Result<Vec<Option<String>>> {
        let call = "context";
        let inputs = requests
            .iter()
            .map(|request| {
                let mode = match request.mode {
                    ContextMode::Prose => "prose",
                    ContextMode::Full => "full",
                    ContextMode::None => "none",
                };
                Map::from_iter([
                    ("collection".to_owned(), Value::from(request.collection)),
                    ("mode".to_owned(), Value::from(mode)),
                    ("own".to_owned(), Value::Bool(request.own)),
                ])
            })
            .collect();
        self.call_each(call, inputs)?
            .into_iter()
            .map(|line| {
                let reason = |reason: String| self.invalid(call, &reason);
                only(&line, &["context"]).map_err(reason)?;
                optional_text(&line, "context").map_err(reason)
            })
            .collect()
    }

    /// A program says which collections it creates in `describe`.
    fn refuses(&self, collection: &str) -> Option<String> {
        let described = match self.describe() {
            Ok(described) => described,
            Err(error) => return Some(error.to_string()),
        };
        let creates = match globs(&described.include) {
            Ok(creates) => creates,
            Err(reason) => return Some(self.invalid("describe", &reason).to_string()),
        };
        (!creates.is_match(collection)).then(|| {
            format!(
                "the source {} creates no collection named {collection}; `srl sources` says \
                 which it creates",
                self.name
            )
        })
    }

    /// Every change goes to the program in one call. The program is read
    /// before and after it, and must have made exactly the changes asked
    /// for.
    fn write(&self, changes: &[Change], dry_run: bool) -> std::result::Result<String, Refusal> {
        let call = "write";
        let before = if dry_run {
            Vec::new()
        } else {
            self.snapshot()?
        };
        let inputs = changes
            .iter()
            .map(|change| {
                let mut input = Map::new();
                input.insert("dry_run".to_owned(), Value::Bool(dry_run));
                match change {
                    Change::Add { collection, item } => {
                        input.insert("op".to_owned(), Value::from("add"));
                        input.insert("collection".to_owned(), Value::from(collection.as_str()));
                        input.insert("item".to_owned(), as_line(item));
                    }
                    Change::Replace {
                        collection,
                        handle,
                        before,
                        after,
                    } => {
                        input.insert("op".to_owned(), Value::from("replace"));
                        input.insert("collection".to_owned(), Value::from(collection.as_str()));
                        input.insert("handle".to_owned(), Value::from(handle.as_str()));
                        input.insert("before".to_owned(), as_line(before));
                        input.insert("after".to_owned(), as_line(after));
                    }
                }
                input
            })
            .collect();
        let mut previews = String::new();
        for (at, line) in self.call_each(call, inputs)?.into_iter().enumerate() {
            let reason = |reason: String| self.invalid(call, &reason);
            if line.contains_key("error") {
                only(&line, &["error"]).map_err(reason)?;
                return Err(Refusal::Change {
                    at,
                    reason: text(&line, "error").map_err(reason)?,
                });
            }
            if line.contains_key("preview") {
                only(&line, &["preview"]).map_err(reason)?;
                let preview = text(&line, "preview").map_err(reason)?;
                previews.push_str(preview.trim_end_matches('\n'));
                previews.push('\n');
                continue;
            }
            only(&line, &["ok"]).map_err(reason)?;
            if line.get("ok") != Some(&Value::Bool(true)) {
                return Err(
                    reason("a line is neither `ok`, `preview` nor `error`".to_owned()).into(),
                );
            }
        }
        if !dry_run {
            let mut expected = before;
            for change in changes {
                match change {
                    Change::Add { collection, item } => {
                        expected.push((collection.clone(), as_line(item).to_string()));
                    }
                    Change::Replace {
                        collection,
                        before,
                        after,
                        ..
                    } => {
                        let old = (collection.clone(), as_line(before).to_string());
                        if let Some(slot) = expected.iter_mut().find(|item| **item == old) {
                            *slot = (collection.clone(), as_line(after).to_string());
                        }
                    }
                }
            }
            expected.sort();
            if self.snapshot()? != expected {
                return Err(Error::SourceFailed {
                    name: self.name.clone(),
                    reason: "its call `write` didn't make exactly the changes it was given, as \
                             reading it again shows"
                        .to_owned(),
                }
                .into());
            }
        }
        Ok(previews)
    }

    /// A program reports what it can as problems of `read`.
    fn lint(&self) -> Result<Vec<Finding>> {
        Ok(Vec::new())
    }

    fn describe(&self) -> Result<Description> {
        let call = "describe";
        let answer = self
            .call_each(call, vec![Map::new()])?
            .pop()
            .unwrap_or_default();
        let reason = |reason: String| self.invalid(call, &reason);
        only(&answer, &["description", "creates"]).map_err(reason)?;
        let description = text(&answer, "description").map_err(reason)?;
        if description.trim().is_empty() {
            return Err(reason("`description` is blank".to_owned()));
        }
        let creates = match answer.get("creates") {
            Some(Value::Array(globs)) => globs
                .iter()
                .map(|glob| {
                    glob.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| reason(format!("`creates` holds {glob}, not text")))
                })
                .collect::<Result<Vec<_>>>()?,
            _ => return Err(reason("`creates` must be a list of globs".to_owned())),
        };
        globs(&creates).map_err(|why| reason(format!("`creates`: {why}")))?;
        Ok(Description {
            summary: description,
            include: creates,
            exclude: Vec::new(),
        })
    }
}

/// An item as the program is given it: its item object, as `srl` writes
/// one.
fn as_line(item: &Object) -> Value {
    Value::Object(object::written(&item.entries, &item.fields))
}

/// Fails if `line` has a key not in `keys`. Errors are plain reasons.
fn only(line: &Map<String, Value>, keys: &[&str]) -> std::result::Result<(), String> {
    match line.keys().find(|key| !keys.contains(&key.as_str())) {
        Some(key) => Err(format!(
            "a line has the key `{key}`, which is not one of {keys:?}"
        )),
        None => Ok(()),
    }
}

/// The text of `key`, which must be there.
fn text(line: &Map<String, Value>, key: &str) -> std::result::Result<String, String> {
    match line.get(key) {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(other) => Err(format!("`{key}` must be text, not {other}")),
        None => Err(format!("a line has no `{key}`")),
    }
}

/// The text of `key`, or `None` when it is `null` or left out.
fn optional_text(
    line: &Map<String, Value>,
    key: &str,
) -> std::result::Result<Option<String>, String> {
    match line.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => text(line, key).map(Some),
    }
}

/// The line of a line, from 1, or `None` when it is `null` or left out.
fn line_number(line: &Map<String, Value>) -> std::result::Result<Option<usize>, String> {
    match line.get("line") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .filter(|number| *number >= 1)
            .map(Some)
            .ok_or_else(|| format!("`line` must be a line number from 1, not {number}")),
        Some(other) => Err(format!("`line` must be a number, not {other}")),
    }
}

/// The collection a line names: parts joined by `/`, none of them empty,
/// `.`, `..` or starting with `.`.
fn collection(line: &Map<String, Value>) -> std::result::Result<String, String> {
    let name = text(line, "collection")?;
    let valid = !name.contains('\\')
        && !name.contains(':')
        && name
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.'));
    if valid {
        Ok(name)
    } else {
        Err(format!(
            "`{name}` is not a collection's name: parts joined by /, none of them empty or \
             starting with ."
        ))
    }
}
