//! Checks the test kind program itself. Having an integration test also
//! makes `cargo test --workspace` build the program, which `srl`'s own
//! tests run (see `docs/rust.md`, "Testing").

use std::io::Write as _;
use std::process::{Command, Stdio};

/// What the program prints for `call` with `input`, and whether it succeeded.
#[expect(clippy::unwrap_used, reason = "a test helper")]
fn run(call: &str, input: &str) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_srl-test-kind"))
        .arg(call)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap().trim().to_owned(),
    )
}

#[test]
fn groups_come_from_the_groups_field() {
    let input = r#"{"id": "abc", "fields": {"GROUPS": "a b|c"}}"#;
    assert_eq!(
        run("ids", input),
        (
            true,
            r#"{"groups":[{"cards":["a","b"]},{"cards":["c"]}]}"#.to_owned()
        )
    );
    assert_eq!(
        run("ids", r#"{"id": "abc", "fields": {}}"#),
        (true, r#"{"groups":[{"cards":[null]}]}"#.to_owned())
    );
}

#[test]
fn a_card_tells_what_the_program_got() {
    let input =
        r#"{"id": "abc", "fields": {}, "suffix": "a", "context": null, "day": "2026-10-27"}"#;
    let (ok, output) = run("card", input);
    assert!(ok);
    assert!(output.starts_with(
        r#"{"prompt":"a?","prompt_revealed":"a!","answer":"day \"2026-10-27\"; context null; in "#
    ));
    let input = r#"{"id": "abc", "fields": {"FAIL_CARD": "a"}, "suffix": "a", "context": null, "day": "2026-10-27"}"#;
    assert_eq!(run("card", input), (false, String::new()));
}

#[test]
fn describe_names_the_fields_above() {
    let (ok, output) = run("describe", "{}");
    assert!(ok);
    assert!(output.contains(r#""name":"GROUPS""#), "{output}");
}
