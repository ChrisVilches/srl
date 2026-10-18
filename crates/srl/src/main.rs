//! The `srl` command line: parses arguments, calls `srl-core`, prints results.
//!
//! No logic about atoms, cards or scheduling lives here; it belongs in
//! `srl-core`, which this crate uses like any other library.

mod args;
mod argv;
mod commands;
mod external;
mod output;

use std::process::ExitCode;

/// How a command that ran without errors ended (see `docs/cli.md`, exit codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Success, or something was found. Exit code 0.
    Done,
    /// Nothing was found or nothing is left. Exit code 1.
    Nothing,
}

fn main() -> ExitCode {
    match commands::run(argv::parse()) {
        Ok(Outcome::Done) => ExitCode::SUCCESS,
        Ok(Outcome::Nothing) => ExitCode::from(1),
        Err(error) => {
            output::error(&error);
            ExitCode::from(2)
        }
    }
}
