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
    assert!(stdout.contains("notes"));
}

#[test]
fn jsonl_json_and_xml_collections_are_read_and_written_alike() {
    let xml = "<items>\n  <!-- By hand. -->\n  <item id=\"xml1\" see=\"rust.md\">\n    <Q>\n      Does this compile?\n\n          let b = a;\n    </Q>\n    <A>No.</A>\n  </item>\n</items>\n";
    let tmp = store(&[
        ("rust.md", "# Rust\n"),
        (
            "a.jsonl",
            "{\"id\": \"jsl1\", \"Q\": \"q\", \"A\": \"a\"}\n",
        ),
        ("b.json", "[{\"id\": \"jsn1\", \"Q\": \"q\", \"A\": \"a\"}]"),
        ("c.xml", xml),
        (
            ".srl/config.toml",
            "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"**/*.jsonl\", \"**/*.json\", \"**/*.xml\"]\n",
        ),
    ]);
    let found = srl(&["find", "--dir", dir(&tmp)]);
    assert_eq!(found.status.code(), Some(0), "{}", stderr(&found));
    assert_eq!(
        lines(&found),
        [
            "jsl1  a.jsonl  q",
            "jsn1  b.json  q",
            "xml1  c.xml  Does this compile?"
        ]
    );
    let shown = json_lines(&srl(&["show", "xml1", "--json", "--dir", dir(&tmp)]));
    assert_eq!(shown[0]["Q"], "Does this compile?\n\n    let b = a;");
    assert_eq!(shown[0]["context"], "# Rust\n");
    assert_eq!(
        stdout(&srl(&["show", "xml1", "--no-context", "--dir", dir(&tmp)])),
        "c.xml\n<item id=\"xml1\" see=\"rust.md\">\n  <Q>\n    Does this compile?\n\n        let b = a;\n  </Q>\n  <A>No.</A>\n</item>\n"
    );
    // An edit changes only the element of its item; an add goes before
    // `</items>`.
    let edited = srl_with_input(
        &["edit", "--stdin", "--dir", dir(&tmp)],
        "{\"id\": \"xml1\", \"see\": \"rust.md\", \"Q\": \"Does this compile?\\n\\n    let b = a;\", \"A\": \"No: `a` < `b`.\"}\n{\"id\": \"jsn1\", \"draft\": true, \"Q\": \"q\", \"A\": \"a\"}\n",
    );
    assert_eq!(edited.status.code(), Some(0), "{}", stderr(&edited));
    let added = srl_with_input(
        &["add", "c.xml", "--stdin", "--dir", dir(&tmp)],
        "{\"id\": \"xml2\", \"Q\": \"Two\\nlines\", \"A\": \"a\"}\n",
    );
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    assert_eq!(
        read(&tmp, "c.xml"),
        "<items>\n  <!-- By hand. -->\n  <item id=\"xml1\" see=\"rust.md\">\n    <Q>\n      Does this compile?\n\n          let b = a;\n    </Q>\n    <A><![CDATA[No: `a` < `b`.]]></A>\n  </item>\n  <item id=\"xml2\">\n    <Q>\n      Two\n      lines\n    </Q>\n    <A>a</A>\n  </item>\n</items>\n"
    );
    assert_eq!(
        read(&tmp, "b.json"),
        "[\n  {\n    \"id\": \"jsn1\",\n    \"draft\": true,\n    \"Q\": \"q\",\n    \"A\": \"a\"\n  }\n]\n"
    );
    // A new collection of each format is created by its extension.
    for (path, text) in [
        (
            "new/d.json",
            "[\n  {\n    \"id\": \"new1\",\n    \"Q\": \"q\",\n    \"A\": \"a\"\n  }\n]\n",
        ),
        (
            "new/e.xml",
            "<items>\n  <item id=\"new2\">\n    <Q>q</Q>\n    <A>a</A>\n  </item>\n</items>\n",
        ),
    ] {
        let id = if path.ends_with("json") {
            "new1"
        } else {
            "new2"
        };
        let added = srl_with_input(
            &["add", path, "--stdin", "--dir", dir(&tmp)],
            &format!("{{\"id\": \"{id}\", \"Q\": \"q\", \"A\": \"a\"}}\n"),
        );
        assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
        assert_eq!(read(&tmp, path), text);
    }
    let checked = srl(&["check", "--dir", dir(&tmp)]);
    assert_eq!(checked.status.code(), Some(0), "{}", stderr(&checked));
    // An XML error names its line.
    write(
        &tmp,
        "c.xml",
        "<items>\n  <item drfat=\"true\"><Q>q</Q></item>\n</items>\n",
    );
    let checked = srl(&["check", "--dir", dir(&tmp)]);
    assert_eq!(checked.status.code(), Some(2));
    assert!(
        stderr(&checked).contains("c.xml: line 2: `drfat` is not a header entry"),
        "{}",
        stderr(&checked)
    );
}

#[test]
fn every_format_names_an_item_in_error_by_its_id_or_position() {
    let tmp = store(&[
        ("a.jsonl", "{\"id\": \"jsl1\", \"Q\": \"no answer\"}\n"),
        (
            "b.json",
            "[{\"Q\": \"q\", \"A\": \"a\"}, {\"id\": \"jsn1\", \"Q\": \"no answer\"}]",
        ),
        (
            "c.xml",
            "<items>\n  <item><Q>q</Q><A>a</A></item>\n  <item id=\"xml1\"><Q>no answer</Q></item>\n</items>\n",
        ),
        (
            ".srl/config.toml",
            "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"*.jsonl\", \"*.json\", \"*.xml\"]\n",
        ),
    ]);
    let output = srl(&["check", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    let problems = stderr(&output);
    let problems = problems.lines().collect::<Vec<_>>();
    let answer = "a qa item needs an answer in its field A, unless Q has cloze deletions";
    assert_eq!(
        problems,
        [
            format!("srl: a.jsonl: item jsl1: {answer}"),
            "srl: b.json: item 1 has no ID; run `srl id` to give it one".to_owned(),
            format!("srl: b.json: item jsn1: {answer}"),
            "srl: c.xml: item 1 has no ID; run `srl id` to give it one".to_owned(),
            format!("srl: c.xml: item xml1: {answer}"),
        ]
    );
}

#[test]
fn a_note_srl_gives_no_text_for_is_named_but_not_shown() {
    let tmp = store(&[
        (
            "a.jsonl",
            "{\"id\": \"aaa1\", \"see\": \"paper.pdf\", \"Q\": \"q\", \"A\": \"a\"}\n",
        ),
        (".srl/config.toml", "[context]\ninclude = [\"*.pdf\"]\n"),
    ]);
    write_bytes(&tmp, "paper.pdf", &[0x25, 0x50, 0x44, 0x46, 0xff, 0xfe]);
    let d = dir(&tmp);
    let shown = srl(&["show", "aaa1", "--dir", d]);
    assert_eq!(shown.status.code(), Some(0), "{}", stderr(&shown));
    assert_eq!(
        stdout(&shown),
        "a.jsonl\n{\"id\": \"aaa1\", \"see\": \"paper.pdf\", \"Q\": \"q\", \"A\": \"a\"}\n--- paper.pdf\n"
    );
    let next = json_lines(&srl(&["next", "--json", "--dir", d]));
    assert_eq!(next[0]["see"], "paper.pdf");
    assert_eq!(next[0]["context"], serde_json::Value::Null);
    let notes = srl(&["notes", "--json", "--dir", d]);
    assert_eq!(
        json_lines(&notes),
        [serde_json::json!({"path": "paper.pdf"})]
    );
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn write_bytes(tmp: &TempDir, path: &str, bytes: &[u8]) {
    fs::write(tmp.path().join(path), bytes).unwrap();
}

#[test]
fn usage_errors_exit_with_2() {
    let output = srl(&["review", "a7f3k2", "--rating", "9"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
}

#[test]
fn unknown_commands_exit_with_2_and_explain() {
    let output = srl(&["no-such-command-for-srl-tests"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("no-such-command-for-srl-tests"));
    assert_eq!(output.stdout, b"");
}

/// A temporary directory with a fresh store in it, and `files` written.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn store(files: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    assert_eq!(srl(&["init", dir(&tmp)]).status.code(), Some(0));
    for (path, text) in files {
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

/// Study material: what the items of [`GRAMMAR`] name with `see`.
const GRAMMAR_NOTE: &str = "# 〜をものともせず

Undeterred by (adversity). Admiring tone.
";

const GRAMMAR: &str = r#"{"id":"x7k2m9","see":"japanese/grammar/wo.md","Q":"What does 〜をものともせず mean?","A":"Undeterred by (adversity), braving."}
{"id":"k3m9q1","see":"japanese/grammar/wo.md","Q":"{{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。","A":"Despite everyone's opposition, she started a company."}
"#;

const FISH: &str = r#"{"id":"tuna01","Q":"鮪","A":"まぐろ — tuna"}
{"id":"saba01","draft":true,"Q":"鯖","A":"さば — mackerel"}
"#;

fn japanese() -> TempDir {
    store(&[
        ("japanese/grammar/wo.md", GRAMMAR_NOTE),
        ("japanese/grammar/wo.jsonl", GRAMMAR),
        ("kanji/fish.jsonl", FISH),
    ])
}

/// Line `n` (from 0) of `text`, with its line ending.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn line(text: &str, n: usize) -> String {
    format!("{}\n", text.lines().nth(n).unwrap())
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
            "x7k2m9  japanese/grammar/wo.jsonl  What does 〜をものともせず mean?",
            "k3m9q1.1  japanese/grammar/wo.jsonl  {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。",
            "k3m9q1.2  japanese/grammar/wo.jsonl  {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。",
            "tuna01  kanji/fish.jsonl  鮪",
            "saba01  kanji/fish.jsonl  鯖",
        ]
    );
}

#[test]
fn ids_written_by_hand_may_have_dashes_and_a_dot_names_the_card() {
    let tmp = store(&[(
        "grammar.jsonl",
        "{\"id\": \"jp-grammar-desu\", \"Q\": \"{{c1::です}} is {{c2::polite}}\"}\n",
    )]);
    let d = dir(&tmp);
    assert_eq!(srl(&["check", "--dir", d]).status.code(), Some(0));
    let cards = |selector: &str| {
        let output = srl(&["find", selector, "--dir", d]);
        lines(&output)
            .iter()
            .map(|line| line.split_whitespace().next().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        cards("id:jp-grammar-desu"),
        ["jp-grammar-desu.1", "jp-grammar-desu.2"]
    );
    assert_eq!(cards("id:jp-grammar-desu.2"), ["jp-grammar-desu.2"]);
    // `jp-grammar` is another item, not the start of this one.
    assert_eq!(cards("id:jp-grammar"), Vec::<String>::new());
    let output = srl(&["cards", "jp-grammar-desu.1", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    // A dot is never part of an item ID, and a dash only joins words.
    for bad in ["jp.grammar", "-jp", "jp-", "jp--grammar"] {
        write(
            &tmp,
            "grammar.jsonl",
            &format!("{{\"id\": \"{bad}\", \"Q\": \"a\", \"A\": \"b\"}}\n"),
        );
        let check = srl(&["check", "--dir", d]);
        assert_eq!(check.status.code(), Some(2), "accepted {bad}");
        assert!(
            stderr(&check).contains("grammar.jsonl: line 1:"),
            "{}",
            stderr(&check)
        );
    }
    for bad in [
        "id:jp.grammar.desu",
        "id:jp--grammar",
        "id:jp-grammar-desu.",
    ] {
        assert_eq!(
            srl(&["find", bad, "--dir", d]).status.code(),
            Some(2),
            "accepted {bad}"
        );
    }
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
    assert_eq!(output.stdout, b"");
}

#[test]
fn in_names_collections_not_study_material() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "notes/fish.md", "# Fish\n");
    write(&tmp, "notes/sushi.md", "# Sushi\n");
    // `in:fish` is the collection kanji/fish.jsonl, not the note beside it.
    assert_eq!(lines(&srl(&["find", "in:fish", "--dir", d])).len(), 2);
    // A note holds no items, whatever its name.
    assert_eq!(
        srl(&["find", "in:sushi", "--dir", d]).status.code(),
        Some(1)
    );
    assert_eq!(
        srl(&["find", "in:notes", "--dir", d]).status.code(),
        Some(1)
    );
}

#[test]
fn find_json_prints_one_card_per_line() {
    let tmp = japanese();
    let output = srl(&["find", "id:k3m9q1.2,saba01", "--json", "--dir", dir(&tmp)]);
    assert_eq!(
        json_lines(&output),
        [
            serde_json::json!({"id": "k3m9q1.2", "item": "k3m9q1", "collection": "japanese/grammar/wo.jsonl",
                "source": "items", "status": "active", "schedule": "new", "leech": false, "due": null, "suspend_reason": null}),
            serde_json::json!({"id": "saba01", "item": "saba01", "collection": "kanji/fish.jsonl",
                "source": "items", "status": "draft", "schedule": "new", "leech": false, "due": null, "suspend_reason": null}),
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
fn errors_in_collections_stop_commands_until_fixed() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(
        &tmp,
        "broken.jsonl",
        "{\"id\": \"bad1\", \"darft\": true, \"Q\": \"a\"}\n{\"Q\": \"no id\", \"A\": \"x\"}\n",
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
            stderr.contains("broken.jsonl: line 1: `darft` is neither a header entry"),
            "{stderr}"
        );
        assert!(
            stderr.contains("broken.jsonl: item 1 has no ID; run `srl id` to give it one"),
            "{stderr}"
        );
    }
    let check = srl(&["check", "--dir", d]);
    assert_eq!(check.status.code(), Some(2));
    assert!(stderr(&check).contains("broken.jsonl: line 1:"));
    // srl id refuses while another problem remains.
    let before = read(&tmp, "broken.jsonl");
    let refused = srl(&["id", "--dir", d]);
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(read(&tmp, "broken.jsonl"), before);
    write(&tmp, "broken.jsonl", "{\"Q\": \"no id\", \"A\": \"x\"}\n");
    let assigned = srl(&["id", "--dir", d]);
    assert_eq!(assigned.status.code(), Some(0), "{}", stderr(&assigned));
    let line = stdout(&assigned);
    let id = line.split_whitespace().next().unwrap();
    assert_eq!(line, format!("{id}  broken.jsonl\n"));
    assert_eq!(
        read(&tmp, "broken.jsonl"),
        format!("{{\"id\":\"{id}\",\"Q\":\"no id\",\"A\":\"x\"}}\n")
    );
    assert_eq!(srl(&["id", "--dir", d]).status.code(), Some(1));
    assert_eq!(srl(&["check", "--dir", d]).status.code(), Some(0));
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(0));
}

#[test]
fn show_prints_the_item_and_the_file_its_see_names() {
    let tmp = japanese();
    let d = dir(&tmp);
    let output = srl(&["show", "k3m9q1.2", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "japanese/grammar/wo.jsonl\n{}--- japanese/grammar/wo.md\n{GRAMMAR_NOTE}",
            line(GRAMMAR, 1)
        )
    );
    let shown = srl(&["show", "x7k2m9", "--json", "--dir", d]);
    let shown = &json_lines(&shown)[0];
    assert_eq!(shown["kind"], "qa");
    assert_eq!(shown["Q"], "What does 〜をものともせず mean?");
    assert_eq!(shown["cards"], serde_json::json!(["x7k2m9"]));
    assert_eq!(shown["see"], "japanese/grammar/wo.md");
    assert_eq!(shown["context"], GRAMMAR_NOTE);
    let keys = shown
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    assert_eq!(
        keys,
        [
            "id",
            "kind",
            "see",
            "draft",
            "Q",
            "A",
            "collection",
            "source",
            "cards",
            "context"
        ]
    );
    // --no-context leaves it out.
    let plain = srl(&["show", "x7k2m9", "--no-context", "--dir", d]);
    assert_eq!(
        stdout(&plain),
        format!("japanese/grammar/wo.jsonl\n{}", line(GRAMMAR, 0))
    );
    let none = srl(&["show", "x7k2m9", "--no-context", "--json", "--dir", d]);
    assert_eq!(json_lines(&none)[0]["context"], serde_json::Value::Null);
    // An item without `see` has no context.
    let none = srl(&["show", "tuna01", "--json", "--dir", d]);
    assert_eq!(json_lines(&none)[0]["context"], serde_json::Value::Null);
}

#[test]
fn show_reports_unknown_ids_and_fails() {
    let tmp = japanese();
    let output = srl(&["show", "x7k2m9", "zzz999", "x7k2m9.1", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).contains("x7k2m9"));
    let stderr = stderr(&output);
    assert!(stderr.contains("zzz999"), "{stderr}");
    assert!(stderr.contains("x7k2m9.1"), "{stderr}");
}

#[test]
fn cards_prints_the_cards_of_items_and_cards() {
    let tmp = japanese();
    let output = srl(&["cards", "k3m9q1", "--dir", dir(&tmp)]);
    assert_eq!(
        stdout(&output),
        "k3m9q1.1\n==[...]==を==[...]==、彼女は起業した。\n---\n==周囲の反対==を==ものともせず==、彼女は起業した。\n---\nDespite everyone's opposition, she started a company.\n\nk3m9q1.2\n周囲の反対をものともせず、==[who]==は起業した。\n---\n周囲の反対をものともせず、==彼女==は起業した。\n---\nDespite everyone's opposition, she started a company.\n"
    );
    let one = srl(&["cards", "k3m9q1.2", "--json", "--dir", dir(&tmp)]);
    let cards = json_lines(&one);
    assert_eq!(cards.len(), 1);
    assert_eq!(
        cards[0],
        serde_json::json!({
            "id": "k3m9q1.2",
            "prompt": "周囲の反対をものともせず、==[who]==は起業した。",
            "prompt_revealed": "周囲の反対をものともせず、==彼女==は起業した。",
            "answer": "Despite everyone's opposition, she started a company.",
        })
    );
}

#[test]
fn add_writes_items_with_their_header_entries() {
    let tmp = japanese();
    let d = dir(&tmp);
    let input = "{\"Q\": \"Use 〜をものともせず\\nabout football.\", \"A\": \"…\", \"see\": \"japanese/grammar/wo.md\", \"draft\": true}\n{\"Q\": \"{{c1::嵐}}をものともせず\", \"draft\": true}\n";
    let args = ["add", "generated/wo.jsonl", "--stdin", "--dir", d];
    let dry = srl_with_input(&[&args[..], &["--dry-run"]].concat(), input);
    assert_eq!(dry.status.code(), Some(0), "{}", stderr(&dry));
    assert!(
        stdout(&dry).contains(
            "\"see\":\"japanese/grammar/wo.md\",\"draft\":true,\"Q\":\"Use 〜をものともせず\\nabout football.\""
        ),
        "{}",
        stdout(&dry)
    );
    assert_eq!(stdout(&dry).matches("\"see\":").count(), 1);
    assert!(!tmp.path().join("generated").exists());
    let added = srl_with_input(&args, input);
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let ids = lines(&added);
    assert_eq!(ids.len(), 2);
    let drafts = srl(&["find", "in:generated", "is:draft", "--dir", d]);
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
        &["add", "a.jsonl", "--stdin", "--dir", dir(&tmp)],
        "{\"Q\": \"q\", \"A\": \"a\"}\n{\"Q\": \"no answer\"}\n",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("line 2"), "{}", stderr(&output));
    assert!(!tmp.path().join("a.jsonl").exists());
}

#[test]
fn drafts_move_through_accept_and_cards_through_suspend() {
    let tmp = japanese();
    let d = dir(&tmp);
    assert_eq!(srl(&["next", "id:saba01", "--dir", d]).stdout, b"");
    let accepted = srl(&["accept", "in:fish", "--dir", d]);
    assert_eq!(lines(&accepted), ["saba01"]);
    assert_eq!(
        read(&tmp, "kanji/fish.jsonl"),
        FISH.replace("\"draft\":true,", "")
    );
    assert_eq!(
        srl(&["accept", "in:fish", "--dir", d]).status.code(),
        Some(1)
    );
    let suspended = srl(&["suspend", "id:k3m9q1.2,tuna01", "--dir", d]);
    assert_eq!(lines(&suspended), ["k3m9q1.2", "tuna01"]);
    let found = srl(&["find", "is:suspended", "--dir", d]);
    assert_eq!(lines(&found).len(), 2);
    let back = srl(&["unsuspend", "--dir", d]);
    assert_eq!(lines(&back), ["k3m9q1.2", "tuna01"]);
    // Suspending never changes a collection.
    assert!(!read(&tmp, "kanji/fish.jsonl").contains("suspend"));
}

#[test]
fn a_suspension_keeps_its_reason_until_another_replaces_it() {
    let tmp = japanese();
    let d = dir(&tmp);
    let reason = |tmp: &TempDir| {
        json_lines(&srl(&["find", "id:tuna01", "--json", "--dir", dir(tmp)]))[0]["suspend_reason"]
            .clone()
    };
    assert_eq!(reason(&tmp), serde_json::Value::Null);
    srl(&[
        "suspend",
        "id:tuna01",
        "--reason",
        "too hard\nfor now",
        "--dir",
        d,
    ]);
    assert_eq!(reason(&tmp), "too hard\nfor now");
    // Without a reason, a suspended card is left alone; with one, it is
    // suspended again.
    let again = srl(&["suspend", "id:tuna01", "--dir", d]);
    assert_eq!(again.status.code(), Some(1));
    let again = srl(&["suspend", "id:tuna01", "--reason", "split it", "--dir", d]);
    assert_eq!(lines(&again), ["tuna01"]);
    assert_eq!(reason(&tmp), "split it");
    let log = lines(&srl(&["log", "id:tuna01", "--dir", d]));
    assert!(log[0].ends_with("  tuna01  suspend  too hard"), "{log:?}");
    assert!(log[1].ends_with("  tuna01  suspend  split it"), "{log:?}");
    srl(&["unsuspend", "id:tuna01", "--dir", d]);
    assert_eq!(reason(&tmp), serde_json::Value::Null);
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
    assert_eq!(
        read(&tmp, "japanese/grammar/wo.jsonl"),
        format!(
            "{{\"id\":\"x7k2m9\",\"Q\":\"What does 〜をものともせず mean?\",\"A\":\"Braving.\"}}\n{}",
            line(GRAMMAR, 1)
        )
    );
    // What `srl show --json` prints goes back as it is, and the item keeps
    // its place.
    let shown = stdout(&srl(&["show", "tuna01", "--json", "--dir", d]));
    let changed = shown.replace("まぐろ — tuna", "まぐろ");
    let output = srl_with_input(&["edit", "--stdin", "--dir", d], &changed);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        read(&tmp, "kanji/fish.jsonl"),
        FISH.replace("まぐろ — tuna", "まぐろ")
    );
    let wrong = srl_with_input(
        &["edit", "--stdin", "--dir", d],
        "{\"id\": \"other\", \"Q\": \"q\", \"A\": \"a\"}\n",
    );
    assert_eq!(wrong.status.code(), Some(2));
    assert!(stderr(&wrong).contains("line 1"), "{}", stderr(&wrong));
}

#[test]
fn see_names_study_material_and_items_without_it_have_no_context() {
    let tmp = japanese();
    let d = dir(&tmp);
    let items = "{\"Q\": \"鯛\", \"A\": \"たい — sea bream\"}\n{\"id\": \"wo0001\", \"see\": \"japanese/grammar/wo.md\", \"Q\": \"{{c1::嵐}}をものともせず\"}\n";
    write(&tmp, "generated/items.jsonl", items);
    let ids = srl(&["id", "--dir", d]);
    assert_eq!(ids.status.code(), Some(0), "{}", stderr(&ids));
    assert!(stdout(&ids).ends_with("  generated/items.jsonl\n"));
    let found = srl(&["find", "in:generated", "--dir", d]);
    let found = lines(&found);
    assert_eq!(found.len(), 2);
    assert!(found[1].starts_with("wo0001.1  generated/items.jsonl  "));
    let id = found[0].split_whitespace().next().unwrap().to_owned();
    // Without `see`, an item has no context: its collection is data.
    let own = srl(&["show", &id, "--json", "--dir", d]);
    assert_eq!(json_lines(&own)[0]["context"], serde_json::Value::Null);
    let own = next_card(d, &[&format!("id:{id}")]);
    assert_eq!(own["collection"], "generated/items.jsonl");
    assert_eq!(own["see"], serde_json::Value::Null);
    assert_eq!(own["context"], serde_json::Value::Null);
    let seen = next_card(d, &["id:wo0001"]);
    assert_eq!(seen["see"], "japanese/grammar/wo.md");
    assert_eq!(seen["context"], GRAMMAR_NOTE);
    // A collection is no study material: `see` can't name it, and neither
    // a file that isn't there or a file outside `[context]`.
    write(&tmp, "notes/table.csv", "a,b\n");
    for (see, reason) in [
        ("generated/items.jsonl", "is not study material"),
        ("notes/table.csv", "is not study material"),
        ("notes/missing.md", "names no file of the store"),
        ("../wo.md", "is not a path relative to the store's folder"),
    ] {
        let input =
            format!("{{\"Q\": \"Which fish?\", \"A\": \"Sea bream.\", \"see\": \"{see}\"}}\n");
        let added = srl_with_input(&["add", "fish.jsonl", "--stdin", "--dir", d], &input);
        assert_eq!(added.status.code(), Some(2), "accepted {see}");
        assert!(stderr(&added).contains(reason), "{see}: {}", stderr(&added));
    }
    assert!(!tmp.path().join("fish.jsonl").exists());
    // `[context]` chooses the files of study material.
    write(
        &tmp,
        ".srl/config.toml",
        "[context]\ninclude = [\"**/*.md\", \"notes/*.csv\"]\n",
    );
    let input = "{\"Q\": \"Which fish?\", \"A\": \"Sea bream.\", \"see\": \"notes/table.csv\"}\n";
    let added = srl_with_input(&["add", "fish.jsonl", "--stdin", "--dir", d], input);
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let shown = srl(&["show", &lines(&added)[0], "--json", "--dir", d]);
    assert_eq!(json_lines(&shown)[0]["context"], "a,b\n");
    // A file that is not UTF-8 is study material with no text to show.
    fs::write(tmp.path().join("notes/table.csv"), [0xff, 0xfe, 0x00]).unwrap();
    let shown = srl(&["show", &lines(&added)[0], "--json", "--dir", d]);
    assert_eq!(shown.status.code(), Some(0), "{}", stderr(&shown));
    assert_eq!(json_lines(&shown)[0]["context"], serde_json::Value::Null);
    // A `see` whose file is gone is an error in the store.
    fs::remove_file(tmp.path().join("notes/table.csv")).unwrap();
    let broken = srl(&["find", "--dir", d]);
    assert_eq!(broken.status.code(), Some(2));
    assert!(
        stderr(&broken).contains("fish.jsonl: item "),
        "{}",
        stderr(&broken)
    );
    assert!(
        stderr(&broken).contains("`see` notes/table.csv names no file of the store"),
        "{}",
        stderr(&broken)
    );
    // An invalid line stops every command that reads items.
    write(&tmp, "fish.jsonl", "");
    write(&tmp, "generated/items.jsonl", "{\"Q\": \"q\", \"B\": 1}\n");
    let broken = srl(&["find", "--dir", d]);
    assert_eq!(broken.status.code(), Some(2));
    assert!(
        stderr(&broken).contains("generated/items.jsonl: line 1:"),
        "{}",
        stderr(&broken)
    );
}

#[test]
fn notes_lists_the_study_material_without_reading_any_item() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "notes/b.txt", "Text.\n");
    write(&tmp, "notes/c.pdf", "%PDF");
    write(&tmp, ".obsidian/x.md", "Hidden.\n");
    write(&tmp, "broken.jsonl", "not json\n");
    let output = srl(&["notes", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(lines(&output), ["japanese/grammar/wo.md", "notes/b.txt"]);
    let output = srl(&["notes", "--json", "--dir", d]);
    assert_eq!(
        json_lines(&output),
        [
            serde_json::json!({"path": "japanese/grammar/wo.md"}),
            serde_json::json!({"path": "notes/b.txt"}),
        ]
    );
    write(
        &tmp,
        ".srl/config.toml",
        "[context]\ninclude = [\"notes/**\"]\nexclude = [\"**/*.txt\"]\n",
    );
    assert_eq!(lines(&srl(&["notes", "--dir", d])), ["notes/c.pdf"]);
    let empty = store(&[]);
    assert_eq!(srl(&["notes", "--dir", dir(&empty)]).status.code(), Some(1));
}

#[test]
fn check_warns_of_likely_mistakes_and_reports_errors_in_lines() {
    let tmp = store(&[(
        "items.jsonl",
        "{\"id\": \"abc\", \"Q\": \"{{C1::upper}}\", \"A\": \"a\"}\n",
    )]);
    let d = dir(&tmp);
    let check = srl(&["check", "--dir", d]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
    assert!(
        stderr(&check)
            .starts_with("srl: warning: items.jsonl: item abc: a line of Q looks like a cloze"),
        "{}",
        stderr(&check)
    );
    write(
        &tmp,
        "items.jsonl",
        "{\"id\": \"abc\", \"Q\": \"q\", \"A\": \"a\"}\n\n{\"id\": \"abd\", \"Q\": \"q\", \"Q\": \"r\"}\n[1]\n",
    );
    let check = srl(&["check", "--dir", d]);
    assert_eq!(check.status.code(), Some(2));
    let problems = stderr(&check);
    assert!(problems.contains("items.jsonl: line 3: "), "{problems}");
    assert!(problems.contains("appears twice"), "{problems}");
    assert!(problems.contains("items.jsonl: line 4: "), "{problems}");
}

#[test]
fn edit_dry_run_prints_the_ids_and_empty_input_finds_nothing() {
    let tmp = japanese();
    let d = dir(&tmp);
    let input = "{\"id\": \"tuna01\", \"Q\": \"鮪\", \"A\": \"まぐろ\"}\n{\"id\": \"x7k2m9\", \"Q\": \"q\", \"A\": \"a\"}\n";
    let dry = srl_with_input(&["edit", "--stdin", "--dry-run", "--dir", d], input);
    assert_eq!(dry.status.code(), Some(0), "{}", stderr(&dry));
    assert_eq!(lines(&dry), ["tuna01", "x7k2m9"]);
    assert_eq!(read(&tmp, "kanji/fish.jsonl"), FISH);
    let empty = srl_with_input(&["edit", "--stdin", "--dir", d], "\n");
    assert_eq!(empty.status.code(), Some(1), "{}", stderr(&empty));
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
    assert_eq!(second["id"], "k3m9q1.1");
    record(d, &second, 1);
    // k3m9q1.2 waits for tomorrow: its sibling was reviewed today.
    let third = next_card(d, &[]);
    assert_eq!(third["id"], "tuna01");
    record(d, &third, 4);
    let none = srl(&["next", "--dir", d]);
    assert_eq!(none.status.code(), Some(1));
    let log = json_lines(&srl(&["log", "--json", "--dir", d]));
    assert_eq!(log.len(), 3);
    assert_eq!(log[1]["card"], "k3m9q1.1");
    assert_eq!(log[1]["hash"].as_str().unwrap().len(), 12);
    let human = srl(&["log", "id:k3m9q1", "--dir", d]);
    assert!(
        stdout(&human).contains("  k3m9q1.1  review  1  human"),
        "{}",
        stdout(&human)
    );
}

#[test]
fn next_json_gives_the_card_with_its_state_collection_and_context() {
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
            "schedule": "new",
            "leech": false,
            "kind": "qa",
            "collection": "japanese/grammar/wo.jsonl",
            "source": "items",
            "see": "japanese/grammar/wo.md",
            "context": GRAMMAR_NOTE,
        })
    );
    let none = next_card(d, &["--no-context"]);
    assert_eq!(none["see"], "japanese/grammar/wo.md");
    assert_eq!(none["context"], serde_json::Value::Null);
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
        "k3m9q1.1",
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
    let output = srl(&["find", "is:young", "--dir", d]);
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
    // The question gets its first cloze deletion: x7k2m9 becomes x7k2m9.1.
    let text = read(&tmp, "japanese/grammar/wo.jsonl").replace(
        "\"Q\":\"What does 〜をものともせず mean?\"",
        "\"Q\":\"What does 〜を{{c1::ものともせず}} mean?\"",
    );
    write(&tmp, "japanese/grammar/wo.jsonl", &text);
    let check = srl(&["check", "--dir", d]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
    assert!(
        stderr(&check).contains("x7k2m9 has reviews but no card; x7k2m9.1 is new. To keep the history: srl move x7k2m9 x7k2m9.1"),
        "{}",
        stderr(&check)
    );
    let moved = srl(&["move", "x7k2m9", "x7k2m9.1", "--dir", d]);
    assert_eq!(moved.status.code(), Some(0), "{}", stderr(&moved));
    assert_eq!(srl(&["check", "--dir", d]).stderr, b"");
    let young = srl(&["find", "is:young", "--dir", d]);
    assert!(
        stdout(&young).starts_with("x7k2m9.1  "),
        "{}",
        stdout(&young)
    );
    assert_eq!(
        srl(&["move", "x7k2m9", "x7k2m9.1", "--dir", d])
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
        "review", "k3m9q1.1", "k3m9q1.2", "--rating", "3", "--dir", d,
    ]);
    assert_eq!(reviewed.status.code(), Some(0), "{}", stderr(&reviewed));
    // Both clozes become c3: two orphans, and one new card k3m9q1.3.
    let text = read(&tmp, "japanese/grammar/wo.jsonl")
        .replace("{{c1::", "{{c3::")
        .replace("{{c2::", "{{c3::");
    write(&tmp, "japanese/grammar/wo.jsonl", &text);
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
fn excluded_files_and_dot_folders_are_not_read() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "templates/t.jsonl", "broken\n");
    write(&tmp, ".obsidian/x.jsonl", "broken\n");
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(2));
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\nexclude = [\"templates/**\"]\n",
    );
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(0));
}

#[test]
fn collections_lists_every_collection_without_reading_it() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "prose.md", "Study material, no collection.\n");
    write(&tmp, "broken.jsonl", "broken\n");
    write(&tmp, "plain.txt", "Not a collection.\n");
    write(&tmp, ".obsidian/x.jsonl", "Not a collection.\n");
    write(&tmp, "templates/t.jsonl", "Excluded.\n");
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\nexclude = [\"templates/**\"]\n",
    );
    let output = srl(&["collections", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        lines(&output),
        [
            "broken.jsonl",
            "japanese/grammar/wo.jsonl",
            "kanji/fish.jsonl"
        ]
    );
    let output = srl(&["collections", "--json", "--dir", d]);
    assert_eq!(
        json_lines(&output)[1],
        serde_json::json!({"collection": "japanese/grammar/wo.jsonl", "source": "items"})
    );
    let empty = store(&[]);
    assert_eq!(
        srl(&["collections", "--dir", dir(&empty)]).status.code(),
        Some(1)
    );
}

#[test]
fn kinds_describes_qa_without_reading_any_item() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "broken.jsonl", "broken\n");
    let output = srl(&["kinds", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let lines = lines(&output);
    assert!(lines[0].starts_with("qa  "), "{lines:?}");
    assert!(lines[1].starts_with("  Q (Question)  "), "{lines:?}");
    assert!(lines[2].starts_with("  A (Answer)  "), "{lines:?}");
    let output = srl(&["kinds", "--json", "--dir", d]);
    let kinds = json_lines(&output);
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0]["name"], "qa");
    assert_eq!(kinds[0]["fields"][0]["name"], "Q");
    assert_eq!(kinds[0]["fields"][0]["label"], "Question");
    assert_eq!(kinds[0]["fields"][1]["label"], "Answer");
}

const EXTRA: &str = "{\"id\": \"ext001\", \"Q\": \"From another folder?\", \"A\": \"Yes.\"}\n";

#[test]
fn only_jsonl_files_are_collections_by_default() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "plain.txt", "broken\n");
    write(&tmp, "page.html", "broken\n");
    write(&tmp, "data.json", "broken\n");
    let output = srl(&["find", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(!stdout(&output).contains("plain.txt"));
}

#[test]
fn a_file_include_matches_in_a_format_srl_doesnt_read_is_an_error() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "kanji/extra.txt", EXTRA);
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"**/*.jsonl\", \"**/*.txt\"]\n",
    );
    let output = srl(&["find", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("kanji/extra.txt: srl reads collections in the formats"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn include_replaces_the_default_rather_than_adding_to_it() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "cards/extra.jsonl", EXTRA);
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"cards/*.jsonl\"]\n",
    );
    let output = srl(&["find", "--json", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let cards = json_lines(&output);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0]["collection"], "cards/extra.jsonl");
    // in: drops the extension like any other.
    assert_eq!(lines(&srl(&["find", "in:extra", "--dir", d])).len(), 1);
}

#[test]
fn exclude_wins_over_include() {
    let tmp = japanese();
    let d = dir(&tmp);
    write(&tmp, "drafts/broken.jsonl", "broken\n");
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(2));
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"**/*.jsonl\"]\nexclude = [\"drafts/**\"]\n",
    );
    assert_eq!(srl(&["find", "--dir", d]).status.code(), Some(0));
}

#[test]
fn an_invalid_include_glob_is_a_configuration_error() {
    let tmp = japanese();
    let d = dir(&tmp);
    for (config, key) in [
        (
            "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"[\"]\n",
            "items.include",
        ),
        ("[context]\nexclude = [\"[\"]\n", "context.exclude"),
    ] {
        write(&tmp, ".srl/config.toml", config);
        let output = srl(&["find", "--dir", d]);
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains(key), "{}", stderr(&output));
        assert_eq!(output.stdout, b"");
    }
}

#[test]
fn unknown_drivers_and_keys_are_configuration_errors() {
    let tmp = japanese();
    let d = dir(&tmp);
    for (config, expected) in [
        (
            "[sources.items]\ndriver = \"org\"\n",
            "unknown driver `org`",
        ),
        (
            "[sources.items]\ndriver = \"filesystem\"\ndatabase = \"db.json\"\n",
            "database",
        ),
    ] {
        write(&tmp, ".srl/config.toml", config);
        let output = srl(&["find", "--dir", d]);
        assert_eq!(output.status.code(), Some(2), "{config}");
        assert!(
            stderr(&output).contains(expected),
            "{config}: {}",
            stderr(&output)
        );
    }
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
        "[sources.items]\ndriver = \"filesystem\"\nexclude = [\"drafts/**\"]\n",
    );
    let output = srl_with_input(&["add", "drafts/a.jsonl", "--stdin", "--dir", d], item);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("`exclude`"), "{}", stderr(&output));
    assert!(!tmp.path().join("drafts").exists());

    let output = srl_with_input(&["add", "new.jsonl", "--stdin", "--dir", d], item);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let id = stdout(&output).trim().to_owned();
    assert!(read(&tmp, "new.jsonl").starts_with(&format!("{{\"id\":\"{id}\"")));
    assert_eq!(lines(&srl(&["find", "in:new", "--dir", d])).len(), 1);

    // A path include matches is refused in a format srl doesn't write.
    let other = store(&[]);
    write(
        &other,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"cards/**\"]\n",
    );
    let output = srl_with_input(
        &["add", "cards/a.md", "--stdin", "--dir", dir(&other)],
        item,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("extension"), "{}", stderr(&output));
}

/// What every command that reads memories prints: standard output,
/// standard error and the exit code of each.
fn snapshot(d: &str) -> Vec<(String, String, Option<i32>)> {
    let commands: [&[&str]; 7] = [
        &["find"],
        &["find", "is:due"],
        &["find", "is:new", "-in:none"],
        &["next", "--json"],
        &["stats", "--json"],
        &["log", "--json"],
        &["log", "is:young"],
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
    assert!(built[4].1.contains("skipped line"), "{}", built[4].1);
    assert_eq!(snapshot(d), built);

    // Deleting the cache loses nothing, and the log's problems are still
    // reported.
    fs::remove_dir_all(&cache).unwrap();
    assert_eq!(snapshot(d), built);

    // Changes to the log show up as they would without a cache.
    srl(&["review", "k3m9q1.1", "--rating", "1", "--dir", d]);
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

    // A cache that can't be used is reported, and the log is read directly.
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
fn the_store_is_found_from_anywhere_under_it() {
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
/// `srl`, with `files`.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn store_with_kind(files: &[(&str, &str)]) -> TempDir {
    let program = Path::new(env!("CARGO_BIN_EXE_srl"))
        .with_file_name(format!("srl-test-kind{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.exists(),
        "{} isn't built; run `cargo test --workspace`",
        program.display()
    );
    let tmp = store(files);
    write(
        &tmp,
        ".srl/config.toml",
        &format!("[kinds]\nfake = ['{}']\n", program.to_str().unwrap()),
    );
    tmp
}

#[test]
fn cards_of_a_kind_program_in_their_own_groups_are_not_siblings() {
    let tmp = store_with_kind(&[(
        "a.jsonl",
        "{\"id\": \"abc\", \"kind\": \"fake\", \"GROUPS\": \"a|b c\"}\n",
    )]);
    let d = dir(&tmp);
    let first = next_card(d, &[]);
    assert_eq!(
        (&first["id"], &first["prompt"], &first["prompt_revealed"]),
        (&"abc.a".into(), &"a?".into(), &"a!".into())
    );
    assert_eq!(first["kind"], "fake");
    record(d, &first, 3);
    let second = next_card(d, &[]);
    assert_eq!(second["id"], "abc.b");
    record(d, &second, 3);
    // abc.c waits for tomorrow: its sibling abc.b was reviewed today.
    assert_eq!(srl(&["next", "--dir", d]).status.code(), Some(1));
}

#[test]
fn a_kind_program_gets_the_file_see_names_and_its_text() {
    let tmp = store_with_kind(&[
        ("a.md", "# A\n"),
        (
            "a.jsonl",
            "{\"id\": \"abc\", \"kind\": \"fake\", \"see\": \"a.md\", \"GROUPS\": \"a\"}\n",
        ),
    ]);
    let output = srl(&["cards", "abc.a", "--json", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let answer = json_lines(&output)[0]["answer"].clone();
    assert!(
        answer
            .as_str()
            .unwrap()
            .contains("context \"# A\\n\" (\"a.md\")"),
        "{answer}"
    );
}

#[test]
fn kind_selects_items_by_their_kind() {
    let tmp = store_with_kind(&[(
        "a.jsonl",
        "{\"id\": \"abc\", \"kind\": \"fake\", \"GROUPS\": \"a|b\"}\n{\"id\": \"def\", \"Q\": \"q\", \"A\": \"a\"}\n",
    )]);
    let d = dir(&tmp);
    let ids = |selector: &[&str]| -> Vec<String> {
        let output = srl(&[&["find", "--dir", d], selector].concat());
        lines(&output)
            .iter()
            .map(|line| line.split("  ").next().unwrap_or_default().to_owned())
            .collect()
    };
    assert_eq!(ids(&[]), ["abc.a", "abc.b", "def"]);
    assert_eq!(ids(&["kind:fake"]), ["abc.a", "abc.b"]);
    assert_eq!(ids(&["kind:qa"]), ["def"]);
    assert_eq!(ids(&["-kind:fake"]), ["def"]);
    let output = srl(&["find", "kind:nope", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("there is no kind `nope`"));
}

/// A store whose one card is made by a test kind program that starts a
/// process appending to `ticks` in the store's folder, then waits a minute.
fn store_with_ticking_kind() -> TempDir {
    store_with_kind(&[(
        "a.jsonl",
        "{\"id\": \"abc\", \"kind\": \"fake\", \"SPAWN_MAKE\": \"ticks\", \"SLEEP_MAKE\": \"60\"}\n",
    )])
}

/// Waits until `ticks` exists: the process the kind program starts is
/// running.
#[cfg(unix)]
fn wait_for_ticks(ticks: &Path) {
    let start = std::time::Instant::now();
    while !ticks.exists() {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "no process started"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Whether `ticks` is still growing: the process appending to it runs.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn still_ticking(ticks: &Path) -> bool {
    let before = fs::metadata(ticks).unwrap().len();
    std::thread::sleep(std::time::Duration::from_millis(300));
    fs::metadata(ticks).unwrap().len() != before
}

#[test]
fn a_kind_program_out_of_time_is_stopped_with_what_it_started() {
    let tmp = store_with_ticking_kind();
    let config = fs::read_to_string(tmp.path().join(".srl/config.toml")).unwrap();
    write(
        &tmp,
        ".srl/config.toml",
        &format!("{config}\n[kind_programs]\ncard_timeout = 1\n"),
    );
    let output = srl(&["next", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("took longer than 1 seconds"),
        "{}",
        stderr(&output)
    );
    assert!(!still_ticking(&tmp.path().join("ticks")));
}

#[cfg(unix)]
#[test]
fn srl_stopped_by_a_signal_stops_its_kind_program_with_what_it_started() {
    use rustix::process::{Pid, Signal, kill_process};
    use std::os::unix::process::ExitStatusExt as _;
    let tmp = store_with_ticking_kind();
    let ticks = tmp.path().join("ticks");
    let mut child = srl_command()
        .args(["next", "--dir", dir(&tmp)])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_ticks(&ticks);
    kill_process(Pid::from_child(&child), Signal::TERM).unwrap();
    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(Signal::TERM.as_raw()));
    assert!(!still_ticking(&ticks));
}

#[test]
fn next_fails_if_its_kind_program_fails_to_make_the_card() {
    let tmp = store_with_kind(&[(
        "a.jsonl",
        "{\"id\": \"abc\", \"kind\": \"fake\", \"GROUPS\": \"a|b\", \"FAIL_MAKE\": \"a\"}\n",
    )]);
    let d = dir(&tmp);
    // The same failure every time, and abc.b is never tried in its place.
    for _ in 0..2 {
        let output = srl(&["next", "--json", "--dir", d]);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert_eq!(output.stdout, b"");
        assert!(
            stderr(&output).contains("cannot make the card abc.a (fix what failed, or set the card aside with `srl suspend abc.a`): the program of kind fake couldn't make it: card a fails on purpose"),
            "{}",
            stderr(&output)
        );
    }
    // Suspending it is how to get past it.
    assert_eq!(
        srl(&["suspend", "id:abc.a", "--dir", d]).status.code(),
        Some(0)
    );
    let output = srl(&["next", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(lines(&output)[0], "abc.b");
}

#[test]
fn cards_fails_at_once_if_a_kind_program_fails() {
    let tmp = store_with_kind(&[(
        "a.jsonl",
        "{\"id\": \"abc\", \"kind\": \"fake\", \"GROUPS\": \"a|b\", \"FAIL_MAKE\": \"b\"}\n",
    )]);
    let d = dir(&tmp);
    let output = srl(&["cards", "abc", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert!(stderr(&output).contains("cannot make the card abc.b"));
    let output = srl(&["cards", "abc.a", "--json", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(json_lines(&output)[0]["prompt"], "a?");
}

#[test]
fn an_item_validate_rejects_is_reported_like_any_error_in_a_source() {
    let tmp = store_with_kind(&[(
        "a.jsonl",
        r#"{"id": "abc", "kind": "fake", "ERRORS": "{\"errors\": [{\"field\": \"VERB\", \"message\": \"is missing\"}]}"}
"#,
    )]);
    let output = srl(&["check", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("a.jsonl: item abc: VERB: is missing"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_kind_program_that_fails_is_reported_once_and_stops_commands() {
    let tmp = store_with_kind(&[(
        "a.jsonl",
        "{\"id\": \"abc\", \"kind\": \"fake\", \"BREAK_GROUPS\": \"on purpose\"}\n{\"id\": \"abd\", \"kind\": \"fake\", \"X\": \"y\"}\n",
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
        "a.jsonl",
        "{\"id\": \"abc\", \"kind\": \"fake\", \"GROUPS\": \"a b\", \"LOG\": \"calls.log\"}\n",
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
        &["add", "new.jsonl", "--stdin", "--dry-run", "--dir", d],
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

#[test]
fn edit_checks_every_new_item_at_once_and_names_the_line_at_fault() {
    let tmp = store_with_kind(&[(
        "a.jsonl",
        "{\"id\": \"aaa1\", \"kind\": \"fake\", \"X\": \"a\"}\n{\"id\": \"aaa2\", \"kind\": \"fake\", \"X\": \"b\"}\n{\"id\": \"aaa3\", \"kind\": \"fake\", \"X\": \"c\"}\n",
    )]);
    let d = dir(&tmp);
    let before = read(&tmp, "a.jsonl");
    let input = "{\"id\": \"aaa1\", \"kind\": \"fake\", \"X\": \"a\", \"LOG\": \"calls.log\"}\n\n{\"id\": \"aaa2\", \"kind\": \"fake\", \"GROUPS\": \"a b\", \"LOG\": \"calls.log\"}\n{\"id\": \"aaa3\", \"kind\": \"fake\", \"INVALID\": \"no good\"}\n";
    let output = srl_with_input(&["edit", "--stdin", "--dir", d], input);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("line 4 of standard input is not a valid item: no good"),
        "{}",
        stderr(&output)
    );
    assert_eq!(read(&tmp, "calls.log"), "validate 3\ngroups 2\n");
    assert_eq!(read(&tmp, "a.jsonl"), before);
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
        let tmp = store(&[(
            "n.jsonl",
            "{\"Q\": \"a\", \"A\": \"b\"}\n{\"Q\": \"c\", \"A\": \"d\"}\n",
        )]);
        let output = srl_command()
            .args(["id", "--dir", dir(&tmp)])
            .env("SRL_SEED", "7")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        read(&tmp, "n.jsonl")
    };
    let first = ids();
    assert!(first.contains("\"id\""), "{first}");
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

#[test]
fn sources_read_their_collections_each_with_its_driver() {
    let tmp = store(&[
        ("japanese/wo.md", "Prose.\n"),
        (
            "japanese/wo.jsonl",
            "{\"id\": \"aaa1\", \"see\": \"japanese/wo.md\", \"Q\": \"q\", \"A\": \"a\"}\n",
        ),
        (
            "vocab/words.jsonl",
            "{\"id\": \"bbb1\", \"see\": \"japanese/wo.md\", \"Q\": \"q\", \"A\": \"a\"}\n",
        ),
    ]);
    let d = dir(&tmp);
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\ninclude = [\"japanese/**/*.jsonl\"]\n\n[sources.vocab]\ndriver = \"filesystem\"\ninclude = [\"vocab/**/*.jsonl\"]\n",
    );
    let collections = srl(&["collections", "--json", "--dir", d]);
    assert_eq!(
        json_lines(&collections),
        [
            serde_json::json!({"collection": "japanese/wo.jsonl", "source": "items"}),
            serde_json::json!({"collection": "vocab/words.jsonl", "source": "vocab"}),
        ]
    );
    let found = srl(&["find", "--dir", d]);
    assert_eq!(found.status.code(), Some(0), "{}", stderr(&found));
    assert!(lines(&found)[1].starts_with("bbb1  vocab/words.jsonl  "));
    // Study material is the same for every source.
    let shown = srl(&["show", "bbb1", "--json", "--dir", d]);
    let shown = &json_lines(&shown)[0];
    assert_eq!(shown["source"], "vocab");
    assert_eq!(shown["context"], "Prose.\n");
    let item = "{\"Q\": \"r\", \"A\": \"b\"}";
    let added = srl_with_input(&["add", "vocab/new.jsonl", "--stdin", "--dir", d], item);
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    assert!(read(&tmp, "vocab/new.jsonl").contains("\"Q\":\"r\""));
    let found = srl(&["find", "source:vocab", "--dir", d]);
    assert_eq!(lines(&found).len(), 2, "{}", stderr(&found));
    let sources = srl(&["sources", "--json", "--dir", d]);
    let sources = json_lines(&sources);
    assert_eq!(sources[1]["name"], "vocab");
    assert_eq!(sources[1]["driver"], "filesystem");
    assert_eq!(
        sources[1]["creates"]["include"],
        serde_json::json!(["vocab/**/*.jsonl"])
    );
}

#[test]
fn a_collection_two_sources_give_is_an_error() {
    let tmp = store(&[(
        "a.jsonl",
        "{\"id\": \"aaa1\", \"Q\": \"q\", \"A\": \"a\"}\n",
    )]);
    let d = dir(&tmp);
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\n\n[sources.again]\ndriver = \"filesystem\"\ninclude = [\"a.jsonl\"]\n",
    );
    let output = srl(&["find", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("a.jsonl: the sources again and items both give it"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn add_creates_a_collection_in_the_one_source_that_accepts_it() {
    let tmp = store(&[]);
    let d = dir(&tmp);
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\nexclude = [\"data/**\"]\n\n[sources.data]\ndriver = \"filesystem\"\ninclude = [\"data/**/*.jsonl\"]\n",
    );
    let item = "{\"Q\": \"q\", \"A\": \"a\"}";
    let add = |collection: &str| srl_with_input(&["add", collection, "--stdin", "--dir", d], item);
    assert_eq!(add("a.jsonl").status.code(), Some(0));
    assert_eq!(add("data/b.jsonl").status.code(), Some(0));
    assert!(read(&tmp, "data/b.jsonl").starts_with("{\"id\""));
    let found = srl(&["find", "source:data", "--json", "--dir", d]);
    assert_eq!(json_lines(&found)[0]["collection"], "data/b.jsonl");
    let refused = add("c.txt");
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        stderr(&refused).contains("not matched by `include`"),
        "{}",
        stderr(&refused)
    );
    write(
        &tmp,
        ".srl/config.toml",
        "[sources.items]\ndriver = \"filesystem\"\n\n[sources.more]\ndriver = \"filesystem\"\ninclude = [\"more/**/*.jsonl\"]\n",
    );
    let both = add("more/d.jsonl");
    assert_eq!(both.status.code(), Some(2));
    assert!(
        stderr(&both).contains("more than one source"),
        "{}",
        stderr(&both)
    );
}

/// A store with `files`, whose one source `vocab` is the test source
/// program of `crates/srl-test-source`, keeping its items in `vocab.json`
/// (holding `data`), logging its calls in `calls.log`, and with the
/// settings `more`.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn store_with_program(files: &[(&str, &str)], data: &str, more: &str) -> TempDir {
    let program = Path::new(env!("CARGO_BIN_EXE_srl"))
        .with_file_name(format!("srl-test-source{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.exists(),
        "{} isn't built; run `cargo test --workspace`",
        program.display()
    );
    let tmp = store(files);
    write(&tmp, "vocab.json", data);
    write(
        &tmp,
        ".srl/config.toml",
        &format!(
            "[sources.vocab]\nprogram = ['{}']\ndata = \"vocab.json\"\nlog = \"calls.log\"\n{more}",
            program.to_str().unwrap()
        ),
    );
    tmp
}

/// The data of the test source: a collection `vocab/words` with an item
/// without an ID that names `lesson.md`, and a draft.
const VOCAB: &str = r#"{"next": 3, "collections": {"vocab/words": {"items": [
  {"handle": "h1", "item": {"see": "lesson.md", "Q": "犬", "A": "dog"}},
  {"handle": "h2", "item": {"id": "cat001", "draft": true, "Q": "猫", "A": "cat"}}]}}}"#;

/// Valid data of the test source: a collection `vocab/words` with two
/// items.
const WORDS: &str = r#"{"next": 3, "collections": {"vocab/words": {"items": [
  {"handle": "h1", "item": {"id": "dog001", "Q": "犬", "A": "dog"}},
  {"handle": "h2", "item": {"id": "cat001", "Q": "猫", "A": "cat"}}]}}}"#;

/// The calls the test source got, as `CALL LINES`, and forgets them.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn calls(tmp: &TempDir) -> Vec<String> {
    let path = tmp.path().join("calls.log");
    let calls = fs::read_to_string(&path).unwrap_or_default();
    if path.exists() {
        fs::remove_file(path).unwrap();
    }
    calls.lines().map(str::to_owned).collect()
}

#[test]
fn a_source_program_s_items_are_given_ids_studied_changed_and_shown() {
    let tmp = store_with_program(
        &[("lesson.md", "# Lesson\n\nAnimals.\n")],
        VOCAB,
        "creates = [\"vocab/*\"]\n",
    );
    let d = dir(&tmp);
    let ids = srl(&["id", "--dir", d]);
    assert_eq!(ids.status.code(), Some(0), "{}", stderr(&ids));
    let given = lines(&ids);
    assert_eq!(given.len(), 1);
    assert!(given[0].ends_with("  vocab/words"), "{given:?}");
    let found = srl(&["find", "source:vocab", "--json", "--dir", d]);
    let found = json_lines(&found);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0]["collection"], "vocab/words");
    assert!(found[0].get("file").is_none() && found[0].get("line").is_none());
    // An item of the program names study material, as any item does.
    let dog = &json_lines(&srl(&["find", "犬", "--json", "--dir", d]))[0]["item"];
    let dog = dog.as_str().unwrap().to_owned();
    let shown = &json_lines(&srl(&["show", &dog, "--json", "--dir", d]))[0];
    assert_eq!(shown["context"], "# Lesson\n\nAnimals.\n");
    // Without a text of its own, an item is shown as its item object.
    let plain = srl(&["show", "cat001", "--no-context", "--dir", d]);
    assert_eq!(
        stdout(&plain),
        "vocab/words\n{\"id\":\"cat001\",\"draft\":true,\"Q\":\"猫\",\"A\":\"cat\"}\n"
    );
    calls(&tmp);
    let accepted = srl(&["accept", "id:cat001", "--dir", d]);
    assert_eq!(accepted.status.code(), Some(0), "{}", stderr(&accepted));
    // One read and one collections to load, then a read, a write and a
    // read to verify it.
    assert_eq!(
        calls(&tmp),
        ["collections 1", "read 1", "read 1", "write 1", "read 1"]
    );
    let edited = srl_with_input(
        &["edit", "--stdin", "--dir", d],
        "{\"id\": \"cat001\", \"Q\": \"猫\", \"A\": \"a cat\"}",
    );
    assert_eq!(edited.status.code(), Some(0), "{}", stderr(&edited));
    let added = srl_with_input(
        &["add", "vocab/birds", "--stdin", "--dir", d],
        "{\"Q\": \"鳥\", \"A\": \"bird\"}",
    );
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let data = read(&tmp, "vocab.json");
    assert!(
        data.contains("a cat") && data.contains("vocab/birds") && !data.contains("draft"),
        "{data}"
    );
    assert!(srl(&["check", "--dir", d]).status.success());
}

#[test]
fn a_command_calls_a_source_program_a_fixed_number_of_times() {
    let tmp = store_with_program(&[], WORDS, "");
    let d = dir(&tmp);
    assert!(srl(&["find", "--dir", d]).status.success());
    assert_eq!(calls(&tmp), ["collections 1", "read 1"]);
    // Context is never asked of a source.
    assert!(srl(&["next", "--json", "--dir", d]).status.success());
    assert_eq!(calls(&tmp), ["collections 1", "read 1"]);
    assert!(
        srl(&["show", "cat001", "--json", "--dir", d])
            .status
            .success()
    );
    assert_eq!(calls(&tmp), ["collections 1", "read 1"]);
    assert!(srl(&["sources", "--dir", d]).status.success());
    assert_eq!(calls(&tmp), ["describe 1"]);
}

#[test]
fn a_source_program_that_fails_or_breaks_the_protocol_stops_commands() {
    for (more, expected) in [
        (
            "fail = \"read\"\n",
            "the source vocab failed: its call `read` failed: broken",
        ),
        (
            "raw_call = \"read\"\nraw_output = \"not json\\n\"\n",
            "its call `read` printed invalid output",
        ),
        (
            "raw_call = \"read\"\nraw_output = '{\"collection\": \"words\", \"handle\": \"a\", \"item\": {\"Q\": \"q\"}, \"extra\": 1}'\n",
            "the key `extra`",
        ),
        (
            "raw_call = \"read\"\nraw_output = '{\"collection\": \"words\", \"handle\": \"a\", \"line\": null, \"item\": {\"Q\": \"q\"}}'\n",
            "the key `line`",
        ),
        (
            "raw_call = \"collections\"\nraw_output = '{\"collection\": \"words\", \"file\": null}'\n",
            "the key `file`",
        ),
        (
            "raw_call = \"read\"\nraw_output = '{\"collection\": \"../x\", \"handle\": \"a\", \"item\": {\"Q\": \"q\"}}'\n",
            "is not a collection's name",
        ),
    ] {
        let tmp = store_with_program(&[], WORDS, more);
        let output = srl(&["find", "--dir", dir(&tmp)]);
        assert_eq!(output.status.code(), Some(2), "{more}");
        assert!(
            stderr(&output).contains(expected),
            "{more}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn a_change_a_source_program_refuses_or_doesn_t_make_fails_the_command() {
    let tmp = store_with_program(&[], WORDS, "refuse = \"the database is read-only\"\n");
    let d = dir(&tmp);
    let added = srl_with_input(
        &["add", "vocab/words", "--stdin", "--dir", d],
        "{\"Q\": \"q\", \"A\": \"a\"}",
    );
    assert_eq!(added.status.code(), Some(2));
    assert!(
        stderr(&added)
            .contains("line 1 of standard input is not a valid item: the database is read-only"),
        "{}",
        stderr(&added)
    );
    let tmp = store_with_program(&[], WORDS, "lie = true\n");
    let d = dir(&tmp);
    let added = srl_with_input(
        &["add", "vocab/words", "--stdin", "--dir", d],
        "{\"Q\": \"q\", \"A\": \"a\"}",
    );
    assert_eq!(added.status.code(), Some(2));
    assert!(
        stderr(&added).contains("didn't make exactly the changes"),
        "{}",
        stderr(&added)
    );
    // A collection the program doesn't create is refused before it is asked
    // to write.
    let tmp = store_with_program(&[], WORDS, "creates = [\"vocab/words\"]\n");
    let d = dir(&tmp);
    let added = srl_with_input(
        &["add", "vocab/fish", "--stdin", "--dir", d],
        "{\"Q\": \"q\", \"A\": \"a\"}",
    );
    assert_eq!(added.status.code(), Some(2));
    assert!(
        stderr(&added).contains("creates no collection named vocab/fish"),
        "{}",
        stderr(&added)
    );
}

/// A web service on `127.0.0.1` for the test source's `url`: it keeps a
/// store's items as one resource, `/data`, and gives it with an `ETag`
/// (304 when the `If-None-Match` is current), replaces it on `PUT` (412
/// when the `If-Match` isn't current), and refuses requests without the
/// bearer token `secret` with 401. Each request is logged as its method
/// and the status it got.
struct Service {
    url: String,
    data: std::sync::Arc<std::sync::Mutex<(u64, String)>>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Service {
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn start(data: &str) -> Self {
        use std::io::{BufRead as _, BufReader, Read as _};
        use std::sync::{Arc, Mutex};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new((1_u64, data.to_owned())));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (shared, log) = (Arc::clone(&state), Arc::clone(&requests));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = Vec::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    head.push(line.trim_end().to_owned());
                }
                let header = |name: &str| {
                    head.iter()
                        .filter_map(|line| line.split_once(": "))
                        .find(|(key, _)| key.eq_ignore_ascii_case(name))
                        .map(|(_, value)| value.to_owned())
                };
                let length = header("Content-Length").map_or(0, |length| length.parse().unwrap());
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let method = head.first().unwrap().split(' ').next().unwrap().to_owned();
                let mut data = shared.lock().unwrap();
                let etag = format!("\"v{}\"", data.0);
                let (status, answer) =
                    if header("Authorization").as_deref() != Some("Bearer secret") {
                        (401, "a bearer token is needed".to_owned())
                    } else if method == "GET"
                        && header("If-None-Match").as_deref() == Some(etag.as_str())
                    {
                        (304, String::new())
                    } else if method == "GET" {
                        (200, data.1.clone())
                    } else if header("If-Match").as_deref() != Some(etag.as_str()) {
                        (412, "it changed".to_owned())
                    } else {
                        *data = (data.0 + 1, String::from_utf8(body).unwrap());
                        (200, String::new())
                    };
                let etag = format!("\"v{}\"", data.0);
                log.lock().unwrap().push(format!("{method} {status}"));
                write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nETag: {etag}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                    answer.len()
                )
                .unwrap();
            }
        });
        Self {
            url,
            data: state,
            requests,
        }
    }

    /// The requests since the last call, as `METHOD STATUS`.
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn requests(&self) -> Vec<String> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }

    /// The items the service keeps.
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn data(&self) -> String {
        self.data.lock().unwrap().1.clone()
    }
}

#[test]
fn a_source_program_of_a_web_service_checks_its_settings_with_it_and_caches_its_answers() {
    let service = Service::start(WORDS);
    let more = format!(
        "url = \"{}\"\ntoken = \"secret\"\ncache = \"service-cache.json\"\n",
        service.url
    );
    let tmp = store_with_program(&[], "", &more);
    let d = dir(&tmp);
    // Two calls, each a request; the second is answered from the cache.
    let found = srl(&["find", "--dir", d]);
    assert_eq!(found.status.code(), Some(0), "{}", stderr(&found));
    assert_eq!(lines(&found).len(), 2);
    assert_eq!(service.requests(), ["GET 200", "GET 304"]);
    assert!(srl(&["find", "--dir", d]).status.success());
    assert_eq!(service.requests(), ["GET 304", "GET 304"]);
    // A change: read, written with the version read, and read again.
    let edited = srl_with_input(
        &["edit", "--stdin", "--dir", d],
        "{\"id\": \"cat001\", \"Q\": \"猫\", \"A\": \"a cat\"}",
    );
    assert_eq!(edited.status.code(), Some(0), "{}", stderr(&edited));
    assert_eq!(
        service.requests(),
        [
            "GET 304", "GET 304", "GET 304", "GET 304", "PUT 200", "GET 200"
        ]
    );
    assert!(service.data().contains("a cat"));
    // The settings are the program's to check, here by the service.
    let more = format!("url = \"{}\"\ntoken = \"wrong\"\n", service.url);
    let tmp = store_with_program(&[], "", &more);
    let output = srl(&["find", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("the service answered 401: a bearer token is needed"),
        "{}",
        stderr(&output)
    );
}
