//! Extension like git: `srl NAME ARGS…` runs `srl-NAME ARGS…` from PATH.

use std::io::ErrorKind;
use std::process;

use anyhow::{Context, bail};

use crate::Outcome;

pub(crate) fn run(args: &[String]) -> anyhow::Result<Outcome> {
    let Some((name, rest)) = args.split_first() else {
        bail!("no command given; run `srl --help` for the list of commands");
    };
    let program = format!("srl-{name}");
    let status = match process::Command::new(&program).args(rest).status() {
        Ok(status) => status,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            bail!("unknown command `{name}`: it is not built in and `{program}` is not on PATH")
        }
        Err(error) => return Err(error).with_context(|| format!("cannot run `{program}`")),
    };
    match status.code() {
        Some(0) => Ok(Outcome::Done),
        Some(1) => Ok(Outcome::Nothing),
        Some(code) => bail!("`{program}` failed with exit code {code}"),
        None => bail!("`{program}` was stopped by a signal"),
    }
}
