//! Kinds that are programs (see `docs/cards.md`, "Kind programs").
//!
//! A kind program answers four calls, `describe`, `validate`, `groups` and
//! `make`, each for a batch: it reads one JSON object per line on standard
//! input, up to its end, and prints one JSON object per line on standard
//! output, in the same order. A status other than 0 is a failure of the
//! whole call, and standard error says why.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Map, Value};

use super::{Cache, FieldDescription};
use crate::card::{Card, CardId};
use crate::note::is_field_key;
use crate::note::{ItemId, is_id_text};
use crate::{Config, Store};

/// How often a `make` call is checked for having finished, while it may
/// still run out of time.
const POLL: Duration = Duration::from_millis(10);

/// An item's card IDs, in groups of siblings.
type Groups = Vec<Vec<CardId>>;

/// The kinds that are programs, and where they run.
#[derive(Debug, Clone, Default)]
pub(crate) struct Programs {
    /// The store's folder: where programs run, and what relative program
    /// paths start from.
    root: PathBuf,
    /// Each kind's command: the program, then its arguments.
    commands: BTreeMap<String, Vec<String>>,
    card_timeout: Duration,
}

/// What `describe` prints.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DescribeOutput {
    description: String,
    fields: Vec<FieldDescription>,
}

/// What `validate` prints for an item: why it is invalid, nothing if it is
/// valid.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidateOutput {
    errors: Vec<ErrorOutput>,
}

/// One reason an item is invalid, about one of its fields, or about the
/// whole item when `field` is `null` or left out.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorOutput {
    #[serde(default)]
    field: Option<String>,
    message: String,
}

/// What `groups` prints for an item: its card suffixes in groups of
/// siblings, `null` for a single card with the item's ID.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupsOutput {
    groups: Vec<Vec<Option<String>>>,
}

/// What `make` prints for a card it made: the fields of a card, without its
/// ID. A field that may be `null` may also be left out.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MadeOutput {
    prompt: String,
    #[serde(default)]
    prompt_revealed: Option<String>,
    #[serde(default)]
    answer: Option<String>,
}

/// What `make` prints for a card it can't make.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MakeFailedOutput {
    error: String,
}

impl Programs {
    /// The programs named in `[kinds]` of the store's configuration.
    pub(crate) fn new(store: &Store, config: &Config) -> Self {
        Self {
            root: store.root().to_path_buf(),
            commands: config.kinds.clone(),
            card_timeout: Duration::from_secs(config.kind_programs.card_timeout),
        }
    }

    /// Whether `name` is a kind program.
    pub(crate) fn has(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    /// The names of the kind programs, sorted.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.commands.keys().map(String::as_str)
    }

    /// What the program of kind `name` says its kind is for, and its
    /// fields. Errors are plain reasons.
    pub(crate) fn describe(&self, name: &str) -> Result<(String, Vec<FieldDescription>), String> {
        let line = self
            .run(name, "describe", &lines(&[Value::Object(Map::new())]), None)?
            .pop()
            .expect("a call should give one line for its one input");
        description(&line).map_err(|reason| invalid_output(name, "describe", &reason))
    }

    /// Why the program of kind `name` says each item of `inputs` is
    /// invalid, in order: nothing for a valid item. An answer that doesn't
    /// follow the protocol is a reason too, about that item. Errors are
    /// plain reasons, and fail the whole call.
    pub(crate) fn validate(
        &self,
        name: &str,
        inputs: &[Value],
        cache: &mut Cache<'_>,
    ) -> Result<Vec<Vec<String>>, String> {
        let lines = self.answers(name, "validate", inputs, cache)?;
        Ok(lines
            .iter()
            .map(|line| {
                reasons(line)
                    .unwrap_or_else(|reason| vec![invalid_output(name, "validate", &reason)])
            })
            .collect())
    }

    /// The card IDs the program of kind `name` gives for each item of
    /// `items`, whose input is the one of `inputs` at the same place, in
    /// groups of siblings; or why its answer about the item doesn't follow
    /// the protocol. Errors are plain reasons, and fail the whole call.
    pub(crate) fn groups(
        &self,
        name: &str,
        items: &[&ItemId],
        inputs: &[Value],
        cache: &mut Cache<'_>,
    ) -> Result<Vec<Result<Groups, String>>, String> {
        let lines = self.answers(name, "groups", inputs, cache)?;
        Ok(items
            .iter()
            .zip(&lines)
            .map(|(item, line)| {
                groups(item, line).map_err(|reason| invalid_output(name, "groups", &reason))
            })
            .collect())
    }

    /// Each card of `cards` as the program of kind `name` makes it from the
    /// input of `inputs` at the same place, or why it can't. The call is
    /// stopped after `kind_programs.card_timeout` for each card. Errors are
    /// plain reasons, and fail the whole call.
    pub(crate) fn make(
        &self,
        name: &str,
        cards: &[&CardId],
        inputs: &[Value],
    ) -> Result<Vec<Result<Card, String>>, String> {
        let count = u32::try_from(inputs.len()).unwrap_or(u32::MAX);
        let lines = self.run(
            name,
            "make",
            &lines(inputs),
            Some(self.card_timeout * count),
        )?;
        Ok(cards
            .iter()
            .zip(&lines)
            .map(|(card, line)| match made(card, line) {
                Ok(Ok(made)) => Ok(made),
                Ok(Err(error)) => Err(format!(
                    "the program of kind {name} couldn't make it: {error}"
                )),
                Err(reason) => Err(invalid_output(name, "make", &reason)),
            })
            .collect())
    }

    /// The lines the program of kind `name` answers to the call `call` for
    /// `inputs`, one per input: those `cache` kept, and the others from one
    /// run of the program, if any is left, which `cache` then keeps.
    fn answers(
        &self,
        name: &str,
        call: &str,
        inputs: &[Value],
        cache: &mut Cache<'_>,
    ) -> Result<Vec<String>, String> {
        let items = inputs.iter().map(|input| {
            input
                .get("id")
                .and_then(Value::as_str)
                .expect("an item's input should hold its ID")
        });
        let lines = lines(inputs);
        let command = self.command(name);
        let kept = match cache {
            Cache::Use(index) => index.kind_answers(name, command, call, &lines),
            Cache::None | Cache::Refresh(_) => vec![None; lines.len()],
        };
        // The items without a kept answer: their IDs, for people reading
        // the cache, and their input lines.
        let (asked_items, asked): (Vec<_>, Vec<_>) = items
            .zip(&lines)
            .zip(&kept)
            .filter(|(_, kept)| kept.is_none())
            .map(|((item, line), _)| (item, line.clone()))
            .unzip();
        if asked.is_empty() {
            return Ok(kept.into_iter().flatten().collect());
        }
        let answered = self.run(name, call, &asked, None)?;
        if let Cache::Use(index) | Cache::Refresh(index) = cache {
            let answers = asked_items
                .into_iter()
                .zip(&asked)
                .zip(&answered)
                .map(|((item, line), answer)| (item, line, answer))
                .collect::<Vec<_>>();
            index.keep_kind_answers(name, command, call, &answers);
        }
        let mut answered = answered.into_iter();
        Ok(kept
            .into_iter()
            .map(|kept| {
                kept.unwrap_or_else(|| {
                    answered
                        .next()
                        .expect("a run should answer every input that had no kept answer")
                })
            })
            .collect())
    }

    /// The command of kind `name`: the program, then its arguments.
    fn command(&self, name: &str) -> &[String] {
        self.commands
            .get(name)
            .expect("a kind program should exist once its kind was read")
    }

    /// Runs the call `call` of the program of kind `name` with `inputs`, one
    /// per line, on its standard input, and gives the lines it printed, one
    /// per input. It is stopped after `timeout`, if there is one.
    fn run(
        &self,
        name: &str,
        call: &str,
        inputs: &[String],
        timeout: Option<Duration>,
    ) -> Result<Vec<String>, String> {
        let (program, args) = self
            .command(name)
            .split_first()
            .expect("a kind's command should have been checked to be non-empty");
        let program = self.program(name, program)?;
        let mut child = Command::new(&program)
            .args(args)
            .arg(call)
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                format!(
                    "cannot run `{}` for kind {name} ({error}); check `kinds.{name}` in \
                     .srl/config.toml",
                    program.display()
                )
            })?;
        let stdin = child
            .stdin
            .take()
            .expect("a program's stdin should be piped");
        let mut input = String::new();
        for line in inputs {
            input.push_str(line);
            input.push('\n');
        }
        let writer = thread::spawn(move || write_input(stdin, &input));
        let stdout = reader(
            child
                .stdout
                .take()
                .expect("a program's stdout should be piped"),
        );
        let stderr = reader(
            child
                .stderr
                .take()
                .expect("a program's stderr should be piped"),
        );
        let failed =
            |error: io::Error| format!("running the program of kind {name} failed: {error}");
        let Some(status) = wait(&mut child, timeout).map_err(failed)? else {
            let what = match inputs.len() {
                1 => "a card".to_owned(),
                count => format!("{count} cards"),
            };
            return Err(format!(
                "the program of kind {name} took longer than {} seconds to make {what}, so it \
                 was stopped; make it faster, or raise `kind_programs.card_timeout` in \
                 .srl/config.toml",
                timeout.unwrap_or_default().as_secs()
            ));
        };
        let joined = |handle: JoinHandle<io::Result<Vec<u8>>>| {
            handle
                .join()
                .expect("reading a program's output should not panic")
        };
        let stdout = joined(stdout).map_err(failed)?;
        let stderr = joined(stderr).map_err(failed)?;
        writer
            .join()
            .expect("writing a program's input should not panic")
            .map_err(failed)?;
        if !status.success() {
            let reason = String::from_utf8_lossy(&stderr);
            let reason = reason.trim();
            return Err(if reason.is_empty() {
                format!("the program of kind {name} failed ({status}) and gave no reason")
            } else {
                format!("the program of kind {name} failed: {reason}")
            });
        }
        let stdout = String::from_utf8(stdout)
            .map_err(|error| invalid_output(name, call, &format!("it is not UTF-8 ({error})")))?;
        let lines = stdout.lines().map(str::to_owned).collect::<Vec<_>>();
        if lines.len() != inputs.len() {
            return Err(invalid_output(
                name,
                call,
                &format!(
                    "it printed {} lines for {} input lines; it must print one line per input \
                     line, in order",
                    lines.len(),
                    inputs.len()
                ),
            ));
        }
        Ok(lines)
    }

    /// The file of `program`: a path with a folder is relative to the
    /// store's folder, unless it is absolute; a bare name is found as the
    /// platform finds programs.
    fn program(&self, name: &str, program: &str) -> Result<PathBuf, String> {
        let path = Path::new(program);
        if path.is_absolute() || path.components().count() > 1 {
            return Ok(self.root.join(path));
        }
        which::which(program).map_err(|error| {
            format!(
                "cannot find the program `{program}` of kind {name} ({error}); check `kinds.{name}` \
                 in .srl/config.toml"
            )
        })
    }
}

/// Each input as the line given to a program.
fn lines(inputs: &[Value]) -> Vec<String> {
    inputs.iter().map(Value::to_string).collect()
}

/// The reason for output that doesn't follow the protocol.
fn invalid_output(name: &str, call: &str, reason: &str) -> String {
    format!(
        "the program of kind {name} printed invalid output for `{call}`: {reason}; see \
         docs/cards.md, \"Kind programs\""
    )
}

/// The description and fields in what `describe` printed. Errors are plain
/// reasons.
fn description(output: &str) -> Result<(String, Vec<FieldDescription>), String> {
    let output =
        serde_json::from_str::<DescribeOutput>(output).map_err(|error| error.to_string())?;
    if output.description.trim().is_empty() {
        return Err("its description is empty".to_owned());
    }
    if output.fields.is_empty() {
        return Err("it has no fields".to_owned());
    }
    let mut seen = HashSet::new();
    for field in &output.fields {
        if !is_field_key(&field.name) {
            return Err(format!(
                "`{}` is not a field name; a field name is A-Z, 0-9 and _, starting with a letter",
                field.name
            ));
        }
        if !seen.insert(&field.name) {
            return Err(format!("the field {} is given twice", field.name));
        }
        if field.description.trim().is_empty() {
            return Err(format!(
                "the description of the field {} is empty",
                field.name
            ));
        }
    }
    Ok((output.description, output.fields))
}

/// The reasons in what `validate` printed for an item, each `FIELD:
/// message` when it is about a field. Errors are plain reasons.
fn reasons(output: &str) -> Result<Vec<String>, String> {
    let output =
        serde_json::from_str::<ValidateOutput>(output).map_err(|error| error.to_string())?;
    output
        .errors
        .into_iter()
        .map(|error| {
            if error.message.trim().is_empty() {
                return Err("an error has an empty message".to_owned());
            }
            match error.field {
                Some(field) if !is_field_key(&field) => Err(format!(
                    "`{field}` is not a field name; a field name is A-Z, 0-9 and _, starting \
                     with a letter"
                )),
                Some(field) => Ok(format!("{field}: {}", error.message)),
                None => Ok(error.message),
            }
        })
        .collect()
}

/// The card IDs of item `item` in what `groups` printed, in groups. Errors
/// are plain reasons.
fn groups(item: &ItemId, output: &str) -> Result<Vec<Vec<CardId>>, String> {
    let output = serde_json::from_str::<GroupsOutput>(output).map_err(|error| error.to_string())?;
    if output.groups.is_empty() {
        return Err("it has no group of cards".to_owned());
    }
    if output.groups.iter().any(Vec::is_empty) {
        return Err("it has a group without cards".to_owned());
    }
    let suffixes = output.groups.iter().flatten().collect::<Vec<_>>();
    if suffixes.len() > 1 && suffixes.contains(&&None) {
        return Err("`null` is only for an item's single card, not one of several".to_owned());
    }
    let mut seen = HashSet::new();
    for suffix in suffixes.into_iter().flatten() {
        if !is_id_text(suffix) {
            return Err(format!(
                "`{suffix}` is not a card suffix; a suffix is 1 to 32 characters a-z and 0-9"
            ));
        }
        if !seen.insert(suffix) {
            return Err(format!("the card suffix `{suffix}` is given twice"));
        }
    }
    Ok(output
        .groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|suffix| CardId::new(item.clone(), suffix))
                .collect()
        })
        .collect())
}

/// The card `card` in what `make` printed for it, or the error the program
/// gave for it. Errors are plain reasons.
fn made(card: &CardId, output: &str) -> Result<Result<Card, String>, String> {
    let value = serde_json::from_str::<Value>(output).map_err(|error| error.to_string())?;
    if value.get("error").is_some() {
        let failed =
            serde_json::from_value::<MakeFailedOutput>(value).map_err(|error| error.to_string())?;
        if failed.error.trim().is_empty() {
            return Err("its error is empty".to_owned());
        }
        return Ok(Err(failed.error));
    }
    let made = serde_json::from_value::<MadeOutput>(value).map_err(|error| error.to_string())?;
    Ok(Ok(Card {
        id: card.clone(),
        prompt: made.prompt,
        prompt_revealed: made.prompt_revealed,
        answer: made.answer,
    }))
}

/// Writes `input` to a program's standard input, and closes it.
fn write_input(mut stdin: ChildStdin, input: &str) -> io::Result<()> {
    match stdin.write_all(input.as_bytes()) {
        // A program may finish without reading its input; whether it
        // succeeded is up to its exit status.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}

/// Reads all of `pipe` on its own thread, so that a program never waits on
/// a full pipe.
fn reader(mut pipe: impl Read + Send + 'static) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

/// Waits for `child` to finish, for at most `timeout` if there is one.
/// `None` if it ran out of time, in which case it is stopped.
fn wait(child: &mut Child, timeout: Option<Duration>) -> io::Result<Option<ExitStatus>> {
    let Some(timeout) = timeout else {
        return child.wait().map(Some);
    };
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if start.elapsed() >= timeout {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use jiff::civil::date;
    use tempfile::TempDir;

    use super::*;
    use crate::{Error, Index, Log, Lookup, Memories, Notes, Selector};

    /// The test kind program of `crates/srl-test-kind`, which `cargo test
    /// --workspace` builds next to the test binaries.
    fn test_kind() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let folder = exe.parent().unwrap().parent().unwrap();
        let path = folder.join(format!("srl-test-kind{}", std::env::consts::EXE_SUFFIX));
        assert!(
            path.exists(),
            "{} isn't built; run `cargo test --workspace`",
            path.display()
        );
        path
    }

    /// A store with `config` and the notes `notes` (path and text), and its
    /// notes as read.
    fn store_with(config: &str, notes: &[(&str, &str)]) -> (TempDir, Store, Notes) {
        store_in(TempDir::new().unwrap(), config, notes)
    }

    /// A store with `config` and one note `a.md` holding `note`, and its
    /// notes as read.
    fn store(config: &str, note: &str) -> (TempDir, Store, Notes) {
        store_with(config, &[("a.md", note)])
    }

    /// [`store_with`], in `tmp`.
    fn store_in(tmp: TempDir, config: &str, notes: &[(&str, &str)]) -> (TempDir, Store, Notes) {
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        fs::write(store.config_file(), config).unwrap();
        for (path, text) in notes {
            fs::write(store.note_file(path), text).unwrap();
        }
        let notes = Notes::load(
            &store,
            &Config::load(&store).unwrap(),
            &mut Index::open(&store),
        )
        .unwrap();
        (tmp, store, notes)
    }

    /// A configuration where kind `fake` is the test kind program, with
    /// `more` after it.
    fn config(more: &str) -> String {
        format!("[kinds]\nfake = ['{}']\n{more}", test_kind().display())
    }

    fn ids(notes: &Notes) -> Vec<String> {
        notes.cards().map(|(_, card)| card.to_string()).collect()
    }

    fn problems(notes: &Notes) -> Vec<String> {
        notes.problems().iter().map(ToString::to_string).collect()
    }

    /// The cards `ids` as made on 2026-10-27, as JSON, or the error.
    fn make(store: &Store, notes: &Notes, ids: &[&str]) -> Result<Vec<Value>, String> {
        let wanted = ids
            .iter()
            .map(|id| notes.card(&CardId::parse(id).unwrap()).unwrap())
            .collect::<Vec<_>>();
        notes
            .make_cards(store, &wanted, date(2026, 10, 27))
            .map(|cards| {
                cards
                    .iter()
                    .map(|card| serde_json::to_value(card).unwrap())
                    .collect()
            })
            .map_err(|error| error.to_string())
    }

    #[test]
    fn a_kind_program_gives_the_cards_of_its_items_in_groups() {
        let (_tmp, _store, notes) = store(
            &config(""),
            "```srl id:abc kind:fake\nGROUPS: te past|negative\n```\n\n```srl id:abd kind:fake\nX: y\n```\n",
        );
        assert_eq!(problems(&notes), Vec::<String>::new());
        assert_eq!(ids(&notes), ["abc-negative", "abc-past", "abc-te", "abd"]);
        let item = notes.item(&ItemId::parse("abc").unwrap()).unwrap();
        let group = |suffix: &str| item.group(&CardId::parse(&format!("abc-{suffix}")).unwrap());
        assert_eq!(group("te"), group("past"));
        assert_ne!(group("te"), group("negative"));
        assert_eq!(item.kind().as_str(), "fake");
    }

    #[test]
    fn validate_and_groups_run_once_for_every_item_of_the_kind() {
        let log = "```srl id:{} kind:fake\nLOG: calls.log\n```\n";
        let (tmp, _store, notes) = store_with(
            &config(""),
            &[
                ("a.md", &log.replace("{}", "aaa")),
                (
                    "b.md",
                    &format!(
                        "{}\n```srl id:bbb kind:fake\nLOG: calls.log\nINVALID: no\n```\n",
                        log.replace("{}", "bab")
                    ),
                ),
            ],
        );
        assert_eq!(ids(&notes), ["aaa", "bab"]);
        assert_eq!(problems(&notes), ["b.md:5: no"]);
        // groups is asked only about the items validate accepted.
        let calls = fs::read_to_string(tmp.path().join("calls.log")).unwrap();
        assert_eq!(calls, "validate 3\ngroups 2\n");
    }

    /// The notes of `store` read again, asking kind programs about every
    /// item if `asking`, and the calls logged to `calls.log`, which is then
    /// emptied.
    fn reload(store: &Store, asking: bool) -> (Notes, String) {
        let config = Config::load(store).unwrap();
        let mut index = Index::open(store);
        let notes = if asking {
            Notes::load_asking(store, &config, &mut index)
        } else {
            Notes::load(store, &config, &mut index)
        }
        .unwrap();
        assert!(index.problems().is_empty());
        let log = store.root().join("calls.log");
        let calls = fs::read_to_string(&log).unwrap_or_default();
        fs::write(&log, "").unwrap();
        (notes, calls)
    }

    #[test]
    fn kept_answers_are_not_asked_for_again() {
        let item = |id: &str, more: &str| {
            format!("```srl id:{id} kind:fake\nLOG: calls.log\n{more}\n```\n")
        };
        let (_tmp, store, _) = store(
            &config(""),
            &[
                item("aaa", "GROUPS: x y"),
                item("bbb", ""),
                item("ccc", "INVALID: no"),
            ]
            .join("\n"),
        );
        fs::write(store.root().join("calls.log"), "").unwrap();
        let (notes, calls) = reload(&store, false);
        assert_eq!(calls, "");
        assert_eq!(ids(&notes), ["aaa-x", "aaa-y", "bbb"]);
        // A kept answer that says the item is invalid says it again.
        assert_eq!(problems(&notes), ["a.md:11: no"]);
        // Only the item that changed is asked about.
        let note = fs::read_to_string(store.note_file("a.md")).unwrap();
        fs::write(store.note_file("a.md"), note.replace("x y", "x z")).unwrap();
        let (notes, calls) = reload(&store, false);
        assert_eq!(calls, "validate 1\ngroups 1\n");
        assert_eq!(ids(&notes), ["aaa-x", "aaa-z", "bbb"]);
        // Asking asks about every item, and keeps the answers.
        let (_, calls) = reload(&store, true);
        assert_eq!(calls, "validate 3\ngroups 2\n");
        let (_, calls) = reload(&store, false);
        assert_eq!(calls, "");
    }

    #[test]
    fn answers_are_kept_for_the_kinds_name_and_command_as_written() {
        let (_tmp, store, _) = store(
            &config(""),
            "```srl id:aaa kind:fake\nLOG: calls.log\n```\n",
        );
        fs::write(store.root().join("calls.log"), "").unwrap();
        let (_, calls) = reload(&store, false);
        assert_eq!(calls, "");
        // Another command is another program, even running the same file.
        fs::write(
            store.config_file(),
            format!("[kinds]\nfake = ['{}', 'more']\n", test_kind().display()),
        )
        .unwrap();
        let (_, calls) = reload(&store, false);
        assert_eq!(calls, "validate 1\ngroups 1\n");
        // Another name is another kind, even with the same command.
        fs::write(
            store.config_file(),
            format!("[kinds]\nother = ['{}', 'more']\n", test_kind().display()),
        )
        .unwrap();
        let note = fs::read_to_string(store.note_file("a.md")).unwrap();
        fs::write(
            store.note_file("a.md"),
            note.replace("kind:fake", "kind:other"),
        )
        .unwrap();
        let (_, calls) = reload(&store, false);
        assert_eq!(calls, "validate 1\ngroups 1\n");
    }

    #[test]
    fn answers_of_a_call_that_failed_are_not_kept() {
        let (_tmp, store, notes) = store(
            &config(""),
            "```srl id:aaa kind:fake\nLOG: calls.log\n```\n\n```srl id:bbb kind:fake\nBREAK_VALIDATE: down\n```\n",
        );
        assert!(ids(&notes).is_empty());
        let note = fs::read_to_string(store.note_file("a.md")).unwrap();
        fs::write(store.note_file("a.md"), note.replace("BREAK_VALIDATE", "X")).unwrap();
        fs::write(store.root().join("calls.log"), "").unwrap();
        let (notes, calls) = reload(&store, false);
        assert_eq!(calls, "validate 2\ngroups 2\n");
        assert_eq!(ids(&notes), ["aaa", "bbb"]);
    }

    #[test]
    fn without_a_usable_cache_every_item_is_asked_about() {
        let (_tmp, store, _) = store(
            &config(""),
            "```srl id:aaa kind:fake\nLOG: calls.log\n```\n",
        );
        fs::remove_dir_all(store.cache_dir()).unwrap();
        fs::write(store.cache_dir(), "not a folder").unwrap();
        fs::write(store.root().join("calls.log"), "").unwrap();
        let mut index = Index::open(&store);
        let notes = Notes::load(&store, &Config::load(&store).unwrap(), &mut index).unwrap();
        assert_eq!(ids(&notes), ["aaa"]);
        assert!(matches!(
            index.problems()[..],
            [Error::CacheUnusable { .. }]
        ));
        let calls = fs::read_to_string(store.root().join("calls.log")).unwrap();
        assert_eq!(calls, "validate 1\ngroups 1\n");
    }

    #[test]
    fn validate_errors_are_errors_in_the_note_naming_their_field() {
        let (_tmp, _store, notes) = store(
            &config(""),
            "```srl id:abc kind:fake\nERRORS: {\"errors\": [{\"field\": \"VERB\", \"message\": \"is missing\"}, {\"field\": null, \"message\": \"also wrong\"}]}\n```\n\n```srl id:abd kind:fake\nERRORS: not json\n```\n",
        );
        assert!(ids(&notes).is_empty());
        let reasons = problems(&notes);
        assert_eq!(
            reasons[..2],
            ["a.md:1: VERB: is missing", "a.md:1: also wrong"]
        );
        assert!(
            reasons[2].starts_with(
                "a.md:5: the program of kind fake printed invalid output for `validate`"
            ),
            "{reasons:?}"
        );
    }

    #[test]
    fn validate_output_must_follow_the_protocol() {
        for bad in [
            "{}",
            r#"{"errors": [{"message": " "}]}"#,
            r#"{"errors": [{"field": "verb", "message": "x"}]}"#,
            r#"{"errors": [], "more": 1}"#,
            r#"{"errors": [{"message": "x", "more": 1}]}"#,
            "",
        ] {
            assert!(reasons(bad).is_err(), "accepted {bad}");
        }
        assert_eq!(reasons(r#"{"errors": []}"#).unwrap(), Vec::<String>::new());
        assert_eq!(
            reasons(r#"{"errors": [{"message": "x"}, {"field": "A_1", "message": "y"}]}"#).unwrap(),
            ["x", "A_1: y"]
        );
    }

    #[test]
    fn invalid_groups_for_an_item_are_an_error_in_the_note() {
        let (_tmp, _store, notes) = store(
            &config(""),
            "```srl id:abc kind:fake\nGROUPS_RAW: {\"groups\": []}\n```\n\n```srl id:abd kind:fake\nX: y\n```\n",
        );
        assert_eq!(ids(&notes), ["abd"]);
        let reasons = problems(&notes);
        assert!(
            reasons[0].starts_with("a.md:1: the program of kind fake printed invalid output for `groups`: it has no group"),
            "{reasons:?}"
        );
    }

    #[test]
    fn groups_output_must_follow_the_protocol() {
        let item = ItemId::parse("abc").unwrap();
        for bad in [
            r#"{"groups": []}"#,
            r#"{"groups": [[]]}"#,
            r#"{"groups": [[null, "a"]]}"#,
            r#"{"groups": [[null], [null]]}"#,
            r#"{"groups": [["A"]]}"#,
            r#"{"groups": [["a"], ["a"]]}"#,
            r#"{"groups": [["a"]], "more": 1}"#,
            r#"{"groups": [{"cards": ["a"]}]}"#,
            "",
        ] {
            assert!(groups(&item, bad).is_err(), "accepted {bad}");
        }
        assert_eq!(
            groups(&item, r#"{"groups": [[null]]}"#).unwrap(),
            [[CardId::new(item.clone(), None)]]
        );
    }

    #[test]
    fn problems_come_by_note_and_line_and_failing_programs_last() {
        let (_tmp, _store, notes) = store_with(
            &config("broken = ['srl-no-such-program-anywhere']\n"),
            &[
                (
                    "a.md",
                    "```srl id:aaa kind:fake\nINVALID: in a\n```\n\n```srl id:aab kind:broken\nX: y\n```\n\n```srl\nQ: no id\nA: a\n```\n",
                ),
                ("b.md", "```srl id:bbb kind:fake\nINVALID: in b\n```\n"),
            ],
        );
        let problems = problems(&notes);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert_eq!(problems[0], "a.md:1: in a");
        assert!(
            problems[1].starts_with("a.md:9: this item has no ID"),
            "{problems:?}"
        );
        assert_eq!(problems[2], "b.md:1: in b");
        assert!(
            problems[3].contains("cannot find the program `srl-no-such-program-anywhere`"),
            "{problems:?}"
        );
    }

    #[test]
    fn a_failing_call_is_a_problem_of_the_program_not_of_its_items() {
        for field in [
            "BREAK_VALIDATE: on purpose",
            "BREAK_GROUPS: on purpose",
            "SHORT: yes",
        ] {
            let (_tmp, _store, notes) = store(
                &config(""),
                &format!(
                    "```srl id:abc kind:fake\n{field}\n```\n\n```srl id:abd kind:fake\nX: y\n```\n\n```srl id:qqq\nQ: q\nA: a\n```\n"
                ),
            );
            // Items of other kinds are read as always.
            assert_eq!(ids(&notes), ["qqq"], "{field}");
            let reasons = problems(&notes);
            assert_eq!(reasons.len(), 1, "{field}: {reasons:?}");
            assert!(
                reasons[0].starts_with("the program of kind fake "),
                "{reasons:?}"
            );
            assert!(
                reasons[0].contains("no item of kind fake can be read"),
                "{reasons:?}"
            );
        }
        let (_tmp, _store, notes) =
            store(&config(""), "```srl id:abc kind:fake\nSHORT: yes\n```\n");
        assert!(
            problems(&notes)[0].contains("it printed 0 lines for 1 input lines"),
            "{:?}",
            problems(&notes)
        );
    }

    #[test]
    fn describe_output_must_follow_the_protocol() {
        for bad in [
            r#"{"description": "", "fields": [{"name": "X", "description": "x"}]}"#,
            r#"{"description": "d", "fields": []}"#,
            r#"{"description": "d", "fields": [{"name": "x", "description": "x"}]}"#,
            r#"{"description": "d", "fields": [{"name": "X", "description": " "}]}"#,
            r#"{"description": "d", "fields": [{"name": "X", "description": "x"}, {"name": "X", "description": "y"}]}"#,
            r#"{"description": "d", "fields": [{"name": "X", "description": "x", "required": true}]}"#,
            r#"{"description": "d", "fields": [{"name": "X", "description": "x"}], "more": 1}"#,
            "",
        ] {
            assert!(description(bad).is_err(), "accepted {bad}");
        }
        let (text, fields) = description(
            r#"{"description": "d", "fields": [{"name": "VERB_1", "description": "x"}]}"#,
        )
        .unwrap();
        assert_eq!(text, "d");
        assert_eq!(fields[0].name, "VERB_1");
    }

    #[test]
    fn kinds_are_qa_then_the_programs_by_name() {
        let (_tmp, store, _notes) = store(
            &format!(
                "[kinds]\nzz = ['{0}']\nfake = ['{0}']\n",
                test_kind().display()
            ),
            "",
        );
        let described = crate::kinds(&store, &Config::load(&store).unwrap()).unwrap();
        let names = described
            .iter()
            .map(|kind| kind.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["qa", "fake", "zz"]);
        assert_eq!(described[0].fields[1].name, "A");
        assert_eq!(described[1].description, "A kind for srl's tests.");
        assert_eq!(described[1].fields[0].name, "GROUPS");
    }

    #[test]
    fn a_failing_or_invalid_describe_call_fails() {
        let kinds = |argument: &str| {
            let (_tmp, store, _notes) = store(
                &format!(
                    "[kinds]\nfake = ['{}', '{argument}']\n",
                    test_kind().display()
                ),
                "",
            );
            crate::kinds(&store, &Config::load(&store).unwrap())
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            kinds("fail"),
            "cannot describe the kind fake: the program of kind fake failed: describe fails on \
             purpose"
        );
        assert!(kinds("{}").contains("printed invalid output for `describe`"));
    }

    #[test]
    fn a_card_gets_its_suffix_context_and_day_and_runs_in_the_store() {
        let (tmp, store, notes) = store(
            &config(""),
            "# Verbs\n\n```srl id:abc kind:fake\nGROUPS: te\n```\n",
        );
        let folder = tmp.path().file_name().unwrap().to_string_lossy();
        assert_eq!(
            make(&store, &notes, &["abc-te"]).unwrap(),
            [serde_json::json!({
                "id": "abc-te",
                "prompt": "te?",
                "prompt_revealed": "te!",
                "answer": format!("day \"2026-10-27\"; context \"# Verbs\\n\"; in {folder}"),
            })]
        );
    }

    #[test]
    fn the_cards_of_a_kind_are_made_in_one_call_in_order() {
        let (tmp, store, notes) = store(
            &config(""),
            "```srl id:abc kind:fake\nGROUPS: a b\nLOG: calls.log\n```\n\n```srl id:qqq\nQ: q\nA: a\n```\n",
        );
        let made = make(&store, &notes, &["abc-b", "qqq", "abc-a"]).unwrap();
        let ids = made
            .iter()
            .map(|card| card["id"].clone())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["abc-b", "qqq", "abc-a"]);
        let calls = fs::read_to_string(tmp.path().join("calls.log")).unwrap();
        assert_eq!(calls, "validate 1\ngroups 1\nmake 2\n");
    }

    #[test]
    fn a_card_may_leave_out_its_revealed_prompt_and_answer() {
        let (_tmp, store, notes) = store(
            &config(""),
            "```srl id:abc kind:fake\nMAKE_RAW: {\"prompt\": \"p\"}\n```\n\n```srl id:abd kind:fake\nMAKE_RAW: {\"prompt\": \"p\", \"prompt_revealed\": null, \"answer\": null}\n```\n",
        );
        for id in ["abc", "abd"] {
            assert_eq!(
                make(&store, &notes, &[id]).unwrap(),
                [
                    serde_json::json!({"id": id, "prompt": "p", "prompt_revealed": null, "answer": null})
                ]
            );
        }
    }

    #[test]
    fn a_card_the_program_cant_make_slow_or_invalid_is_an_error() {
        let (_tmp, store, notes) = store(
            &config("[kind_programs]\ncard_timeout = 1\n"),
            "```srl\n@ id:aaa kind:fake\nFAIL_MAKE: _\n\n@ id:aab kind:fake\nSLEEP_MAKE: 5\n\n@ id:aac kind:fake\nMAKE_RAW: {\"prompt_revealed\": \"p\", \"answer\": \"a\"}\n\n@ id:aad kind:fake\nBREAK_MAKE: on purpose\n\n@ id:aae kind:fake\nMAKE_RAW: {\"error\": \" \"}\n```\n",
        );
        let made = |id: &str| make(&store, &notes, &[id]).unwrap_err();
        assert_eq!(
            made("aaa"),
            "cannot make the card aaa (fix what failed, or set the card aside with `srl suspend \
             aaa`): the program of kind fake couldn't make it: card _ fails on purpose"
        );
        assert!(made("aab").contains("took longer than 1 seconds to make a card"));
        assert!(made("aac").contains("printed invalid output for `make`"));
        assert!(made("aad").contains("the program of kind fake failed: on purpose"));
        assert!(made("aae").contains("its error is empty"));
    }

    #[test]
    fn the_time_limit_grows_with_the_cards_made_at_once() {
        let (_tmp, store, notes) = store(
            &config("[kind_programs]\ncard_timeout = 1\n"),
            "```srl id:abc kind:fake\nGROUPS: a b c\nSLEEP_MAKE: 2\n```\n",
        );
        // Three cards may take three seconds: two seconds is in time.
        assert_eq!(
            make(&store, &notes, &["abc-a", "abc-b", "abc-c"])
                .unwrap()
                .len(),
            3
        );
        assert!(
            make(&store, &notes, &["abc-a"])
                .unwrap_err()
                .contains("took longer than 1 seconds to make a card")
        );
    }

    #[test]
    fn a_relative_program_path_starts_at_the_store() {
        let tmp = TempDir::new().unwrap();
        let name = format!("srl-test-kind{}", std::env::consts::EXE_SUFFIX);
        fs::create_dir(tmp.path().join("kinds")).unwrap();
        fs::copy(test_kind(), tmp.path().join("kinds").join(&name)).unwrap();
        let (_tmp, _store, notes) = store_in(
            tmp,
            &format!("[kinds]\nfake = ['kinds/{name}']\n"),
            &[("a.md", "```srl id:abc kind:fake\nX: y\n```\n")],
        );
        assert_eq!(problems(&notes), Vec::<String>::new());
        assert_eq!(ids(&notes), ["abc"]);
    }

    #[test]
    fn a_kind_that_is_neither_qa_nor_configured_is_unknown() {
        let (_tmp, _store, notes) = store(&config(""), "```srl id:abc kind:conj\nX: y\n```\n");
        assert!(problems(&notes)[0].contains("there is no kind `conj`"));
        let (_tmp, _store, notes) = store(
            "[kinds]\nfake = ['srl-no-such-program-anywhere']\n",
            "```srl id:abc kind:fake\nX: y\n```\n",
        );
        assert!(
            problems(&notes)[0].contains("cannot find the program `srl-no-such-program-anywhere`")
        );
    }

    #[test]
    fn a_card_that_fails_fails_next_without_trying_another() {
        let (_tmp, store, notes) = store(
            &config(""),
            "```srl id:abc kind:fake\nGROUPS: a b\nFAIL_MAKE: a\n```\n",
        );
        let log = Log {
            entries: Vec::new(),
            problems: Vec::new(),
        };
        let config = Config::default();
        let day = date(2026, 10, 27);
        let memories = Memories::replay(&log, &config, day).unwrap();
        let next = crate::next(
            &store,
            &notes,
            &memories,
            &Selector::parse("", &Config::default()).unwrap(),
            &config,
            day,
            0,
        );
        assert!(matches!(next, Err(Error::CardFailed { card, .. }) if card == "abc-a"));
    }
}
