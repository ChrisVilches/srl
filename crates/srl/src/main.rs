//! The `srl` command line: parses arguments, calls `srl-core`, prints results.
//!
//! No logic about sources, items, cards or scheduling lives here; it belongs in
//! `srl-core`, which this crate uses like any other library.

mod args;
mod argv;
mod commands;
mod output;

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
    #[cfg(unix)]
    stop_programs_on_signals();
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

/// When `srl` is stopped by `SIGINT`, `SIGTERM` or `SIGHUP`, stops the
/// programs it runs, with every process they started, then ends by the same
/// signal (see `docs/cards.md`, "Stopping a program"). On Windows a job
/// object stops them, with nothing to set up here.
#[cfg(unix)]
fn stop_programs_on_signals() {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    // Without the handler, which the system practically never refuses,
    // `srl` ends on these signals as before, leaving its programs running.
    let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGINT, SIGTERM, SIGHUP]) else {
        return;
    };
    std::thread::spawn(move || {
        if let Some(signal) = signals.forever().next() {
            srl_core::stop_programs();
            if signal_hook::low_level::emulate_default_handler(signal).is_err() {
                #[expect(clippy::exit, reason = "ending as the signal would have")]
                std::process::exit(128 + signal);
            }
        }
    });
}
