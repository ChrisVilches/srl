//! Checks the test source program itself. Having an integration test also
//! makes `cargo test --workspace` build the program, which `srl`'s own
//! tests run (see `docs/rust.md`, "Testing").

use std::io::Write as _;
use std::process::{Command, Stdio};

/// What the program prints for `call` with `input` (JSONL), line by line,
/// and whether it succeeded.
#[expect(clippy::unwrap_used, reason = "a test helper")]
fn run(call: &str, input: &str) -> (bool, Vec<String>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_srl-test-source"))
        .arg(call)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
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
fn describe_gives_its_settings() {
    let (ok, lines) = run(
        "describe",
        "{\"config\": {\"data\": \"nowhere.json\", \"creates\": [\"words\"]}}\n",
    );
    assert!(ok);
    assert_eq!(
        lines,
        [
            "{\"description\":\"Items in a JSON file, for srl's tests.\",\"settings\":{\"data\":\"nowhere.json\",\"creates\":[\"words\"]}}"
        ]
    );
}

#[test]
fn accepts_takes_the_names_of_creates() {
    let config = "\"config\": {\"data\": \"nowhere.json\", \"creates\": [\"words\", \"vocab/*\"]}";
    let input = ["words", "vocab/a", "vocab/a/b", "other"]
        .map(|name| format!("{{{config}, \"collection\": \"{name}\"}}\n"))
        .concat();
    let (ok, lines) = run("accepts", &input);
    assert!(ok);
    let refused = "{\"error\":\"it matches none of `creates`\"}";
    assert_eq!(lines, ["{\"ok\":true}", "{\"ok\":true}", refused, refused]);
}

#[test]
fn a_missing_data_setting_fails_the_call() {
    assert!(!run("read", "{\"config\": {}}\n").0);
}
