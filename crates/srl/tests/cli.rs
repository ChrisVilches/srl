//! Runs the real `srl` binary and checks exit codes and output streams.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn srl(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_srl"))
        .args(args)
        .env_remove("SRL_DIR")
        .output()
        .unwrap()
}

/// Runs `srl` with `input` on standard input.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn srl_with_input(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_srl"))
        .args(args)
        .env_remove("SRL_DIR")
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

#[test]
fn help_succeeds_and_lists_commands() {
    let output = srl(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
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
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("srl-no-such-command-for-srl-tests"));
    assert!(output.stdout.is_empty());
}

/// A temporary directory with a fresh store in it.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn store() -> TempDir {
    let tmp = TempDir::new().unwrap();
    assert_eq!(srl(&["init", dir(&tmp)]).status.code(), Some(0));
    tmp
}

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn dir(tmp: &TempDir) -> &str {
    tmp.path().to_str().unwrap()
}

#[test]
fn commands_refuse_a_directory_that_is_not_a_store() {
    let tmp = TempDir::new().unwrap();
    let output = srl(&["find", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("srl init"));
}

#[test]
fn init_prints_what_it_creates_and_refuses_to_run_twice() {
    let tmp = TempDir::new().unwrap();
    let output = srl(&["init", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("srl.toml"));
    assert!(stdout.contains("atoms"));
    assert_eq!(srl(&["init", dir(&tmp)]).status.code(), Some(2));
}

#[test]
fn init_dry_run_creates_nothing() {
    let tmp = TempDir::new().unwrap();
    let output = srl(&["init", "--dry-run", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0));
    assert!(!tmp.path().join("srl.toml").exists());
}

#[test]
fn added_atoms_can_be_shown() {
    let tmp = store();
    let added = srl(&[
        "add",
        "--dir",
        dir(&tmp),
        "--title",
        "鮪",
        "--answer",
        "まぐろ — tuna",
        "--tag",
        "kanji/food",
    ]);
    assert_eq!(added.status.code(), Some(0));
    let id = stdout(&added).trim().to_owned();
    assert_eq!(id.len(), 6);
    let path = tmp
        .path()
        .join("atoms")
        .join("kanji")
        .join("food")
        .join("鮪.md");
    let file = fs::read_to_string(path).unwrap();
    let shown = srl(&["show", "--dir", dir(&tmp), &id]);
    assert_eq!(shown.status.code(), Some(0));
    assert_eq!(stdout(&shown), file);
    assert!(file.contains("tags: [kanji/food]"));
}

#[test]
fn shown_json_can_be_added_again() {
    let tmp = store();
    let line = r#"{"title": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"], "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred.\n"}"#;
    let added = srl_with_input(&["add", "--stdin", "--dir", dir(&tmp)], line);
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let id = stdout(&added).trim().to_owned();
    let shown = srl(&["show", "--json", "--dir", dir(&tmp), &id]);
    let mut json: serde_json::Value = serde_json::from_str(&stdout(&shown)).unwrap();
    let object = json.as_object_mut().unwrap();
    object.remove("id");
    object.remove("path");
    let again = srl_with_input(
        &["add", "--stdin", "--dir", dir(&tmp)],
        &serde_json::to_string(&json).unwrap(),
    );
    assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
    let second = stdout(&again).trim().to_owned();
    let reshown = srl(&["show", "--json", "--dir", dir(&tmp), &second]);
    let mut rejson: serde_json::Value = serde_json::from_str(&stdout(&reshown)).unwrap();
    let reobject = rejson.as_object_mut().unwrap();
    reobject.remove("id");
    reobject.remove("path");
    assert_eq!(rejson, json);
}

#[test]
fn one_bad_stdin_line_adds_nothing() {
    let tmp = store();
    let output = srl_with_input(
        &["add", "--stdin", "--dir", dir(&tmp)],
        "{\"title\": \"fine\"}\n{\"title\": \"bad\", \"status\": \"paused\"}\n",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("line 2"));
    assert!(
        fs::read_dir(tmp.path().join("atoms"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn add_dry_run_writes_nothing() {
    let tmp = store();
    let output = srl(&["add", "--dry-run", "--dir", dir(&tmp), "--title", "x"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("atoms/x.md"));
    assert!(
        fs::read_dir(tmp.path().join("atoms"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn show_prints_what_it_finds_and_fails_on_unknown_ids() {
    let tmp = store();
    let id = stdout(&srl(&["add", "--dir", dir(&tmp), "--title", "x"]))
        .trim()
        .to_owned();
    let output = srl(&["show", "--dir", dir(&tmp), &id, "zzzzzz"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).contains("title: x"));
    assert!(stderr(&output).contains("zzzzzz"));
}

#[test]
fn broken_atom_files_are_reported_and_skipped() {
    let tmp = store();
    let id = stdout(&srl(&["add", "--dir", dir(&tmp), "--title", "x"]))
        .trim()
        .to_owned();
    fs::write(
        tmp.path().join("atoms").join("broken.md"),
        "no front-matter\n",
    )
    .unwrap();
    let output = srl(&["show", "--dir", dir(&tmp), &id]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("title: x"));
    assert!(stderr(&output).contains("atoms/broken.md"));
}

/// A store with three atoms: N1 grammar, N5 grammar and a draft food kanji.
fn japanese() -> TempDir {
    let tmp = store();
    let lines = [
        r#"{"title": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"]}"#,
        r#"{"title": "です", "tags": ["japanese/grammar", "jlpt/n5"]}"#,
    ];
    let added = srl_with_input(&["add", "--stdin", "--dir", dir(&tmp)], &lines.join("\n"));
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let draft = srl(&[
        "add",
        "--draft",
        "--dir",
        dir(&tmp),
        "--title",
        "鮪",
        "--tag",
        "kanji/food",
    ]);
    assert_eq!(draft.status.code(), Some(0), "{}", stderr(&draft));
    tmp
}

#[test]
fn find_takes_negated_terms_and_lists_atoms() {
    let tmp = japanese();
    let output = srl(&["find", "tag:japanese", "-tag:jlpt/n5", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let stdout = stdout(&output);
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.contains("〜をものともせず  japanese/grammar jlpt/n1"));
}

#[test]
fn find_exits_1_when_nothing_matches() {
    let tmp = japanese();
    let output = srl(&["find", "tag:nothing", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn find_json_prints_one_atom_per_line() {
    let tmp = japanese();
    let output = srl(&["find", "--json", "--dir", dir(&tmp)]);
    let lines = stdout(&output)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|atom| atom["id"].is_string()));
}

#[test]
fn bad_selectors_exit_with_2_and_explain() {
    let tmp = japanese();
    let output = srl(&["find", "tga:x", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("tga:"));
}

#[test]
fn drafts_lists_only_drafts() {
    let tmp = japanese();
    let output = srl(&["drafts", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout(&output);
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.contains("鮪"));
    let none = srl(&["drafts", "tag:japanese", "--dir", dir(&tmp)]);
    assert_eq!(none.status.code(), Some(1));
}

#[test]
fn tags_counts_every_tag() {
    let tmp = japanese();
    let output = srl(&["tags", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        "2  japanese/grammar\n1  jlpt/n1\n1  jlpt/n5\n1  kanji/food\n"
    );
    let json = srl(&["tags", "--json", "--dir", dir(&tmp)]);
    assert!(stdout(&json).starts_with("{\"tag\":\"japanese/grammar\",\"count\":2}\n"));
    let empty = store();
    assert_eq!(srl(&["tags", "--dir", dir(&empty)]).status.code(), Some(1));
}

/// The IDs printed by a command, one per line.
fn ids(output: &Output) -> Vec<String> {
    stdout(output).lines().map(str::to_owned).collect()
}

#[test]
fn drafts_move_through_accept_suspend_and_unsuspend() {
    let tmp = store();
    let d = dir(&tmp);
    // A file written by hand, straight into atoms/, with a comment and CRLF.
    let fish = "a7f3k2";
    let stored = tmp.path().join("atoms").join("fish.md");
    let text = format!(
        "---\r\nid: {fish}\r\ntitle: 鮪  # the fish\r\nstatus: draft # check it\r\ntags: [kanji/food]\r\n---\r\nまぐろ\r\n"
    );
    fs::write(&stored, &text).unwrap();
    let added = srl(&[
        "add", "--draft", "--dir", d, "--title", "です", "--tag", "jlpt/n5",
    ]);
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let desu = stdout(&added).trim().to_owned();
    assert_eq!(ids(&srl(&["drafts", "--dir", d])).len(), 2);

    let dry = srl(&["accept", "tag:kanji", "--dry-run", "--dir", d]);
    assert_eq!(ids(&dry), [fish]);
    assert_eq!(ids(&srl(&["drafts", "--dir", d])).len(), 2);

    let accepted = srl(&["accept", "tag:kanji", "--dir", d]);
    assert_eq!(accepted.status.code(), Some(0), "{}", stderr(&accepted));
    assert_eq!(ids(&accepted), [fish]);
    let again = srl(&["accept", "tag:kanji", "--dir", d]);
    assert_eq!(again.status.code(), Some(1));
    assert!(again.stdout.is_empty());

    let suspended = srl(&["suspend", "--dir", d]);
    assert_eq!(ids(&suspended), [fish]);
    let found = srl(&["find", "status:suspended", "--dir", d]);
    assert!(stdout(&found).starts_with(fish));
    assert_eq!(ids(&srl(&["unsuspend", "-tag:jlpt", "--dir", d])), [fish]);
    assert_eq!(srl(&["unsuspend", "--dir", d]).status.code(), Some(1));

    assert_eq!(
        fs::read_to_string(stored).unwrap(),
        text.replace("status: draft", "status: active")
    );
    let drafts = srl(&["drafts", "--dir", d]);
    assert!(stdout(&drafts).starts_with(&desu));
}

#[test]
fn edit_stdin_replaces_the_file_in_place() {
    let tmp = store();
    let d = dir(&tmp);
    let id = stdout(&srl(&["add", "--dir", d, "--title", "x", "--tag", "old"]))
        .trim()
        .to_owned();
    let text = format!("---\nid: {id}\ntitle: x, better\ntags: [new]\n---\nAn explanation.\n");
    let output = srl_with_input(&["edit", &id, "--stdin", "--dir", d], &text);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(tmp.path().join("atoms").join("old").join("x.md")).unwrap(),
        text
    );
    assert_eq!(stdout(&srl(&["show", &id, "--dir", d])), text);

    let renamed = text.replace(&format!("id: {id}"), "id: zzzzzz");
    let refused = srl_with_input(&["edit", &id, "--stdin", "--dir", d], &renamed);
    assert_eq!(refused.status.code(), Some(2));
    assert!(stderr(&refused).contains(&format!("keep `id: {id}`")));

    let unknown = srl_with_input(&["edit", "zzzzzz", "--stdin", "--dir", d], &renamed);
    assert_eq!(unknown.status.code(), Some(2));

    let dry = srl_with_input(
        &["edit", &id, "--stdin", "--dry-run", "--dir", d],
        "---\n---\n",
    );
    assert_eq!(dry.status.code(), Some(2));
    let fine = text.replace("better", "best");
    let dry = srl_with_input(&["edit", &id, "--stdin", "--dry-run", "--dir", d], &fine);
    assert_eq!(dry.status.code(), Some(0));
    assert_eq!(stdout(&srl(&["show", &id, "--dir", d])), text);
}

/// Writes a drill program `srl-drill-NAME` into `dir` and returns its path. With `--help` it
/// prints `help`. Otherwise it saves its input as `srl-drill-NAME.input` next
/// to itself, then sleeps for `seconds` if that is above 0, and prints
/// `lines` if not. Lines must not hold characters that `cmd.exe` treats
/// specially (`& | < > ^ %`).
#[cfg(unix)]
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn drill_program(dir: &Path, name: &str, help: &str, seconds: u32, lines: &[&str]) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let quote = |text: &str| format!("'{}'", text.replace('\'', r"'\''"));
    // `exec`, so that killing the drill kills the sleep, and nothing is left
    // holding standard error open.
    let rest = if seconds > 0 {
        format!("exec sleep {seconds}\n")
    } else {
        let quoted = lines.iter().map(|line| quote(line)).collect::<Vec<_>>();
        format!("printf '%s\\n' {}\n", quoted.join(" "))
    };
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = --help ]; then printf '%s\\n' {}; exit 0; fi\ncat > \"$0.input\"\n{rest}",
        quote(help),
    );
    let path = dir.join(format!("srl-drill-{name}"));
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(windows)]
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn drill_program(dir: &Path, name: &str, help: &str, seconds: u32, lines: &[&str]) -> PathBuf {
    let rest = if seconds > 0 {
        format!("ping -n {} 127.0.0.1 > nul 2> nul\r\n", seconds + 1)
    } else {
        lines
            .iter()
            .map(|line| format!("echo {line}\r\n"))
            .collect::<Vec<_>>()
            .concat()
    };
    let script = format!(
        "@echo off\r\nif \"%~1\"==\"--help\" (\r\n  echo {help}\r\n  exit /b 0\r\n)\r\nfindstr \"^\" > \"%~dpn0.input\"\r\n{rest}"
    );
    let path = dir.join(format!("srl-drill-{name}.cmd"));
    fs::write(&path, script).unwrap();
    path
}

/// A store with a `drills/` folder, and that folder.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn store_with_drills() -> (TempDir, PathBuf) {
    let tmp = store();
    let drills = tmp.path().join("drills");
    fs::create_dir(&drills).unwrap();
    (tmp, drills)
}

#[test]
fn drills_lists_built_in_drills_and_the_stores_programs() {
    let (tmp, drills) = store_with_drills();
    drill_program(&drills, "conj", "# Conjugation cards", 0, &[]);
    drill_program(&drills, "cloze", "My own cloze", 0, &[]);
    let output = srl(&["drills", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let listed = stdout(&output);
    let lines = listed.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 6, "{listed}");
    assert!(lines[0].starts_with("cards  built-in  "));
    assert!(lines[1].starts_with("cloze  ") && lines[1].ends_with("  My own cloze"));
    assert!(lines[3].starts_with("conj  ") && lines[3].ends_with("  Conjugation cards"));
    assert!(lines[3].contains("srl-drill-conj"));
    let json = srl(&["drills", "--json", "--dir", dir(&tmp)]);
    let reveal = stdout(&json)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|drill| drill["name"] == "reveal")
        .unwrap();
    assert!(reveal["path"].is_null());
}

#[test]
fn cards_runs_every_drill_that_applies_and_drops_invalid_cards() {
    let (tmp, drills) = store_with_drills();
    drill_program(
        &drills,
        "conj",
        "Conjugation",
        0,
        &[
            r#"{"ref": "te", "prompt": "te-form?", "answer": "tabete", "grading": "exact", "accept": [["tabete"]]}"#,
            r#"{"ref": "past", "prompt": "no grading", "answer": "tabeta"}"#,
        ],
    );
    let d = dir(&tmp);
    let line = r#"{"title": "taberu", "answer": "to eat", "conj": {"verb": "taberu"}, "cloze": ["I {{eat}} bread."], "body": "A verb."}"#;
    let id = stdout(&srl_with_input(&["add", "--stdin", "--dir", d], line))
        .trim()
        .to_owned();
    let output = srl(&["cards", &id, "--json", "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    let refs = stdout(&output)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["ref"].clone())
        .collect::<Vec<_>>();
    assert_eq!(refs, ["reveal", "cloze:0:1", "conj:te"]);
    assert!(
        stderr(&output).contains("no `grading`"),
        "{}",
        stderr(&output)
    );

    let input = fs::read_to_string(drills.join("srl-drill-conj.input")).unwrap();
    let input = serde_json::from_str::<serde_json::Value>(&input).unwrap();
    assert_eq!(input["id"], id.as_str());
    assert_eq!(input["path"], "atoms/taberu.md");
    assert_eq!(input["frontmatter"]["conj"]["verb"], "taberu");
    assert_eq!(input["body"], "A verb.\n");
    assert!(input["day"].as_str().unwrap().len() == "2026-10-26".len());

    let text = srl(&["cards", &id, "--dir", d]);
    assert!(stdout(&text).starts_with("reveal  self\ntaberu\n---\nto eat\n\ncloze:0:1  exact\n"));
}

#[test]
fn a_drill_that_takes_too_long_is_skipped_and_reported() {
    let (tmp, drills) = store_with_drills();
    fs::write(
        tmp.path().join("srl.toml"),
        "[drills]\ntimeout_seconds = 1\n",
    )
    .unwrap();
    drill_program(&drills, "slow", "Slow", 5, &[]);
    let d = dir(&tmp);
    let id = stdout(&srl_with_input(
        &["add", "--stdin", "--dir", d],
        r#"{"title": "x", "slow": 1}"#,
    ))
    .trim()
    .to_owned();
    let output = srl(&["cards", &id, "--dir", d]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).starts_with("reveal  self\n"));
    assert!(
        stderr(&output).contains("longer than 1 seconds"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn check_reports_problems_and_fix_assigns_missing_ids() {
    let tmp = store();
    let d = dir(&tmp);
    let clean = srl(&["check", "--dir", d]);
    assert_eq!(clean.status.code(), Some(0), "{}", stderr(&clean));
    assert!(clean.stdout.is_empty() && clean.stderr.is_empty());

    let hand = tmp.path().join("atoms").join("hand.md");
    fs::write(&hand, "---\ntitle: written by hand  # a comment\n---\n").unwrap();
    let missing = srl(&["check", "--dir", d]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("srl check --fix"));

    let dry = srl(&["check", "--fix", "--dry-run", "--dir", d]);
    assert!(stdout(&dry).ends_with("  atoms/hand.md\n"));
    assert!(!fs::read_to_string(&hand).unwrap().contains("id:"));

    let fixed = srl(&["check", "--fix", "--dir", d]);
    assert_eq!(fixed.status.code(), Some(0), "{}", stderr(&fixed));
    let id = stdout(&fixed).split("  ").next().unwrap().to_owned();
    assert_eq!(
        fs::read_to_string(&hand).unwrap(),
        format!("---\nid: {id}\ntitle: written by hand  # a comment\n---\n")
    );

    srl_with_input(
        &["add", "--stdin", "--dir", d],
        r#"{"title": "y", "conj": {"verb": "x"}}"#,
    );
    let no_drill = srl(&["check", "--dir", d]);
    assert_eq!(no_drill.status.code(), Some(2));
    assert!(stderr(&no_drill).contains("no drill `conj`"));
}

/// The JSON values printed by a command, one per line.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn json_lines(output: &Output) -> Vec<serde_json::Value> {
    stdout(output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Runs `srl next --json` and returns the card.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn next_card(d: &str, extra: &[&str]) -> serde_json::Value {
    let args = [&["next", "--json", "--dir", d][..], extra].concat();
    let output = srl(&args);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    json_lines(&output).into_iter().next().unwrap()
}

/// Records `rating` for `card` with `srl review --stdin`.
fn record(d: &str, card: &serde_json::Value, rating: u8) {
    let request = serde_json::json!({"card": card, "rating": rating, "response": []});
    let output = srl_with_input(&["review", "--stdin", "--dir", d], &request.to_string());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
}

/// A store with two atoms, `鮪` (with a cloze sentence) and `鯖`, and
/// their IDs.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn fish() -> (TempDir, String, String) {
    let tmp = store();
    let lines = [
        r#"{"title": "鮪", "answer": "tuna", "tags": ["kanji"], "cloze": ["{{鮪}}は魚です。"]}"#,
        r#"{"title": "鯖", "answer": "mackerel", "tags": ["kanji"]}"#,
    ];
    let added = srl_with_input(&["add", "--stdin", "--dir", dir(&tmp)], &lines.join("\n"));
    let [tuna, mackerel] = ids(&added).try_into().unwrap();
    (tmp, tuna, mackerel)
}

#[test]
fn the_study_loop_records_reviews_and_schedules_atoms() {
    let (tmp, tuna, mackerel) = fish();
    let d = dir(&tmp);
    assert_eq!(ids(&srl(&["find", "state:new", "--dir", d])).len(), 2);

    // A new atom comes first, by path, with the first card in drill order.
    let card = next_card(d, &[]);
    assert_eq!(card["atoms"][0], tuna.as_str());
    assert_eq!(card["ref"], "reveal");

    // The cloze card is graded by comparing the answer.
    let cloze = next_card(d, &["--drill", "cloze"]);
    let request = serde_json::json!({"card": cloze, "response": ["鮪"]});
    let graded = srl_with_input(&["grade", "--dir", d], &request.to_string());
    assert_eq!(
        json_lines(&graded),
        [serde_json::json!({"correct": [true], "suggested_rating": 3, "feedback": null})]
    );

    record(d, &card, 3);
    // Rated today, so it is not next again; the other new atom is.
    let card = next_card(d, &[]);
    assert_eq!(card["atoms"][0], mackerel.as_str());
    let by_id = srl(&[
        "review", &mackerel, "--rating", "1", "--by", "agent", "--dir", d,
    ]);
    assert_eq!(by_id.status.code(), Some(0), "{}", stderr(&by_id));

    // Nothing is left today.
    let none = srl(&["next", "--dir", d]);
    assert_eq!(none.status.code(), Some(1));
    assert!(none.stdout.is_empty());
    assert_eq!(
        srl(&["find", "state:new", "--dir", d]).status.code(),
        Some(1)
    );
    assert_eq!(ids(&srl(&["find", "state:young", "--dir", d])).len(), 2);

    let log = srl(&["log", "--dir", d]);
    let lines = stdout(&log);
    let lines = lines.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].ends_with(&format!("  {tuna}  review  3  reveal  human")));
    assert!(lines[1].ends_with(&format!("  {mackerel}  review  1  -  agent")));
    let only_tuna = format!("id:{tuna}");
    let events = json_lines(&srl(&["log", &only_tuna, "--json", "--dir", d]));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["card"]["ref"], "reveal");
    assert_eq!(events[0]["hash"].as_str().unwrap().len(), 12);

    // A reset makes reviewed atoms new again; then there is nothing to reset.
    let reset = srl(&["reset", "--dir", d]);
    assert_eq!(ids(&reset), [tuna.clone(), mackerel.clone()]);
    assert_eq!(srl(&["reset", "--dir", d]).status.code(), Some(1));
    assert_eq!(ids(&srl(&["find", "state:new", "--dir", d])).len(), 2);
    assert_eq!(next_card(d, &[])["atoms"][0], tuna.as_str());
}

#[test]
fn the_new_atom_limit_holds_back_new_atoms() {
    let (tmp, _tuna, _mackerel) = fish();
    let d = dir(&tmp);
    fs::write(tmp.path().join("srl.toml"), "[schedule]\nnew_per_day = 1\n").unwrap();
    let card = next_card(d, &[]);
    record(d, &card, 3);
    assert_eq!(srl(&["next", "--dir", d]).status.code(), Some(1));
    assert_eq!(ids(&srl(&["find", "state:new", "--dir", d])).len(), 1);
}

#[test]
fn dry_runs_print_events_and_write_nothing() {
    let (tmp, tuna, _mackerel) = fish();
    let d = dir(&tmp);
    let dry = srl(&["review", &tuna, "--rating", "2", "--dry-run", "--dir", d]);
    assert_eq!(dry.status.code(), Some(0), "{}", stderr(&dry));
    assert_eq!(json_lines(&dry)[0]["rating"], 2);
    assert_eq!(srl(&["log", "--dir", d]).status.code(), Some(1));
    srl(&["review", &tuna, "--rating", "2", "--dir", d]);
    let reset = srl(&["reset", "--dry-run", "--dir", d]);
    assert_eq!(ids(&reset), [tuna.as_str()]);
    assert_eq!(ids(&srl(&["find", "state:new", "--dir", d])).len(), 1);
}

#[test]
fn bad_reviews_record_nothing() {
    let (tmp, tuna, _mackerel) = fish();
    let d = dir(&tmp);
    let unknown = srl(&["review", "zzzzzz", "--rating", "3", "--dir", d]);
    assert_eq!(unknown.status.code(), Some(2));
    let card = next_card(d, &[]);
    let bad = serde_json::json!({"card": card, "rating": 7});
    let output = srl_with_input(&["review", "--stdin", "--dir", d], &bad.to_string());
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("rating"), "{}", stderr(&output));
    let both = srl(&["review", &tuna, "--rating", "3", "--stdin", "--dir", d]);
    assert_eq!(both.status.code(), Some(2));
    assert_eq!(srl(&["log", "--dir", d]).status.code(), Some(1));
}

#[test]
fn a_broken_log_line_is_reported_and_skipped() {
    let (tmp, tuna, _mackerel) = fish();
    let d = dir(&tmp);
    srl(&["review", &tuna, "--rating", "3", "--dir", d]);
    let file = fs::read_dir(tmp.path().join("log"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, format!("{text}not an event\n")).unwrap();
    let output = srl(&["log", "--dir", d]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output).lines().count(), 1);
    assert!(stderr(&output).contains("line 2"), "{}", stderr(&output));
}

#[test]
fn cmd_cards_are_graded_by_the_configured_grader() {
    let (tmp, drills) = store_with_drills();
    let d = dir(&tmp);
    let id = stdout(&srl(&["add", "--title", "x", "--dir", d]))
        .trim()
        .to_owned();
    let card = serde_json::json!({"atoms": [id], "drill": "llm", "ref": "llm:1", "prompt": "Use it in a sentence.",
        "answer": "Any sentence.", "grading": "cmd"});
    let request = serde_json::json!({"card": card, "response": ["My sentence."]});

    let fallback = srl_with_input(&["grade", "--dir", d], &request.to_string());
    assert_eq!(fallback.status.code(), Some(0), "{}", stderr(&fallback));
    assert_eq!(json_lines(&fallback)[0]["fallback"], "self");

    let program = drill_program(
        &drills,
        "grader",
        "A grader",
        0,
        &[r#"{"correct": [true], "rating": 4, "feedback": "Natural."}"#],
    );
    let config = format!(
        "[grading]\ncmd = {}\n",
        serde_json::json!(program.to_str().unwrap())
    );
    fs::write(tmp.path().join("srl.toml"), config).unwrap();
    let graded = srl_with_input(&["grade", "--dir", d], &request.to_string());
    assert_eq!(graded.status.code(), Some(0), "{}", stderr(&graded));
    assert_eq!(
        json_lines(&graded),
        [serde_json::json!({"correct": [true], "suggested_rating": 4, "feedback": "Natural."})]
    );
    let input = fs::read_to_string(drills.join("srl-drill-grader.input")).unwrap();
    let input = serde_json::from_str::<serde_json::Value>(&input).unwrap();
    assert_eq!(input["atom"]["title"], "x");
    assert_eq!(input["response"][0], "My sentence.");
}

#[test]
fn stats_counts_states_reviews_and_retention() {
    let (tmp, tuna, _mackerel) = fish();
    let d = dir(&tmp);
    srl(&["review", &tuna, "--rating", "3", "--dir", d]);
    let output = srl(&["stats", "--dir", d]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "atoms  2\nnew  1\ndue  0\nyoung  1\nmature  0\nleech  0\nreviews today  1\nretention  -\n"
    );
    let json = json_lines(&srl(&["stats", "state:young", "--json", "--dir", d]));
    assert_eq!(
        json,
        [
            serde_json::json!({"atoms": 1, "states": {"new": 0, "due": 0, "young": 1, "mature": 0, "leech": 0},
            "reviews_today": 1, "retention": null, "retention_reviews": 0})
        ]
    );
    assert_eq!(
        srl(&["stats", "tag:none", "--dir", d]).status.code(),
        Some(1)
    );
}

#[test]
fn interactive_commands_refuse_without_a_terminal() {
    let (tmp, tuna, _mackerel) = fish();
    let d = dir(&tmp);
    let study = srl_with_input(&["study", "--dir", d], "\n3\n");
    assert_eq!(study.status.code(), Some(2));
    assert!(stderr(&study).contains("srl next"), "{}", stderr(&study));
    let edit = srl_with_input(&["edit", &tuna, "--dir", d], "");
    assert_eq!(edit.status.code(), Some(2));
    assert!(stderr(&edit).contains("--stdin"), "{}", stderr(&edit));
    assert_eq!(srl(&["log", "--dir", d]).status.code(), Some(1));
}

#[test]
fn sessions_are_saved_listed_and_removed() {
    let (tmp, tuna, mackerel) = fish();
    let d = dir(&tmp);
    let card = |id: &str, prompt: &str| {
        serde_json::json!({"atoms": [id], "drill": "agent", "ref": format!("agent:{prompt}"),
            "prompt": prompt, "answer": "a", "grading": "self"})
        .to_string()
    };
    let cards = [card(&mackerel, "one"), card(&tuna, "two")].join("\n");
    let put = srl_with_input(&["session", "put", "n1-food", "--dir", d], &cards);
    assert_eq!(put.status.code(), Some(0), "{}", stderr(&put));
    assert!(put.stdout.is_empty());
    assert_eq!(
        stdout(&srl(&["session", "list", "--dir", d])),
        "n1-food  2\n"
    );
    assert_eq!(
        json_lines(&srl(&["session", "list", "--json", "--dir", d])),
        [serde_json::json!({"name": "n1-food", "cards": 2})]
    );

    let bad = format!("{cards}\n{}", card("zzzzzz", "three"));
    let refused = srl_with_input(&["session", "put", "other", "--dir", d], &bad);
    assert_eq!(refused.status.code(), Some(2));
    assert!(stderr(&refused).contains("line 3"), "{}", stderr(&refused));
    let badly_named = srl_with_input(&["session", "put", "../x", "--dir", d], &cards);
    assert_eq!(badly_named.status.code(), Some(2));

    let study = srl_with_input(&["study", "--session", "n1-food", "--dir", d], "\n3\n");
    assert_eq!(study.status.code(), Some(2));
    assert!(stderr(&study).contains("terminal"), "{}", stderr(&study));

    let dry = srl(&["session", "rm", "n1-food", "--dry-run", "--dir", d]);
    assert_eq!(dry.status.code(), Some(0));
    assert!(srl(&["session", "list", "--dir", d]).status.success());
    assert_eq!(
        srl(&["session", "rm", "n1-food", "--dir", d]).status.code(),
        Some(0)
    );
    assert_eq!(srl(&["session", "list", "--dir", d]).status.code(), Some(1));
    assert_eq!(
        srl(&["session", "rm", "n1-food", "--dir", d]).status.code(),
        Some(2)
    );
}

#[test]
fn new_refuses_without_a_terminal() {
    let tmp = store();
    let output = srl_with_input(&["new", "--dir", dir(&tmp)], "");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("srl add --stdin"),
        "{}",
        stderr(&output)
    );
}
/// What every command that reads atoms and memories prints: standard
/// output, standard error and the exit code of each.
fn snapshot(d: &str) -> Vec<(String, String, Option<i32>)> {
    let commands: [&[&str]; 8] = [
        &["find"],
        &["find", "state:due"],
        &["find", "state:new", "-tag:none"],
        &["next", "--json"],
        &["stats", "--json"],
        &["log", "--json"],
        &["log", "state:young"],
        &["tags", "--json"],
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
    let (tmp, tuna, mackerel) = fish();
    let d = dir(&tmp);
    let cache = tmp.path().join(".cache");
    srl(&["review", &tuna, "--rating", "3", "--dir", d]);
    fs::write(
        tmp.path().join("atoms").join("broken.md"),
        "no front-matter\n",
    )
    .unwrap();
    let log = tmp.path().join("log");
    let month = fs::read_dir(&log).unwrap().next().unwrap().unwrap().path();
    let text = fs::read_to_string(&month).unwrap();
    fs::write(&month, format!("{text}not an event\n")).unwrap();

    // Built by the first command, then used.
    let built = snapshot(d);
    assert!(cache.join("index.sqlite").is_file());
    assert_eq!(fs::read_to_string(cache.join(".gitignore")).unwrap(), "*\n");
    assert!(built[0].1.contains("broken.md"), "{}", built[0].1);
    assert!(built[4].1.contains("skipped line"), "{}", built[4].1);
    assert_eq!(snapshot(d), built);

    // srl reindex starts over, prints nothing, and reports the files' problems.
    let reindex = srl(&["reindex", "--dir", d]);
    assert_eq!(reindex.status.code(), Some(0));
    assert!(reindex.stdout.is_empty());
    assert!(stderr(&reindex).contains("broken.md"));
    assert_eq!(snapshot(d), built);

    // Deleting the cache loses nothing.
    fs::remove_dir_all(&cache).unwrap();
    assert_eq!(snapshot(d), built);

    // Changes to atoms and the log show up as they would without a cache.
    srl(&["review", &mackerel, "--rating", "1", "--dir", d]);
    srl(&["review", &tuna, "--rating", "4", "--dir", d]);
    srl(&["suspend", &format!("id:{tuna}"), "--dir", d]);
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
    assert_eq!(srl(&["reindex", "--dir", d]).status.code(), Some(2));
}

#[test]
fn concurrent_srl_processes_share_the_log_and_the_cache() {
    let (tmp, tuna, mackerel) = fish();
    let d = dir(&tmp);
    let spawn = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_srl"))
            .args(args)
            .args(["--dir", d])
            .env_remove("SRL_DIR")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let children = (0..10)
        .flat_map(|round| {
            let atom = if round % 2 == 0 { &tuna } else { &mackerel };
            [
                spawn(&["review", atom, "--rating", "3"]),
                spawn(&["next", "--json"]),
                spawn(&["stats", "--json"]),
            ]
        })
        .collect::<Vec<_>>();
    for child in children {
        let output = child.wait_with_output().unwrap();
        // `srl next` exits 1 once both atoms have been reviewed.
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "{}",
            stderr(&output)
        );
        assert_eq!(stderr(&output), "");
    }
    assert!(tmp.path().join("log").join(".lock").is_file());
    let events = json_lines(&srl(&["log", "--json", "--dir", d]));
    assert_eq!(events.len(), 10);
    let cached = snapshot(d);
    fs::remove_dir_all(tmp.path().join(".cache")).unwrap();
    assert_eq!(snapshot(d), cached);
}
