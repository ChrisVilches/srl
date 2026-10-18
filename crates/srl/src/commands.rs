//! Runs the built-in commands.
//!
//! Each command gathers its inputs, calls `srl-core` and hands the result to
//! `output`. Commands that are not written yet fail with a clear message.

use std::env;
use std::io;
use std::path::PathBuf;

use anyhow::{Context, bail};
use srl_core::{Atoms, Content, Error, Flags, Id, Lookup, Selector, Status, Store};

use crate::args::{AddArgs, Cli, Command, SessionCommand};
use crate::{Outcome, external, output};

pub(crate) fn run(cli: Cli) -> anyhow::Result<Outcome> {
    let store = || -> anyhow::Result<Store> { Ok(Store::open(&lookup(cli.dir.clone())?)?) };
    let name = match cli.command {
        Command::External(args) => return external::run(&args),
        Command::Init { dir, dry_run } => return init(cli.dir, dir, dry_run.dry_run),
        Command::Show { ids, json } => return show(&store()?, &ids, json.json),
        Command::Add(args) => return add(&store()?, args),
        Command::Find { selector, json } => {
            let selector = Selector::parse(&selector.terms.join(" "))?;
            return find(&store()?, &selector, json.json);
        }
        Command::Inbox { selector, json } => {
            let selector = Selector::parse(&selector.terms.join(" "))?.and_status(Status::Draft);
            return find(&store()?, &selector, json.json);
        }
        Command::Tags { json } => return tags(&store()?, json.json),
        Command::Check { .. } => "check",
        Command::Reindex => "reindex",
        Command::New { .. } => "new",
        Command::Edit { .. } => "edit",
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
    bail!(
        "`srl {name}` is not implemented yet (store: {})",
        store()?.root().display()
    )
}

/// `srl add`: from flags or JSON lines. Prints the new IDs, or with
/// `--dry-run` the IDs and the files that would be written.
fn add(store: &Store, args: AddArgs) -> anyhow::Result<Outcome> {
    if !args.files.is_empty() {
        bail!("`srl add FILE…` is not implemented yet; use --cue or --stdin");
    }
    let contents = if args.stdin {
        let text = io::read_to_string(io::stdin()).context("cannot read standard input")?;
        Content::from_json_lines(&text)?
    } else if let Some(cue) = args.cue {
        vec![Content::from_flags(Flags {
            cue,
            answer: args.answer,
            tags: args.tags,
            source: args.source,
        })?]
    } else {
        bail!("nothing to add; give --cue, --stdin or Markdown files");
    };
    let seed = getrandom::u64().context("cannot get random numbers for new IDs")?;
    let dry_run = args.dry_run.dry_run;
    let added = srl_core::add(store, &load(store)?, contents, args.draft, seed, dry_run)?;
    output::added(&added, dry_run);
    Ok(Outcome::Done)
}

/// `srl find` and `srl inbox`: lists the matching atoms.
fn find(store: &Store, selector: &Selector, json: bool) -> anyhow::Result<Outcome> {
    let atoms = load(store)?;
    let found = atoms.select(selector);
    if json {
        output::json_lines(&found)?;
    } else {
        output::atom_lines(&found);
    }
    Ok(found_anything(!found.is_empty()))
}

/// `srl tags`: every tag in use, with counts.
fn tags(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let counts = load(store)?.tags();
    if json {
        output::json_lines(&counts)?;
    } else {
        output::tag_counts(&counts);
    }
    Ok(found_anything(!counts.is_empty()))
}

/// Exit code 0 if something was found, 1 if not (like `grep`).
fn found_anything(found: bool) -> Outcome {
    if found {
        Outcome::Done
    } else {
        Outcome::Nothing
    }
}

/// Reads every atom and reports the files that were skipped.
fn load(store: &Store) -> anyhow::Result<Atoms> {
    let atoms = Atoms::load(store)?;
    atoms.problems.iter().for_each(output::problem);
    Ok(atoms)
}

/// `srl show ID…`: prints the atoms that exist, and fails if any does not.
fn show(store: &Store, ids: &[String], json: bool) -> anyhow::Result<Outcome> {
    let ids = ids
        .iter()
        .map(|id| Id::parse(id))
        .collect::<srl_core::Result<Vec<_>>>()?;
    let atoms = load(store)?;
    let found = ids
        .iter()
        .filter_map(|id| atoms.get(id))
        .collect::<Vec<_>>();
    if json {
        output::json_lines(&found)?;
    } else {
        output::files(found.iter().map(|atom| atom.text()));
    }
    let missing = ids
        .iter()
        .filter(|id| atoms.get(id).is_none())
        .map(|id| Error::UnknownId { id: id.clone() })
        .collect::<Vec<_>>();
    missing.iter().for_each(output::problem);
    Ok(if missing.is_empty() {
        Outcome::Done
    } else {
        Outcome::Failed
    })
}

/// `srl init [DIR]`: prints each path it creates.
fn init(flag: Option<PathBuf>, dir: Option<PathBuf>, dry_run: bool) -> anyhow::Result<Outcome> {
    let root = Store::init_target(&lookup(flag)?, dir);
    output::paths(&Store::init(&root, dry_run)?);
    Ok(Outcome::Done)
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
