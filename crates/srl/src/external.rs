//! Extension like git: `srl NAME ARGS…` runs `srl-NAME ARGS…` from PATH.

use std::process;

use anyhow::{Context, bail};

use crate::Outcome;

pub(crate) fn run(args: &[String]) -> anyhow::Result<Outcome> {
    let Some((name, rest)) = args.split_first() else {
        bail!("no command given; run `srl --help` for the list of commands");
    };
    let program = format!("srl-{name}");
    // `which` searches PATH the way the platform's shell does: on Windows it
    // also tries each extension in PATHEXT (`.exe`, `.cmd`, `.bat`, …), which
    // running the bare name would not.
    let path = which::which(&program).map_err(|error| {
        anyhow::anyhow!(
            "unknown command `{name}`: it is not built in and `{program}` is not on PATH ({error})"
        )
    })?;
    let status = process::Command::new(&path)
        .args(rest)
        .status()
        .with_context(|| format!("cannot run `{}`", path.display()))?;
    match status.code() {
        Some(0) => Ok(Outcome::Done),
        Some(1) => Ok(Outcome::Nothing),
        Some(code) => bail!("`{program}` failed with exit code {code}"),
        // Only on Unix: a process killed by a signal has no exit code.
        None => bail!("`{program}` was stopped by a signal"),
    }
}
