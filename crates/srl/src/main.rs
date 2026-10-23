//! The `srl` command line: parses arguments, calls `srl-core`, prints results.
//!
//! No logic about notes, items, cards or scheduling lives here; it belongs in
//! `srl-core`, which this crate uses like any other library.

mod args;
mod argv;
mod commands;
mod editor;
mod external;
mod output;
mod study;

use std::process::ExitCode;

/// How a command that ran without errors ended (see `docs/cli.md`, exit codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Success, or something was found. Exit code 0.
    Done,
    /// Nothing was found or nothing is left. Exit code 1.
    Nothing,
    /// Part of the command failed, and was already reported on standard
    /// error. Exit code 2.
    Failed,
}

fn main() -> ExitCode {
    match commands::run(argv::parse()) {
        Ok(Outcome::Done) => ExitCode::SUCCESS,
        Ok(Outcome::Nothing) => ExitCode::from(1),
        Ok(Outcome::Failed) => ExitCode::from(2),
        Err(error) => {
            output::error(&error);
            ExitCode::from(2)
        }
    }
}
