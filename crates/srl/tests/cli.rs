//! Runs the real `srl` binary and checks exit codes and output streams.

use std::process::{Command, Output};

use tempfile::TempDir;

#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn srl(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_srl"))
        .args(args)
        .env_remove("SRL_DIR")
        .output()
        .unwrap()
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
