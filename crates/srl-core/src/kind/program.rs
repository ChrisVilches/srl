//! Kinds that are programs (see `docs/cards.md`, "Kind programs").
//!
//! A kind program answers two calls, `ids` and `card`. Each reads one JSON
//! object on standard input and prints one on standard output; a status
//! other than 0 is a failure, and standard error says why.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::card::{Card, CardId};
use crate::note::{ItemId, is_id_text};
use crate::{Config, Store};

/// How often a `card` call is checked for having finished, while it may
/// still run out of time.
const POLL: Duration = Duration::from_millis(10);

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

/// What `ids` prints (see `docs/cards.md`, "Kind programs").
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdsOutput {
    groups: Vec<GroupOutput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupOutput {
    cards: Vec<Option<String>>,
}

/// What `card` prints: the fields of a card, without its ID. A field that
/// may be `null` may also be left out.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardOutput {
    prompt: String,
    #[serde(default)]
    prompt_revealed: Option<String>,
    #[serde(default)]
    answer: Option<String>,
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

    /// The card IDs the program of kind `name` gives for the item `input`,
    /// whose ID is `item`, in groups of siblings. Errors are plain reasons.
    pub(crate) fn groups(
        &self,
        name: &str,
        item: &ItemId,
        input: &Value,
    ) -> Result<Vec<Vec<CardId>>, String> {
        let output = self.run(name, "ids", input, None)?;
        groups(item, &output).map_err(|reason| invalid_output(name, "ids", &reason))
    }

    /// The card `card` as the program of kind `name` makes it from `input`.
    /// Errors are plain reasons.
    pub(crate) fn card(&self, name: &str, card: &CardId, input: &Value) -> Result<Card, String> {
        let output = self.run(name, "card", input, Some(self.card_timeout))?;
        let made = serde_json::from_str::<CardOutput>(&output)
            .map_err(|error| invalid_output(name, "card", &error.to_string()))?;
        Ok(Card {
            id: card.clone(),
            prompt: made.prompt,
            prompt_revealed: made.prompt_revealed,
            answer: made.answer,
        })
    }

    /// Runs the call `call` of the program of kind `name` with `input` on
    /// its standard input, and gives what it printed. It is stopped after
    /// `timeout`, if there is one.
    fn run(
        &self,
        name: &str,
        call: &str,
        input: &Value,
        timeout: Option<Duration>,
    ) -> Result<String, String> {
        let command = self
            .commands
            .get(name)
            .expect("a kind program should exist once its kind was read");
        let (program, args) = command
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
        let input = input.to_string();
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
            return Err(format!(
                "the program of kind {name} took longer than {} seconds to make a card, so it \
                 was stopped; make it faster, or raise `kind_programs.card_timeout` in \
                 .srl/config.toml",
                self.card_timeout.as_secs()
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
        String::from_utf8(stdout)
            .map_err(|error| invalid_output(name, call, &format!("it is not UTF-8 ({error})")))
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

/// The reason for output that doesn't follow the protocol.
fn invalid_output(name: &str, call: &str, reason: &str) -> String {
    format!(
        "the program of kind {name} printed invalid output for `{call}`: {reason}; see \
         docs/cards.md, \"Kind programs\""
    )
}

/// The card IDs of item `item` in what `ids` printed, in groups. Errors are
/// plain reasons.
fn groups(item: &ItemId, output: &str) -> Result<Vec<Vec<CardId>>, String> {
    let output = serde_json::from_str::<IdsOutput>(output).map_err(|error| error.to_string())?;
    if output.groups.is_empty() {
        return Err("it has no group of cards".to_owned());
    }
    if output.groups.iter().any(|group| group.cards.is_empty()) {
        return Err("it has a group without cards".to_owned());
    }
    let suffixes = output
        .groups
        .iter()
        .flat_map(|group| &group.cards)
        .collect::<Vec<_>>();
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
                .cards
                .into_iter()
                .map(|suffix| CardId::new(item.clone(), suffix))
                .collect()
        })
        .collect())
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
    use crate::{Error, Log, Lookup, Memories, Notes, Selector};

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

    /// A store with `config` and one note `a.md` holding `note`, and its
    /// notes as read.
    fn store(config: &str, note: &str) -> (TempDir, Store, Notes) {
        store_in(TempDir::new().unwrap(), config, note)
    }

    /// [`store`], in `tmp`.
    fn store_in(tmp: TempDir, config: &str, note: &str) -> (TempDir, Store, Notes) {
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        fs::write(store.config_file(), config).unwrap();
        fs::write(store.note_file("a.md"), note).unwrap();
        let notes = Notes::load(&store, &Config::load(&store).unwrap()).unwrap();
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

    fn reasons(notes: &Notes) -> Vec<String> {
        notes.problems().iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_kind_program_gives_the_cards_of_its_items_in_groups() {
        let (_tmp, _store, notes) = store(
            &config(""),
            "<!-- srl id:abc kind:fake -->\nGROUPS: te past|negative\n\n<!-- srl id:abd kind:fake -->\nX: y\n",
        );
        assert_eq!(reasons(&notes), Vec::<String>::new());
        assert_eq!(ids(&notes), ["abc-negative", "abc-past", "abc-te", "abd"]);
        let item = notes.item(&ItemId::parse("abc").unwrap()).unwrap();
        let group = |suffix: &str| item.group(&CardId::parse(&format!("abc-{suffix}")).unwrap());
        assert_eq!(group("te"), group("past"));
        assert_ne!(group("te"), group("negative"));
        assert_eq!(item.kind().as_str(), "fake");
    }

    #[test]
    fn a_failing_or_invalid_ids_call_is_an_error_in_the_note() {
        let (_tmp, _store, notes) = store(
            &config(""),
            "<!-- srl id:abc kind:fake -->\nFAIL_IDS: VERB is missing\n\n<!-- srl id:abd kind:fake -->\nIDS: not json\n",
        );
        assert!(ids(&notes).is_empty());
        let reasons = reasons(&notes);
        assert_eq!(
            reasons[0],
            "a.md:1: the program of kind fake failed: VERB is missing"
        );
        assert!(
            reasons[1]
                .starts_with("a.md:4: the program of kind fake printed invalid output for `ids`"),
            "{reasons:?}"
        );
    }

    #[test]
    fn ids_output_must_follow_the_protocol() {
        let item = ItemId::parse("abc").unwrap();
        for bad in [
            r#"{"groups": []}"#,
            r#"{"groups": [{"cards": []}]}"#,
            r#"{"groups": [{"cards": [null, "a"]}]}"#,
            r#"{"groups": [{"cards": [null]}, {"cards": [null]}]}"#,
            r#"{"groups": [{"cards": ["A"]}]}"#,
            r#"{"groups": [{"cards": ["a"]}, {"cards": ["a"]}]}"#,
            r#"{"groups": [{"cards": ["a"]}], "more": 1}"#,
            r#"{"groups": [{"cards": ["a"], "name": "x"}]}"#,
            "",
        ] {
            assert!(groups(&item, bad).is_err(), "accepted {bad}");
        }
        assert_eq!(
            groups(&item, r#"{"groups": [{"cards": [null]}]}"#).unwrap(),
            [[CardId::new(item.clone(), None)]]
        );
    }

    #[test]
    fn a_card_gets_its_suffix_context_and_day_and_runs_in_the_store() {
        let (tmp, store, notes) = store(
            &config(""),
            "# Verbs\n\n<!-- srl id:abc kind:fake -->\nGROUPS: te\n",
        );
        let (item, card) = notes.card(&CardId::parse("abc-te").unwrap()).unwrap();
        let made = notes
            .make_card(&store, item, card, date(2026, 10, 27))
            .unwrap();
        let folder = tmp.path().file_name().unwrap().to_string_lossy();
        assert_eq!(
            serde_json::to_value(&made).unwrap(),
            serde_json::json!({
                "id": "abc-te",
                "prompt": "te?",
                "prompt_revealed": "te!",
                "answer": format!("day \"2026-10-27\"; context \"# Verbs\\n\"; in {folder}"),
            })
        );
    }

    #[test]
    fn a_card_may_leave_out_its_revealed_prompt_and_answer() {
        let (_tmp, store, notes) = store(
            &config(""),
            "<!-- srl id:abc kind:fake -->\nCARD: {\"prompt\": \"p\"}\n\n<!-- srl id:abd kind:fake -->\nCARD: {\"prompt\": \"p\", \"prompt_revealed\": null, \"answer\": null}\n",
        );
        for id in ["abc", "abd"] {
            let (item, card) = notes.card(&CardId::parse(id).unwrap()).unwrap();
            let made = notes
                .make_card(&store, item, card, date(2026, 10, 27))
                .unwrap();
            assert_eq!(
                serde_json::to_value(&made).unwrap(),
                serde_json::json!({"id": id, "prompt": "p", "prompt_revealed": null, "answer": null})
            );
        }
    }

    #[test]
    fn a_failing_slow_or_invalid_card_call_is_an_error() {
        let (_tmp, store, notes) = store(
            &config("[kind_programs]\ncard_timeout = 1\n"),
            "<!-- srl id:aaa kind:fake -->\nFAIL_CARD: _\n\n<!-- srl id:aab kind:fake -->\nSLEEP_CARD: 5\n\n<!-- srl id:aac kind:fake -->\nCARD: {\"prompt_revealed\": \"p\", \"answer\": \"a\"}\n",
        );
        let made = |id: &str| {
            let (item, card) = notes.card(&CardId::parse(id).unwrap()).unwrap();
            notes
                .make_card(&store, item, card, date(2026, 10, 27))
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            made("aaa"),
            "cannot make the card aaa: the program of kind fake failed: card _ fails on purpose"
        );
        assert!(made("aab").contains("took longer than 1 seconds"));
        assert!(made("aac").contains("printed invalid output for `card`"));
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
            "<!-- srl id:abc kind:fake -->\nX: y\n",
        );
        assert_eq!(reasons(&notes), Vec::<String>::new());
        assert_eq!(ids(&notes), ["abc"]);
    }

    #[test]
    fn a_kind_that_is_neither_qa_nor_configured_is_unknown() {
        let (_tmp, _store, notes) = store(&config(""), "<!-- srl id:abc kind:conj -->\nX: y\n");
        assert!(reasons(&notes)[0].contains("there is no kind `conj`"));
        let (_tmp, _store, notes) = store(
            "[kinds]\nfake = ['srl-no-such-program-anywhere']\n",
            "<!-- srl id:abc kind:fake -->\nX: y\n",
        );
        assert!(
            reasons(&notes)[0].contains("cannot find the program `srl-no-such-program-anywhere`")
        );
    }

    #[test]
    fn a_card_that_fails_is_skipped_by_next() {
        let (_tmp, store, notes) = store(
            &config(""),
            "<!-- srl id:abc kind:fake -->\nGROUPS: a b\nFAIL_CARD: a\n",
        );
        let log = Log {
            entries: Vec::new(),
            problems: Vec::new(),
        };
        let config = Config::default();
        let day = date(2026, 10, 27);
        let memories = Memories::replay(&log, &config, day).unwrap();
        let (next, failed) = crate::next(
            &store,
            &notes,
            &memories,
            &Selector::parse("").unwrap(),
            &config,
            day,
        );
        assert_eq!(next.unwrap().1.id().to_string(), "abc-b");
        assert!(matches!(&failed[..], [Error::CardFailed { card, .. }] if card == "abc-a"));
    }
}
