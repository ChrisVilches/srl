//! Runs the real `srl` binary and checks exit codes and output streams.

use std::fs;
use std::io::Write;
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
fn unwritten_commands_say_so_on_stderr() {
    let tmp = store();
    let output = srl(&["next", "-tag:a", "--dir", dir(&tmp)]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not implemented yet"));
    assert!(stderr.contains(dir(&tmp)));
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
        "--cue",
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
    let line = r#"{"cue": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"], "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred.\n"}"#;
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
        "{\"cue\": \"fine\"}\n{\"cue\": \"bad\", \"status\": \"paused\"}\n",
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
    let output = srl(&["add", "--dry-run", "--dir", dir(&tmp), "--cue", "x"]);
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
    let id = stdout(&srl(&["add", "--dir", dir(&tmp), "--cue", "x"]))
        .trim()
        .to_owned();
    let output = srl(&["show", "--dir", dir(&tmp), &id, "zzzzzz"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).contains("cue: x"));
    assert!(stderr(&output).contains("zzzzzz"));
}

#[test]
fn broken_atom_files_are_reported_and_skipped() {
    let tmp = store();
    let id = stdout(&srl(&["add", "--dir", dir(&tmp), "--cue", "x"]))
        .trim()
        .to_owned();
    fs::write(
        tmp.path().join("atoms").join("broken.md"),
        "no front-matter\n",
    )
    .unwrap();
    let output = srl(&["show", "--dir", dir(&tmp), &id]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("cue: x"));
    assert!(stderr(&output).contains("atoms/broken.md"));
}

/// A store with three atoms: N1 grammar, N5 grammar and a draft food kanji.
fn japanese() -> TempDir {
    let tmp = store();
    let lines = [
        r#"{"cue": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"]}"#,
        r#"{"cue": "です", "tags": ["japanese/grammar", "jlpt/n5"]}"#,
    ];
    let added = srl_with_input(&["add", "--stdin", "--dir", dir(&tmp)], &lines.join("\n"));
    assert_eq!(added.status.code(), Some(0), "{}", stderr(&added));
    let draft = srl(&[
        "add",
        "--draft",
        "--dir",
        dir(&tmp),
        "--cue",
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
