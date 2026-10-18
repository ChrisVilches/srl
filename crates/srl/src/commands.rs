//! Runs the built-in commands.
//!
//! Each command gathers its inputs, calls `srl-core` and hands the result to
//! `output`. Commands that are not written yet fail with a clear message.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, bail};
use srl_core::{Lookup, Store};

use crate::args::{Cli, Command, SessionCommand};
use crate::{Outcome, external};

pub(crate) fn run(cli: Cli) -> anyhow::Result<Outcome> {
    let name = match cli.command {
        Command::External(args) => return external::run(&args),
        Command::Init { .. } => "init",
        Command::Check { .. } => "check",
        Command::Reindex => "reindex",
        Command::New { .. } => "new",
        Command::Add(_) => "add",
        Command::Edit { .. } => "edit",
        Command::Show { .. } => "show",
        Command::Find { .. } => "find",
        Command::Tags { .. } => "tags",
        Command::Inbox { .. } => "inbox",
        Command::Accept { .. } => "accept",
        Command::Suspend { .. } => "suspend",
        Command::Unsuspend { .. } => "unsuspend",
        Command::Study { .. } => "study",
        Command::Next { .. } => "next",
        Command::Cards { .. } => "cards",
        Command::Grade => "grade",
        Command::Review(_) => "review",
        Command::Reset { .. } => "reset",
        Command::Session { action } => match action {
            SessionCommand::Put { .. } => "session put",
            SessionCommand::List { .. } => "session list",
            SessionCommand::Rm { .. } => "session rm",
        },
        Command::Drills { .. } => "drills",
        Command::Stats { .. } => "stats",
        Command::Log { .. } => "log",
    };
    let store = Store::locate(&lookup(cli.dir)?)?;
    bail!(
        "`srl {name}` is not implemented yet (store: {})",
        store.root().display()
    )
}

/// Gathers what `Store::locate` needs from the flag and the environment.
fn lookup(dir: Option<PathBuf>) -> anyhow::Result<Lookup> {
    Ok(Lookup {
        dir,
        env_dir: env::var_os("SRL_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from),
        cwd: env::current_dir().context("cannot read the current directory")?,
        home: env::home_dir(),
    })
}
