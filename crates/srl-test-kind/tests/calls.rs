//! Checks the test kind program itself. Having an integration test also
//! makes `cargo test --workspace` build the program, which `srl`'s own
//! tests run (see `docs/rust.md`, "Testing").

use std::io::Write as _;
use std::process::{Command, Stdio};

/// What the program prints for `call` with `input` (JSONL), line by line,
/// and whether it succeeded.
#[expect(clippy::unwrap_used, reason = "a test helper")]
fn run(call: &str, input: &str) -> (bool, Vec<String>) {
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
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect(),
    )
}

#[test]
fn each_input_line_gets_one_line_in_order() {
    let input = "{\"id\": \"abc\", \"fields\": {\"GROUPS\": \"a b|c\"}}\n{\"id\": \"abd\", \"fields\": {}}\n";
    assert_eq!(
        run("groups", input),
        (
            true,
            vec![
                r#"{"groups":[["a","b"],["c"]]}"#.to_owned(),
                r#"{"groups":[[null]]}"#.to_owned(),
            ]
        )
    );
    let input =
        "{\"id\": \"abc\", \"fields\": {}}\n{\"id\": \"abd\", \"fields\": {\"INVALID\": \"no\"}}\n";
    assert_eq!(
        run("validate", input),
        (
            true,
            vec![
                r#"{"errors":[]}"#.to_owned(),
                r#"{"errors":[{"field":null,"message":"no"}]}"#.to_owned(),
            ]
        )
    );
}

#[test]
fn a_card_tells_what_the_program_got() {
    let input = "{\"id\": \"abc\", \"fields\": {}, \"suffix\": \"a\", \"context\": \"c\", \"context_format\": \"markdown\", \"day\": \"2026-10-27\"}\n";
    let (ok, output) = run("make", input);
    assert!(ok);
    assert!(output[0].starts_with(
        r#"{"prompt":"a?","prompt_revealed":"a!","answer":"day \"2026-10-27\"; context \"c\" (\"markdown\"); in "#
    ));
    let input = "{\"id\": \"abc\", \"fields\": {\"FAIL_MAKE\": \"a\"}, \"suffix\": \"a\", \"context\": null, \"day\": \"2026-10-27\"}\n";
    assert_eq!(
        run("make", input),
        (
            true,
            vec![r#"{"error":"card a fails on purpose"}"#.to_owned()]
        )
    );
}

#[test]
fn describe_names_the_fields_above() {
    let (ok, output) = run("describe", "{}\n");
    assert!(ok);
    assert!(output[0].contains(r#""name":"GROUPS""#), "{output:?}");
}
