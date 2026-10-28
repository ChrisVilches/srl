//! Runs the real `srl` binary and checks exit codes and output streams.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

/// The `srl` binary, unaffected by the `SRL_` variables of whoever runs the
/// tests.
fn srl_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_srl"));
    command
        .env_remove("SRL_DIR")
        .env_remove("SRL_NOW")
        .env_remove("SRL_SEED");
    command
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn srl(args: &[&str]) -> Output {
    srl_command().args(args).output().unwrap()
}

/// Runs `srl` with `input` on standard input.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn srl_with_input(args: &[&str], input: &str) -> Output {
    let mut child = srl_command()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn json_lines(output: &Output) -> Vec<serde_json::Value> {
    stdout(output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn lines(output: &Output) -> Vec<String> {
    stdout(output).lines().map(str::to_owned).collect()
}

#[test]
fn help_succeeds_and_lists_commands() {
    let output = srl(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout(&output);
    assert!(stdout.contains("next"));
    assert!(stdout.contains("review"));
}

#[test]
fn usage_errors_exit_with_2() {
    let output = srl(&["review", "a7f3k2", "--rating", "9"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn unknown_commands_exit_with_2_and_explain() {
    let output = srl(&["no-such-command-for-srl-tests"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("no-such-command-for-srl-tests"));
    assert!(output.stdout.is_empty());
}

/// A temporary directory with a fresh store in it, and `notes` written.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn store(notes: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    assert_eq!(srl(&["init", dir(&tmp)]).status.code(), Some(0));
    for (path, text) in notes {
        write(&tmp, path, text);
    }
    tmp
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn write(tmp: &TempDir, path: &str, text: &str) {
    let file = tmp.path().join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn read(tmp: &TempDir, path: &str) -> String {
    fs::read_to_string(tmp.path().join(path)).unwrap()
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn dir(tmp: &TempDir) -> &str {
    tmp.path().to_str().unwrap()
}

const GRAMMAR: &str = "# 〜をものともせず

Undeterred by (adversity). Admiring tone.

```srl id:x7k2m9
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.
```

```srl id:k3m9q1
Q: {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。
A: Despite everyone's opposition, she started a company.
```
";

const FISH: &str = "```srl
@ id:tuna01
Q: 鮪
A: まぐろ — tuna

@ id:saba01 draft
Q: 鯖
A: さば — mackerel
```
";

fn japanese() -> TempDir {
    store(&[("japanese/grammar/wo.md", GRAMMAR), ("kanji/fish.md", FISH)])
}

#[test]
fn commands_refuse_a_directory_that_is_not_a_store() {
    let tmp = TempDir::new().unwrap();
    let output = srl(&["find", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("srl init"));
}

#[test]
fn init_prints_what_it_creates_and_refuses_to_run_twice() {
    let tmp = TempDir::new().unwrap();
    write(&tmp, "notes.md", "Already here.\n");
    let output = srl(&["init", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout(&output);
    assert!(stdout.contains("config.toml"), "{stdout}");
    assert!(stdout.contains("log"), "{stdout}");
    assert!(tmp.path().join(".srl").join("config.toml").is_file());
    assert_eq!(read(&tmp, "notes.md"), "Already here.\n");
    assert_eq!(srl(&["init", dir(&tmp)]).status.code(), Some(2));
}

#[test]
fn init_creates_the_store_given_with_dir() {
    let tmp = TempDir::new().unwrap();
    let output = srl(&["init", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(tmp.path().join(".srl").join("config.toml").is_file());
    let before = srl(&["--dir", dir(&tmp), "init"]);
    assert_eq!(before.status.code(), Some(2), "{}", stderr(&before));
}

#[test]
fn init_dry_run_creates_nothing() {
    let tmp = TempDir::new().unwrap();
    let output = srl(&["init", "--dry-run", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0));
    assert!(!tmp.path().join(".srl").exists());
}

#[test]
fn find_lists_cards_in_written_order() {
    let tmp = japanese();
    let output = srl(&["find", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        lines(&output),
        [
            "x7k2m9  japanese/grammar/wo.md:5  What does 〜をものともせず mean?",
            "k3m9q1-1  japanese/grammar/wo.md:10  {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。",
            "k3m9q1-2  japanese/grammar/wo.md:10  {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。",
            "tuna01  kanji/fish.md:2  鮪",
            "saba01  kanji/fish.md:6  鯖",
        ]
    );
}

#[test]
fn find_takes_in_and_negated_terms_and_exits_1_when_nothing_matches() {
    let tmp = japanese();
    let d = dir(&tmp);
    let output = srl(&["find", "in:japanese", "-id:k3m9q1", "--dir", d]);
    assert_eq!(lines(&output).len(), 1);
    assert!(lines(&output)[0].starts_with("x7k2m9"));
    let output = srl(&["find", "in:nothing", "--dir", d]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn find_json_prints_one_card_per_line() {
    let tmp = japanese();
    let output = srl(&["find", "id:k3m9q1-2,saba01", "--json", "--dir", dir(&tmp)]);
    assert_eq!(
        json_lines(&output),
        [
            serde_json::json!({"id": "k3m9q1-2", "item": "k3m9q1", "path": "japanese/grammar/wo.md",
                "line": 10, "status": "active", "state": "new", "due": null}),
            serde_json::json!({"id": "saba01", "item": "saba01", "path": "kanji/fish.md",
                "line": 6, "status": "draft", "state": "new", "due": null}),
        ]
    );
}

#[test]
fn bad_selectors_exit_with_2_and_explain() {
    let tmp = japanese();
    let output = srl(&["find", "tag:x", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("in, path"), "{}", stderr(&output));
}

#[test]
fn errors_in_notes_stop_commands_until_fixed() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(
        &tmp,
        "broken.md",
        "```srl id:bad1 darft\nQ: a\n```\n```srl\nQ: no id\nA: x\n```\n",
    );
    for args in [
        &["find"][..],
        &["next"],
        &["stats"],
        &["review", "x7k2m9", "--rating", "3"],
    ] {
        let output = srl(&[args, &["--dir", d]].concat());
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = stderr(&output);
        assert!(
            stderr.contains("broken.md:1: `darft` is not a header entry"),
            "{stderr}"
        );
        assert!(
            stderr.contains("broken.md:4: this item has no ID"),
            "{stderr}"
        );
    }
    let check = srl(&["check", "--dir", d]);
    assert_eq!(check.status.code(), Some(2));
    assert!(stderr(&check).contains("broken.md:1:"));
    // srl id refuses while another problem remains.
    let before = read(&tmp, "broken.md");
    let refused = srl(&["id", "--dir", d]);
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(read(&tmp, "broken.md"), before);
    write(&tmp, "broken.md", "```srl\nQ: no id\nA: x\n```\n");
    let assigned = srl(&["id", "--dir", d]);
    assert_eq!(assigned.status.code(), Some(0), "{}", stderr(&assigned));
    let line = stdout(&assigned);
    let id = line.split_whitespace().next().unwrap();
    assert!(line.trim_end().ends_with("broken.md:1"), "{line}");
    assert_eq!(
        read(&tmp, "broken.md"),
        format!("```srl id:{id}\nQ: no id\nA: x\n```\n")
    );
    assert_eq!(srl(&["id", "--dir", d]).status.code(), Some(1));
    assert_eq!(srl(&["check", "--dir", d]).status.code(), Some(0));
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(0));
}

#[test]
fn show_prints_the_item_and_its_context() {
    let tmp = japanese();
    let d = dir(&tmp);
    let output = srl(&["show", "k3m9q1-2", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "japanese/grammar/wo.md:10\n```srl id:k3m9q1\nQ: {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。\nA: Despite everyone's opposition, she started a company.\n```\n---\n# 〜をものともせず\n\nUndeterred by (adversity). Admiring tone.\n"
    );
    let full = srl(&["show", "x7k2m9", "--context", "full", "--json", "--dir", d]);
    let shown = &json_lines(&full)[0];
    assert_eq!(shown["kind"], "qa");
    assert_eq!(shown["Q"], "What does 〜をものともせず mean?");
    assert_eq!(shown["cards"], serde_json::json!(["x7k2m9"]));
    assert!(shown["context"].as_str().unwrap().contains("Q: {{c1::"));
    let none = srl(&["show", "tuna01", "--json", "--dir", d]);
    assert_eq!(json_lines(&none)[0]["context"], serde_json::Value::Null);
}

#[test]
fn show_reports_unknown_ids_and_fails() {
    let tmp = japanese();
    let output = srl(&["show", "x7k2m9", "zzz999", "x7k2m9-1", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).contains("x7k2m9"));
    let stderr = stderr(&output);
    assert!(stderr.contains("zzz999"), "{stderr}");
    assert!(stderr.contains("x7k2m9-1"), "{stderr}");
}

#[test]
fn cards_prints_the_cards_of_items_and_cards() {
    let tmp = japanese();
    let output = srl(&["cards", "k3m9q1", "--dir", dir(&tmp)]);
    assert_eq!(
        stdout(&output),
        "k3m9q1-1\n==[...]==を==[...]==、彼女は起業した。\n---\n==周囲の反対==を==ものともせず==、彼女は起業した。\n---\nDespite everyone's opposition, she started a company.\n\nk3m9q1-2\n周囲の反対をものともせず、==[who]==は起業した。\n---\n周囲の反対をものともせず、==彼女==は起業した。\n---\nDespite everyone's opposition, she started a company.\n"
    );
    let one = srl(&["cards", "k3m9q1-2", "--json", "--dir", dir(&tmp)]);
    let cards = json_lines(&one);
    assert_eq!(cards.len(), 1);
    assert_eq!(
        cards[0],
        serde_json::json!({
            "id": "k3m9q1-2",
            "prompt": "周囲の反対をものともせず、==[who]==は起業した。",
            "prompt_revealed": "周囲の反対をものともせず、==彼女==は起業した。",
            "answer": "Despite everyone's opposition, she started a company.",
        })
    );
}

#[test]
fn add_appends_items_with_their_header_entries() {
    let tmp = japanese();
    let d = dir(&tmp);
    let input = "{\"Q\": \"Use 〜をものともせず\\nabout football.\", \"A\": \"…\", \"see\": \"japanese/grammar/wo.md\", \"draft\": true}\n{\"Q\": \"{{c1::嵐}}をものともせず\", \"draft\": true}\n";
    let args = ["add", "generated/wo.md", "--stdin", "--dir", d];
    let dry = srl_with_input(&[&args[..], &["--dry-run"]].concat(), input);
    assert_eq!(dry.status.code(), Some(0), "{}", stderr(&dry));
    assert!(stdout(&dry).contains(
        "see:japanese/grammar/wo.md draft\nQ: Use 〜をものともせず\n  about football.\n"
    ));
    assert_eq!(stdout(&dry).matches("see:").count(), 1);
    assert!(!tmp.path().join("generated").exists());
    let added = srl_with_input(&args, input);
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let ids = lines(&added);
    assert_eq!(ids.len(), 2);
    let drafts = srl(&["find", "in:generated", "status:draft", "--dir", d]);
    assert_eq!(lines(&drafts).len(), 2);
    let context = srl(&["show", &ids[0], "--json", "--dir", d]);
    assert!(
        json_lines(&context)[0]["context"]
            .as_str()
            .unwrap()
            .contains("Admiring tone")
    );
}

#[test]
fn one_bad_line_adds_nothing() {
    let tmp = japanese();
    let output = srl_with_input(
        &["add", "a.md", "--stdin", "--dir", dir(&tmp)],
        "{\"Q\": \"q\", \"A\": \"a\"}\n{\"Q\": \"no answer\"}\n",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("line 2"), "{}", stderr(&output));
    assert!(!tmp.path().join("a.md").exists());
}

#[test]
fn drafts_move_through_accept_and_cards_through_suspend() {
    let tmp = japanese();
    let d = dir(&tmp);
    assert!(srl(&["next", "id:saba01", "--dir", d]).stdout.is_empty());
    let accepted = srl(&["accept", "in:fish", "--dir", d]);
    assert_eq!(lines(&accepted), ["saba01"]);
    assert!(read(&tmp, "kanji/fish.md").contains("\n@ id:saba01\n"));
    assert_eq!(
        srl(&["accept", "in:fish", "--dir", d]).status.code(),
        Some(1)
    );
    let suspended = srl(&["suspend", "id:k3m9q1-2,tuna01", "--dir", d]);
    assert_eq!(lines(&suspended), ["k3m9q1-2", "tuna01"]);
    let found = srl(&["find", "status:suspended", "--dir", d]);
    assert_eq!(lines(&found).len(), 2);
    let back = srl(&["unsuspend", "--dir", d]);
    assert_eq!(lines(&back), ["k3m9q1-2", "tuna01"]);
    // Suspending never changes a note.
    assert!(!read(&tmp, "kanji/fish.md").contains("suspend"));
}

#[test]
fn edit_stdin_replaces_the_items_in_place() {
    let tmp = japanese();
    let d = dir(&tmp);
    let output = srl_with_input(
        &["edit", "--stdin", "--dir", d],
        "{\"id\": \"x7k2m9\", \"Q\": \"What does 〜をものともせず mean?\", \"A\": \"Braving.\"}\n",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(lines(&output), ["x7k2m9"]);
    let text = read(&tmp, "japanese/grammar/wo.md");
    assert!(
        text.contains("```srl id:x7k2m9\nQ: What does 〜をものともせず mean?\nA: Braving.\n```\n\n```srl id:k3m9q1\n"),
        "{text}"
    );
    // What `srl show --json` prints goes back as it is, and an item that
    // starts with an `@` line keeps its place in its block.
    let shown = stdout(&srl(&["show", "tuna01", "--json", "--dir", d]));
    let changed = shown.replace("まぐろ — tuna", "まぐろ");
    let output = srl_with_input(&["edit", "--stdin", "--dir", d], &changed);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        read(&tmp, "kanji/fish.md"),
        FISH.replace("A: まぐろ — tuna", "A: まぐろ")
    );
    let wrong = srl_with_input(
        &["edit", "--stdin", "--dir", d],
        "{\"id\": \"other\", \"Q\": \"q\", \"A\": \"a\"}\n",
    );
    assert_eq!(wrong.status.code(), Some(2));
    assert!(stderr(&wrong).contains("line 1"), "{}", stderr(&wrong));
}

#[test]
fn jsonl_notes_hold_one_item_per_line_and_no_context() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(
        &tmp,
        ".srl/config.toml",
        "[notes]\ninclude = [\"**/*.md\", \"**/*.jsonl\"]\n",
    );
    let items = "{\"Q\": \"鯛\", \"A\": \"たい — sea bream\"}\n{\"id\": \"wo0001\", \"see\": \"japanese/grammar/wo.md\", \"Q\": \"{{c1::嵐}}をものともせず\"}\n";
    write(&tmp, "generated/items.jsonl", items);
    let ids = srl(&["id", "--dir", d]);
    assert_eq!(ids.status.code(), Some(0), "{}", stderr(&ids));
    assert!(stdout(&ids).contains("generated/items.jsonl:1"));
    let found = srl(&["find", "in:generated", "--dir", d]);
    let found = lines(&found);
    assert_eq!(found.len(), 2);
    assert!(found[1].starts_with("wo0001-1  generated/items.jsonl:2  "));
    let id = found[0].split_whitespace().next().unwrap().to_owned();
    // Without `see`, an item of a JSONL note has no context.
    let own = srl(&["show", &id, "--json", "--context", "full", "--dir", d]);
    assert_eq!(json_lines(&own)[0]["context"], serde_json::Value::Null);
    let seen = srl(&["show", "wo0001", "--json", "--dir", d]);
    assert!(
        json_lines(&seen)[0]["context"]
            .as_str()
            .unwrap()
            .contains("Admiring tone")
    );
    // A `see` naming a JSONL note gives it as it is.
    let input =
        "{\"Q\": \"Which fish?\", \"A\": \"Sea bream.\", \"see\": \"generated/items.jsonl\"}\n";
    let added = srl_with_input(&["add", "fish.md", "--stdin", "--dir", d], input);
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let shown = srl(&["show", &lines(&added)[0], "--json", "--dir", d]);
    assert_eq!(
        json_lines(&shown)[0]["context"],
        read(&tmp, "generated/items.jsonl")
    );
    // Without --json, the item is printed as its line.
    let plain = srl(&["show", "wo0001", "--context", "none", "--dir", d]);
    assert_eq!(
        stdout(&plain),
        format!(
            "generated/items.jsonl:2\n{}\n",
            items.lines().nth(1).unwrap()
        )
    );
    // An invalid line stops every command that reads items.
    write(&tmp, "generated/items.jsonl", "{\"Q\": \"q\", \"B\": 1}\n");
    let broken = srl(&["find", "--dir", d]);
    assert_eq!(broken.status.code(), Some(2));
    assert!(stderr(&broken).contains("generated/items.jsonl:1:"));
}

fn next_card(d: &str, extra: &[&str]) -> serde_json::Value {
    let output = srl(&[&["next", "--json", "--dir", d][..], extra].concat());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    json_lines(&output).remove(0)
}

fn record(d: &str, card: &serde_json::Value, rating: u8) {
    let request = serde_json::json!({"card": card["id"], "rating": rating});
    let output = srl_with_input(&["review", "--stdin", "--dir", d], &request.to_string());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn the_study_loop_records_reviews_and_holds_back_siblings() {
    let tmp = japanese();
    let d = dir(&tmp);
    let first = next_card(d, &[]);
    assert_eq!(first["id"], "x7k2m9");
    assert_eq!(first["prompt"], "What does 〜をものともせず mean?");
    record(d, &first, 3);
    let second = next_card(d, &[]);
    assert_eq!(second["id"], "k3m9q1-1");
    record(d, &second, 1);
    // k3m9q1-2 waits for tomorrow: its sibling was reviewed today.
    let third = next_card(d, &[]);
    assert_eq!(third["id"], "tuna01");
    record(d, &third, 4);
    let none = srl(&["next", "--dir", d]);
    assert_eq!(none.status.code(), Some(1));
    let log = json_lines(&srl(&["log", "--json", "--dir", d]));
    assert_eq!(log.len(), 3);
    assert_eq!(log[1]["card"], "k3m9q1-1");
    assert_eq!(log[1]["hash"].as_str().unwrap().len(), 12);
    let human = srl(&["log", "id:k3m9q1", "--dir", d]);
    assert!(
        stdout(&human).contains("  k3m9q1-1  review  1  human"),
        "{}",
        stdout(&human)
    );
}

#[test]
fn next_json_gives_the_card_with_its_state_note_and_context() {
    let tmp = japanese();
    let d = dir(&tmp);
    let first = next_card(d, &[]);
    assert_eq!(
        first,
        serde_json::json!({
            "id": "x7k2m9",
            "prompt": "What does 〜をものともせず mean?",
            "prompt_revealed": null,
            "answer": "Undeterred by (adversity), braving.",
            "state": "new",
            "kind": "qa",
            "path": "japanese/grammar/wo.md",
            "see": null,
            "context": "# 〜をものともせず\n\nUndeterred by (adversity). Admiring tone.\n",
        })
    );
    assert_eq!(
        next_card(d, &["--context", "none"])["context"],
        serde_json::Value::Null
    );
    assert!(
        next_card(d, &["--context", "full"])["context"]
            .as_str()
            .unwrap()
            .contains("Q: What does")
    );
}

#[test]
fn the_new_card_limit_holds_back_new_cards() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, ".srl/config.toml", "[schedule]\nnew_per_day = 1\n");
    record(d, &next_card(d, &[]), 3);
    assert_eq!(srl(&["next", "--dir", d]).status.code(), Some(1));
}

#[test]
fn reviews_by_id_need_cards_and_dry_runs_write_nothing() {
    let tmp = japanese();
    let d = dir(&tmp);
    let item_only = srl(&["review", "k3m9q1", "--rating", "3", "--dir", d]);
    assert_eq!(item_only.status.code(), Some(2));
    assert!(
        stderr(&item_only).contains("one card"),
        "{}",
        stderr(&item_only)
    );
    let dry = srl(&[
        "review",
        "k3m9q1-1",
        "tuna01",
        "--rating",
        "2",
        "--by",
        "agent",
        "--dry-run",
        "--dir",
        d,
    ]);
    assert_eq!(dry.status.code(), Some(0), "{}", stderr(&dry));
    let events = json_lines(&dry);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["by"], "agent");
    assert_eq!(srl(&["log", "--dir", d]).status.code(), Some(1));
}

#[test]
fn bad_reviews_record_nothing() {
    let tmp = japanese();
    let d = dir(&tmp);
    for input in [
        "{\"card\": \"x7k2m9\", \"rating\": 5}",
        "{\"card\": \"zzz999\", \"rating\": 3}",
        "{\"card\": \"x7k2m9\", \"rating\": 3, \"extra\": 1}",
        "not json",
    ] {
        let output = srl_with_input(&["review", "--stdin", "--dir", d], input);
        assert_eq!(output.status.code(), Some(2), "{input}");
    }
    assert_eq!(srl(&["log", "--dir", d]).status.code(), Some(1));
}

#[test]
fn a_broken_log_line_is_reported_and_skipped() {
    let tmp = japanese();
    let d = dir(&tmp);
    record(d, &next_card(d, &[]), 3);
    let log = tmp.path().join(".srl").join("log");
    let month = fs::read_dir(&log)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .unwrap();
    let text = fs::read_to_string(&month).unwrap();
    fs::write(&month, format!("{text}not an event\n")).unwrap();
    let output = srl(&["find", "state:young", "--dir", d]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stderr(&output).contains("skipped line 2"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn stats_counts_cards_reviews_and_retention() {
    let tmp = japanese();
    let d = dir(&tmp);
    record(d, &next_card(d, &[]), 3);
    let output = srl(&["stats", "--dir", d]);
    assert_eq!(
        stdout(&output),
        "cards  5\nnew  4\ndue  0\nyoung  1\nmature  0\nleech  0\nreviews today  1\nretention  -\n"
    );
    let json = json_lines(&srl(&["stats", "in:fish", "--json", "--dir", d]));
    assert_eq!(json[0]["cards"], 2);
}

#[test]
fn orphans_are_reported_and_their_history_moves() {
    let tmp = japanese();
    let d = dir(&tmp);
    record(d, &next_card(d, &[]), 3);
    // The question gets its first cloze deletion: x7k2m9 becomes x7k2m9-1.
    let text = read(&tmp, "japanese/grammar/wo.md").replace(
        "Q: What does 〜をものともせず mean?",
        "Q: What does 〜を{{c1::ものともせず}} mean?",
    );
    write(&tmp, "japanese/grammar/wo.md", &text);
    let check = srl(&["check", "--dir", d]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
    assert!(
        stderr(&check).contains("x7k2m9 has reviews but no card; x7k2m9-1 is new. To keep the history: srl move x7k2m9 x7k2m9-1"),
        "{}",
        stderr(&check)
    );
    let moved = srl(&["move", "x7k2m9", "x7k2m9-1", "--dir", d]);
    assert_eq!(moved.status.code(), Some(0), "{}", stderr(&moved));
    assert!(srl(&["check", "--dir", d]).stderr.is_empty());
    let young = srl(&["find", "state:young", "--dir", d]);
    assert!(
        stdout(&young).starts_with("x7k2m9-1  "),
        "{}",
        stdout(&young)
    );
    assert_eq!(
        srl(&["move", "x7k2m9", "x7k2m9-1", "--dir", d])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn a_move_is_suggested_only_for_an_item_with_one_orphan() {
    let tmp = japanese();
    let d = dir(&tmp);
    let reviewed = srl(&[
        "review", "k3m9q1-1", "k3m9q1-2", "--rating", "3", "--dir", d,
    ]);
    assert_eq!(reviewed.status.code(), Some(0), "{}", stderr(&reviewed));
    // Both clozes become c3: two orphans, and one new card k3m9q1-3.
    let text = read(&tmp, "japanese/grammar/wo.md")
        .replace("{{c1::", "{{c3::")
        .replace("{{c2::", "{{c3::");
    write(&tmp, "japanese/grammar/wo.md", &text);
    let check = srl(&["check", "--dir", d]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
    assert_eq!(stderr(&check).lines().count(), 2, "{}", stderr(&check));
    assert!(
        !stderr(&check).contains("srl move k3m9q1"),
        "{}",
        stderr(&check)
    );
}

#[test]
fn lint_finds_questions_without_headers() {
    let tmp = japanese();
    let d = dir(&tmp);
    assert_eq!(srl(&["lint", "--dir", d]).status.code(), Some(1));
    write(
        &tmp,
        "inbox.md",
        "Notes.\n\nQ: forgot the header\nA: here\n",
    );
    let output = srl(&["lint", "--dir", d]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stdout(&output).starts_with("inbox.md:3: "),
        "{}",
        stdout(&output)
    );
    assert_eq!(lines(&output).len(), 1);
}

#[test]
fn excluded_notes_and_dot_folders_are_not_read() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "templates/t.md", "```srl\nbroken\n```\n");
    write(&tmp, ".obsidian/x.md", "```srl\nbroken\n```\n");
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(2));
    write(
        &tmp,
        ".srl/config.toml",
        "[notes]\nexclude = [\"templates/**\"]\n",
    );
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(0));
}

#[test]
fn notes_lists_every_note_without_reading_it() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "prose.md", "Only prose, no items.\n");
    write(&tmp, "broken.md", "```srl\nbroken\n```\n");
    write(&tmp, "plain.txt", "Not a note.\n");
    write(&tmp, ".obsidian/x.md", "Not a note.\n");
    write(&tmp, "templates/t.md", "Excluded.\n");
    write(
        &tmp,
        ".srl/config.toml",
        "[notes]\nexclude = [\"templates/**\"]\n",
    );
    let output = srl(&["notes", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        lines(&output),
        [
            "broken.md",
            "japanese/grammar/wo.md",
            "kanji/fish.md",
            "prose.md"
        ]
    );
    let output = srl(&["notes", "--json", "--dir", d]);
    assert_eq!(
        json_lines(&output)[1],
        serde_json::json!({"path": "japanese/grammar/wo.md"})
    );
    let empty = store(&[]);
    assert_eq!(srl(&["notes", "--dir", dir(&empty)]).status.code(), Some(1));
}

#[test]
fn kinds_describes_qa_without_reading_the_notes() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "broken.md", "```srl\nbroken\n```\n");
    let output = srl(&["kinds", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let lines = lines(&output);
    assert!(lines[0].starts_with("qa  "), "{lines:?}");
    assert!(lines[1].starts_with("  Q  "), "{lines:?}");
    assert!(lines[2].starts_with("  A  "), "{lines:?}");
    let output = srl(&["kinds", "--json", "--dir", d]);
    let kinds = json_lines(&output);
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0]["name"], "qa");
    assert_eq!(kinds[0]["fields"][0]["name"], "Q");
}

const TEXT_NOTE: &str = "```srl id:txt001\nQ: From a text file?\nA: Yes.\n```\n";

#[test]
fn only_md_files_are_notes_by_default() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "plain.txt", "```srl\nbroken\n```\n");
    write(&tmp, "page.html", "```srl\nbroken\n```\n");
    let output = srl(&["find", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(!stdout(&output).contains("plain.txt"));
}

#[test]
fn include_makes_other_files_notes() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "kanji/extra.txt", TEXT_NOTE);
    write(&tmp, "lint.txt", "Q: forgot the header\nA: here\n");
    write(
        &tmp,
        ".srl/config.toml",
        "[notes]\ninclude = [\"**/*.md\", \"**/*.txt\"]\n",
    );
    let output = srl(&["find", "--json", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let paths: Vec<_> = json_lines(&output)
        .iter()
        .map(|card| card["path"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(paths.contains(&"kanji/extra.txt".to_owned()), "{paths:?}");
    assert!(
        paths.contains(&"japanese/grammar/wo.md".to_owned()),
        "{paths:?}"
    );
    // in: drops the .txt extension like any other.
    assert_eq!(lines(&srl(&["find", "in:extra", "--dir", d])).len(), 1);
    // srl lint reads the same files as every other command.
    assert!(stdout(&srl(&["lint", "--dir", d])).starts_with("lint.txt:1: "));
}

#[test]
fn include_replaces_the_default_rather_than_adding_to_it() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "extra.txt", TEXT_NOTE);
    write(&tmp, ".srl/config.toml", "[notes]\ninclude = [\"*.txt\"]\n");
    let output = srl(&["find", "--json", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let cards = json_lines(&output);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0]["path"], "extra.txt");
}

#[test]
fn exclude_wins_over_include() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "drafts/broken.txt", "```srl\nbroken\n```\n");
    write(
        &tmp,
        ".srl/config.toml",
        "[notes]\ninclude = [\"**/*.md\", \"**/*.txt\"]\n",
    );
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(2));
    write(
        &tmp,
        ".srl/config.toml",
        "[notes]\ninclude = [\"**/*.md\", \"**/*.txt\"]\nexclude = [\"drafts/**\"]\n",
    );
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(0));
}

#[test]
fn an_invalid_include_glob_is_a_configuration_error() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, ".srl/config.toml", "[notes]\ninclude = [\"[\"]\n");
    let output = srl(&["find", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("notes.include"),
        "{}",
        stderr(&output)
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn add_takes_paths_that_include_matches_and_exclude_does_not() {
    let tmp = japanese();
    let d = dir(&tmp);
    let item = "{\"Q\": \"q\", \"A\": \"a\"}\n";

    let output = srl_with_input(&["add", "new.txt", "--stdin", "--dir", d], item);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("`include`"), "{}", stderr(&output));
    assert!(!tmp.path().join("new.txt").exists());

    write(
        &tmp,
        ".srl/config.toml",
        "[notes]\ninclude = [\"**/*.md\", \"**/*.txt\"]\nexclude = [\"drafts/**\"]\n",
    );
    let output = srl_with_input(&["add", "drafts/a.txt", "--stdin", "--dir", d], item);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("`exclude`"), "{}", stderr(&output));
    assert!(!tmp.path().join("drafts").exists());

    let output = srl_with_input(&["add", "new.txt", "--stdin", "--dir", d], item);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let id = stdout(&output).trim().to_owned();
    assert!(read(&tmp, "new.txt").contains(&format!("```srl id:{id}\n")));
    assert_eq!(lines(&srl(&["find", "in:new", "--dir", d])).len(), 1);
}

/// What every command that reads memories prints: standard output,
/// standard error and the exit code of each.
fn snapshot(d: &str) -> Vec<(String, String, Option<i32>)> {
    let commands: [&[&str]; 7] = [
        &["find"],
        &["find", "state:due"],
        &["find", "state:new", "-in:none"],
        &["next", "--json"],
        &["stats", "--json"],
        &["log", "--json"],
        &["log", "state:young"],
    ];
    commands
        .iter()
        .map(|args| {
            let output = srl(&[*args, &["--dir", d]].concat());
            (stdout(&output), stderr(&output), output.status.code())
        })
        .collect()
}

#[test]
fn the_index_cache_never_changes_what_commands_print() {
    let tmp = japanese();
    let d = dir(&tmp);
    let cache = tmp.path().join(".srl").join("cache");
    srl(&["review", "x7k2m9", "--rating", "3", "--dir", d]);
    let log = tmp.path().join(".srl").join("log");
    let month = fs::read_dir(&log)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .unwrap();
    let text = fs::read_to_string(&month).unwrap();
    fs::write(&month, format!("{text}not an event\n")).unwrap();

    // Built by the first command, then used.
    let built = snapshot(d);
    assert!(cache.join("index.sqlite").is_file());
    assert_eq!(fs::read_to_string(cache.join(".gitignore")).unwrap(), "*\n");
    assert!(built[4].1.contains("skipped line"), "{}", built[4].1);
    assert_eq!(snapshot(d), built);

    // Deleting the cache loses nothing, and the log's problems are still
    // reported.
    fs::remove_dir_all(&cache).unwrap();
    assert_eq!(snapshot(d), built);

    // Changes to the log show up as they would without a cache.
    srl(&["review", "k3m9q1-1", "--rating", "1", "--dir", d]);
    srl(&["review", "x7k2m9", "--rating", "4", "--dir", d]);
    srl(&["suspend", "id:tuna01", "--dir", d]);
    let cached = snapshot(d);
    fs::remove_dir_all(&cache).unwrap();
    assert_eq!(snapshot(d), cached);
    assert_ne!(cached, built);

    // A damaged cache is rebuilt and reported.
    fs::write(cache.join("index.sqlite"), "garbage ".repeat(100)).unwrap();
    let found = srl(&["find", "--dir", d]);
    assert!(stderr(&found).contains("rebuilt"), "{}", stderr(&found));
    assert_eq!(stdout(&found), cached[0].0);
    assert_eq!(snapshot(d), cached);

    // A cache that can't be used is reported, and the files are read directly.
    fs::remove_dir_all(&cache).unwrap();
    fs::write(&cache, "not a folder").unwrap();
    let found = srl(&["find", "--dir", d]);
    assert!(
        stderr(&found).contains("cannot use the index cache"),
        "{}",
        stderr(&found)
    );
    assert_eq!(stdout(&found), cached[0].0);
    let next = srl(&["next", "--json", "--dir", d]);
    assert_eq!(stdout(&next), cached[3].0);
}

#[test]
fn concurrent_srl_processes_share_the_log_and_the_cache() {
    let tmp = japanese();
    let d = dir(&tmp);
    let spawn = |args: &[&str]| {
        srl_command()
            .args(args)
            .args(["--dir", d])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let children = (0..10)
        .flat_map(|round| {
            let card = if round % 2 == 0 { "x7k2m9" } else { "tuna01" };
            [
                spawn(&["review", card, "--rating", "3"]),
                spawn(&["next", "--json"]),
                spawn(&["stats", "--json"]),
            ]
        })
        .collect::<Vec<_>>();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "{}",
            stderr(&output)
        );
        assert_eq!(stderr(&output), "");
    }
    assert!(tmp.path().join(".srl").join("log").join(".lock").is_file());
    let events = json_lines(&srl(&["log", "--json", "--dir", d]));
    assert_eq!(events.len(), 10);
    let cached = snapshot(d);
    fs::remove_dir_all(tmp.path().join(".srl").join("cache")).unwrap();
    assert_eq!(snapshot(d), cached);
}

#[test]
fn notes_can_live_anywhere_under_the_store() {
    let tmp = japanese();
    fs::create_dir_all(tmp.path().join("deep").join("er")).unwrap();
    let cwd = tmp.path().join("deep").join("er");
    let output = srl_command()
        .args(["find", "in:fish"])
        .current_dir(&cwd)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(lines(&output).len(), 2);
    assert!(Path::new(&cwd).is_dir());
}

/// A store whose kind `fake` is the test kind program of
/// `crates/srl-test-kind`, which `cargo test --workspace` builds next to
/// `srl`, with `notes`.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn store_with_kind(notes: &[(&str, &str)]) -> TempDir {
    let program = Path::new(env!("CARGO_BIN_EXE_srl"))
        .with_file_name(format!("srl-test-kind{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.exists(),
        "{} isn't built; run `cargo test --workspace`",
        program.display()
    );
    let tmp = store(notes);
    write(
        &tmp,
        ".srl/config.toml",
        &format!("[kinds]\nfake = ['{}']\n", program.to_str().unwrap()),
    );
    tmp
}

#[test]
fn cards_of_a_kind_program_in_their_own_groups_are_not_siblings() {
    let tmp = store_with_kind(&[("a.md", "```srl id:abc kind:fake\nGROUPS: a|b c\n```\n")]);
    let d = dir(&tmp);
    let first = next_card(d, &[]);
    assert_eq!(
        (&first["id"], &first["prompt"], &first["prompt_revealed"]),
        (&"abc-a".into(), &"a?".into(), &"a!".into())
    );
    assert_eq!(first["kind"], "fake");
    record(d, &first, 3);
    let second = next_card(d, &[]);
    assert_eq!(second["id"], "abc-b");
    record(d, &second, 3);
    // abc-c waits for tomorrow: its sibling abc-b was reviewed today.
    assert_eq!(srl(&["next", "--dir", d]).status.code(), Some(1));
}

#[test]
fn kind_selects_items_by_their_kind() {
    let tmp = store_with_kind(&[(
        "a.md",
        "```srl\n@ id:abc kind:fake\nGROUPS: a|b\n\n@ id:def\nQ: q\nA: a\n```\n",
    )]);
    let d = dir(&tmp);
    let ids = |selector: &[&str]| -> Vec<String> {
        let output = srl(&[&["find", "--dir", d], selector].concat());
        lines(&output)
            .iter()
            .map(|line| line.split("  ").next().unwrap_or_default().to_owned())
            .collect()
    };
    assert_eq!(ids(&[]), ["abc-a", "abc-b", "def"]);
    assert_eq!(ids(&["kind:fake"]), ["abc-a", "abc-b"]);
    assert_eq!(ids(&["kind:qa"]), ["def"]);
    assert_eq!(ids(&["-kind:fake"]), ["def"]);
    let output = srl(&["find", "kind:nope", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("there is no kind `nope`"));
}

#[test]
fn next_fails_if_its_kind_program_fails_to_make_the_card() {
    let tmp = store_with_kind(&[(
        "a.md",
        "```srl id:abc kind:fake\nGROUPS: a|b\nFAIL_MAKE: a\n```\n",
    )]);
    let d = dir(&tmp);
    // The same failure every time, and abc-b is never tried in its place.
    for _ in 0..2 {
        let output = srl(&["next", "--json", "--dir", d]);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(output.stdout.is_empty());
        assert!(
            stderr(&output).contains("cannot make the card abc-a (fix what failed, or set the card aside with `srl suspend abc-a`): the program of kind fake couldn't make it: card a fails on purpose"),
            "{}",
            stderr(&output)
        );
    }
    // Suspending it is how to get past it.
    assert_eq!(
        srl(&["suspend", "id:abc-a", "--dir", d]).status.code(),
        Some(0)
    );
    let output = srl(&["next", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(lines(&output)[0], "abc-b");
}

#[test]
fn cards_fails_at_once_if_a_kind_program_fails() {
    let tmp = store_with_kind(&[(
        "a.md",
        "```srl id:abc kind:fake\nGROUPS: a|b\nFAIL_MAKE: b\n```\n",
    )]);
    let d = dir(&tmp);
    let output = srl(&["cards", "abc", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("cannot make the card abc-b"));
    let output = srl(&["cards", "abc-a", "--json", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(json_lines(&output)[0]["prompt"], "a?");
}

#[test]
fn an_item_validate_rejects_is_reported_like_any_error_in_a_note() {
    let tmp = store_with_kind(&[(
        "a.md",
        "```srl id:abc kind:fake\nERRORS: {\"errors\": [{\"field\": \"VERB\", \"message\": \"is missing\"}]}\n```\n",
    )]);
    let output = srl(&["check", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("a.md:1: VERB: is missing"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_kind_program_that_fails_is_reported_once_and_stops_commands() {
    let tmp = store_with_kind(&[(
        "a.md",
        "```srl id:abc kind:fake\nBREAK_GROUPS: on purpose\n```\n\n```srl id:abd kind:fake\nX: y\n```\n",
    )]);
    let d = dir(&tmp);
    let output = srl(&["check", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output).lines().next().unwrap(),
        "srl: the program of kind fake failed: on purpose; no item of kind fake can be read until \
         the program works"
    );
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(2));
}

#[test]
fn kind_answers_are_kept_until_check_asks_again_or_the_cache_is_deleted() {
    let tmp = store_with_kind(&[(
        "a.md",
        "```srl id:abc kind:fake\nGROUPS: a b\nLOG: calls.log\n```\n",
    )]);
    let d = dir(&tmp);
    let calls_after = |args: &[&str]| {
        write(&tmp, "calls.log", "");
        let output = srl(&[args, &["--dir", d]].concat());
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        read(&tmp, "calls.log")
    };
    assert_eq!(calls_after(&["find"]), "validate 1\ngroups 1\n");
    assert_eq!(calls_after(&["find"]), "");
    // srl check asks kind programs about every item.
    assert_eq!(calls_after(&["check"]), "validate 1\ngroups 1\n");
    assert_eq!(calls_after(&["find"]), "");
    // Deleting the cache forgets the answers; the next command asks again.
    fs::remove_dir_all(tmp.path().join(".srl").join("cache")).unwrap();
    assert_eq!(calls_after(&["find"]), "validate 1\ngroups 1\n");
}

#[test]
fn add_checks_every_new_item_at_once_and_names_the_line_at_fault() {
    let tmp = store_with_kind(&[]);
    let d = dir(&tmp);
    let input = "{\"kind\": \"fake\", \"X\": \"a\", \"LOG\": \"calls.log\"}\n\n{\"kind\": \"fake\", \"GROUPS\": \"a b\", \"LOG\": \"calls.log\"}\n{\"kind\": \"fake\", \"INVALID\": \"no good\"}\n";
    let output = srl_with_input(
        &["add", "new.md", "--stdin", "--dry-run", "--dir", d],
        input,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("line 4 of standard input is not a valid item: no good"),
        "{}",
        stderr(&output)
    );
    assert_eq!(read(&tmp, "calls.log"), "validate 3\ngroups 2\n");
}

/// Runs `srl` as if it were `now`, in UTC.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn srl_at(now: &str, args: &[&str]) -> Output {
    srl_command()
        .args(args)
        .env("SRL_NOW", now)
        .env("TZ", "UTC")
        .output()
        .unwrap()
}

#[test]
fn srl_now_is_the_time_of_reviews_and_of_the_study_day() {
    let tmp = japanese();
    let d = dir(&tmp);
    let day = "2026-01-05T09:00:00Z";
    let reviewed = srl_at(day, &["review", "tuna01", "--rating", "3", "--dir", d]);
    assert_eq!(reviewed.status.code(), Some(0), "{}", stderr(&reviewed));
    let log = json_lines(&srl(&["log", "--json", "--dir", d]));
    assert_eq!(log[0]["t"], "2026-01-05T09:00:00+00:00");
    let today = srl_at(day, &["next", "id:tuna01", "--dir", d]);
    assert_eq!(today.status.code(), Some(1), "{}", stderr(&today));
    let next_year = srl_at("2027-01-05T09:00:00Z", &["next", "id:tuna01", "--dir", d]);
    assert_eq!(next_year.status.code(), Some(0), "{}", stderr(&next_year));
}

#[test]
fn srl_seed_makes_new_ids_repeatable() {
    let ids = || {
        let tmp = store(&[("n.md", "```srl\n@\nQ: a\nA: b\n\n@\nQ: c\nA: d\n```\n")]);
        let output = srl_command()
            .args(["id", "--dir", dir(&tmp)])
            .env("SRL_SEED", "7")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        read(&tmp, "n.md")
    };
    let first = ids();
    assert!(first.contains("id:"), "{first}");
    assert_eq!(ids(), first);
}

#[test]
fn invalid_srl_now_and_srl_seed_exit_with_2_and_explain() {
    let tmp = japanese();
    let d = dir(&tmp);
    let now = srl_at("yesterday", &["next", "--dir", d]);
    assert_eq!(now.status.code(), Some(2));
    assert!(stderr(&now).contains("SRL_NOW"), "{}", stderr(&now));
    let seed = srl_command()
        .args(["next", "--dir", d])
        .env("SRL_SEED", "-1")
        .output()
        .unwrap();
    assert_eq!(seed.status.code(), Some(2));
    assert!(stderr(&seed).contains("SRL_SEED"), "{}", stderr(&seed));
}
