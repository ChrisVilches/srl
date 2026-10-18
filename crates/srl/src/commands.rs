//! Runs the built-in commands.
//!
//! Each command gathers its inputs, calls `srl-core` and hands the result to
//! `output`. Atoms and memories are read through the index cache.

use std::env;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anyhow::{Context, bail};
use jiff::Zoned;
use jiff::civil::Date;
use srl_core::{
    Atom, Atoms, Config, Content, Drills, Error, Flags, Id, Index, Log, Lookup, Memories,
    NextOptions, ReviewRequest, Search, Selector, Status, Store,
};

use crate::args::{AddArgs, By, Cli, Command, ReviewArgs, SessionCommand};
use crate::{Outcome, editor, external, output, study};

pub(crate) fn run(cli: Cli) -> anyhow::Result<Outcome> {
    let store = || -> anyhow::Result<Store> { Ok(Store::open(&lookup(cli.dir.clone())?)?) };
    match cli.command {
        Command::External(args) => external::run(&args),
        Command::Init { dir, dry_run } => init(cli.dir, dir, dry_run.dry_run),
        Command::Show { ids, json } => show(&store()?, &ids, json.json),
        Command::Add(args) => add(&store()?, args),
        Command::Find { selector, json } => {
            let selector = Selector::parse(&selector.terms.join(" "))?;
            find(&store()?, &selector, json.json)
        }
        Command::Drafts { selector, json } => {
            let selector = Selector::parse(&selector.terms.join(" "))?.and_status(Status::Draft);
            find(&store()?, &selector, json.json)
        }
        Command::Tags { json } => tags(&store()?, json.json),
        Command::Check { fix, dry_run } => check(&store()?, fix, dry_run.dry_run),
        Command::Reindex => reindex(&store()?),
        Command::New { draft, dry_run } => new(&store()?, draft, dry_run.dry_run),
        Command::Edit { id, stdin, dry_run } => match (stdin, dry_run.dry_run) {
            (true, dry_run) => edit_stdin(&store()?, &id, dry_run),
            (false, false) => edit_in_editor(&store()?, &id),
            (false, true) => {
                bail!("--dry-run only works with --stdin; `srl edit ID` opens the editor")
            }
        },
        Command::Accept { selector, dry_run } => {
            let change = (Status::Draft, Status::Active);
            change_status(&store()?, &selector.terms, change, dry_run.dry_run)
        }
        Command::Suspend { selector, dry_run } => {
            let change = (Status::Active, Status::Suspended);
            change_status(&store()?, &selector.terms, change, dry_run.dry_run)
        }
        Command::Unsuspend { selector, dry_run } => {
            let change = (Status::Suspended, Status::Active);
            change_status(&store()?, &selector.terms, change, dry_run.dry_run)
        }
        Command::Study {
            selector,
            drill,
            session: None,
        } => study::run(&store()?, &selector.terms, drill.as_deref()),
        Command::Study {
            session: Some(name),
            ..
        } => study::run_session(&store()?, &name),
        Command::Next {
            selector,
            drill,
            json,
        } => next(&store()?, &selector.terms, drill.as_deref(), json.json),
        Command::Cards { id, json } => cards(&store()?, &id, json.json),
        Command::Review(args) => review(&store()?, &args),
        Command::Reset { selector, dry_run } => reset(&store()?, &selector.terms, dry_run.dry_run),
        Command::Session { action } => match action {
            SessionCommand::Put { name, dry_run } => session_put(&store()?, &name, dry_run.dry_run),
            SessionCommand::List { json } => session_list(&store()?, json.json),
            SessionCommand::Rm { name, dry_run } => {
                srl_core::remove_session(&store()?, &name, dry_run.dry_run)?;
                Ok(Outcome::Done)
            }
        },
        Command::Drills { json } => drills(&store()?, json.json),
        Command::Stats { selector, json } => stats(&store()?, &selector.terms, json.json),
        Command::Log { selector, json } => log(&store()?, &selector.terms, json.json),
    }
}

/// `srl accept`, `srl suspend` and `srl unsuspend`: moves the matching atoms
/// from one status to another, and prints the IDs of those that changed (or
/// would, with `--dry-run`).
fn change_status(
    store: &Store,
    terms: &[String],
    (from, to): (Status, Status),
    dry_run: bool,
) -> anyhow::Result<Outcome> {
    let selector = Selector::parse(&terms.join(" "))?;
    let mut index = Index::open(store);
    let atoms = load(&mut index)?;
    let selected = select(&mut index, &atoms, &selector)?;
    let changed = srl_core::change_status(store, &selected, from, to, dry_run)?;
    output::ids(&changed);
    Ok(found_anything(!changed.is_empty()))
}

/// `srl new`: writes a new atom in the editor and prints its ID (with
/// `--dry-run`, its ID and file). Exit 1 if the template came back
/// unchanged.
fn new(store: &Store, draft: bool, dry_run: bool) -> anyhow::Result<Outcome> {
    if !io::stdin().is_terminal() {
        bail!("`srl new` needs a terminal; in scripts and pipes, use `srl add --stdin`");
    }
    match editor::new(store, draft, dry_run)? {
        Some(added) => {
            output::added(&[added], dry_run);
            Ok(Outcome::Done)
        }
        None => Ok(Outcome::Nothing),
    }
}

/// `srl session put NAME`: saves the cards on standard input as a session.
fn session_put(store: &Store, name: &str, dry_run: bool) -> anyhow::Result<Outcome> {
    let text = io::read_to_string(io::stdin()).context("cannot read standard input")?;
    srl_core::put_session(store, &load(&mut Index::open(store))?, name, &text, dry_run)?;
    Ok(Outcome::Done)
}

/// `srl session list`: every session with its number of cards. Exit 1 if
/// there are none.
fn session_list(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let sessions = srl_core::list_sessions(store)?;
    if json {
        output::json_lines(&sessions)?;
    } else {
        output::sessions(&sessions);
    }
    Ok(found_anything(!sessions.is_empty()))
}

/// `srl edit ID`: edits the atom in the editor. Refuses without a terminal,
/// like every interactive command.
fn edit_in_editor(store: &Store, id: &str) -> anyhow::Result<Outcome> {
    let id = Id::parse(id)?;
    if !io::stdin().is_terminal() {
        bail!("`srl edit ID` needs a terminal; in scripts and pipes, use `srl edit ID --stdin`");
    }
    editor::edit(store, &load(&mut Index::open(store))?, &id)?;
    Ok(Outcome::Done)
}

/// `srl stats`: atoms per state, reviews today and recent retention, for
/// the matching atoms. Exit 1 if no atom matches.
fn stats(store: &Store, terms: &[String], json: bool) -> anyhow::Result<Outcome> {
    let selector = Selector::parse(&terms.join(" "))?;
    let mut index = Index::open(store);
    let atoms = load(&mut index)?;
    let config = Config::load(store)?;
    let history = index.history(&config);
    report_cache(&mut index);
    let (history, problems) = history?;
    problems.iter().for_each(output::problem);
    let today = config.schedule.study_day(now()?.datetime());
    let memories = Memories::new(history.memories, &config, today);
    let selected = atoms.select(&selector, Some(&memories));
    let stats = srl_core::stats(&selected, &memories, &history.reviews);
    if json {
        output::json_lines(&[&stats])?;
    } else {
        output::stats(&stats);
    }
    Ok(found_anything(!selected.is_empty()))
}

/// `srl edit ID --stdin`: replaces the atom's file with standard input.
fn edit_stdin(store: &Store, id: &str, dry_run: bool) -> anyhow::Result<Outcome> {
    let id = Id::parse(id)?;
    let text = io::read_to_string(io::stdin()).context("cannot read standard input")?;
    srl_core::replace(store, &load(&mut Index::open(store))?, &id, &text, dry_run)?;
    Ok(Outcome::Done)
}

/// `srl add`: from flags or JSON lines. Prints the new IDs, or with
/// `--dry-run` the IDs and the files that would be written.
fn add(store: &Store, args: AddArgs) -> anyhow::Result<Outcome> {
    let contents = if args.stdin {
        let text = io::read_to_string(io::stdin()).context("cannot read standard input")?;
        Content::from_json_lines(&text)?
    } else if let Some(title) = args.title {
        vec![Content::from_flags(Flags {
            title,
            tags: args.tags,
            source: args.source,
        })?]
    } else {
        bail!("nothing to add; give --title or --stdin");
    };
    let seed = getrandom::u64().context("cannot get random numbers for new IDs")?;
    let dry_run = args.dry_run.dry_run;
    let added = srl_core::add(
        store,
        &load(&mut Index::open(store))?,
        contents,
        args.draft,
        seed,
        dry_run,
    )?;
    output::added(&added, dry_run);
    Ok(Outcome::Done)
}

/// `srl find` and `srl drafts`: lists the matching atoms.
fn find(store: &Store, selector: &Selector, json: bool) -> anyhow::Result<Outcome> {
    let mut index = Index::open(store);
    let atoms = load(&mut index)?;
    let found = select(&mut index, &atoms, selector)?;
    if json {
        output::json_lines(&found)?;
    } else {
        output::atom_lines(&found);
    }
    Ok(found_anything(!found.is_empty()))
}

/// `srl tags`: every tag in use, with counts.
fn tags(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let counts = load(&mut Index::open(store))?.tags();
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

/// Reads every atom through the index cache, and reports the files that were
/// skipped and any problem with the cache.
pub(crate) fn load(index: &mut Index) -> anyhow::Result<Atoms> {
    let atoms = index.atoms();
    report_cache(index);
    let atoms = atoms?;
    atoms.problems.iter().for_each(output::problem);
    Ok(atoms)
}

/// Reports the problems with the index cache since the last report.
pub(crate) fn report_cache(index: &mut Index) {
    index.problems().iter().for_each(output::problem);
}

/// `srl reindex`: builds the index cache from scratch.
fn reindex(store: &Store) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    let (atoms, log_problems) = Index::rebuild(store, &config)?;
    atoms.problems.iter().for_each(output::problem);
    log_problems.iter().for_each(output::problem);
    Ok(Outcome::Done)
}

/// `srl show ID…`: prints the atoms that exist, and fails if any does not.
fn show(store: &Store, ids: &[String], json: bool) -> anyhow::Result<Outcome> {
    let ids = ids
        .iter()
        .map(|id| Id::parse(id))
        .collect::<srl_core::Result<Vec<_>>>()?;
    let atoms = load(&mut Index::open(store))?;
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

/// `srl drills`: every drill, where it was found, and its summary. A drill
/// whose `--help` fails is listed without a summary, and reported.
fn drills(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    let (drills, mut problems) = find_drills(store)?;
    let listed = drills
        .all()
        .map(|drill| match drill.summary(config.drills.timeout()) {
            Ok(summary) => (drill, Some(summary)),
            Err(problem) => {
                problems.push(problem);
                (drill, None)
            }
        })
        .collect::<Vec<_>>();
    if json {
        output::drills_json(&listed)?;
    } else {
        output::drills(&listed);
    }
    Ok(reported(&problems))
}

/// `srl cards ID`: every card of the atom, from every drill that applies.
fn cards(store: &Store, id: &str, json: bool) -> anyhow::Result<Outcome> {
    let id = Id::parse(id)?;
    let atoms = load(&mut Index::open(store))?;
    let atom = atoms.get(&id).ok_or(Error::UnknownId { id })?;
    let config = Config::load(store)?;
    let (drills, mut problems) = find_drills(store)?;
    let made = drills.cards(atom, &atoms, today(&config)?, config.drills.timeout());
    let cards = made
        .into_iter()
        .flat_map(|made| {
            problems.extend(made.problems);
            made.cards
        })
        .collect::<Vec<_>>();
    if json {
        output::json_lines(&cards)?;
    } else {
        output::cards(&cards);
    }
    Ok(reported(&problems))
}

/// `srl check [--fix]`: reports every problem, and prints the IDs it
/// assigned. Fails if any problem is left.
fn check(store: &Store, fix: bool, dry_run: bool) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    let (drills, search_problems) = find_drills(store)?;
    let seed = if fix {
        Some(getrandom::u64().context("cannot get random numbers for new IDs")?)
    } else {
        None
    };
    let day = today(&config)?;
    let report = srl_core::check(store, &drills, day, config.drills.timeout(), seed, dry_run)?;
    output::fixed(&report.fixed);
    let problems = search_problems
        .into_iter()
        .chain(report.problems)
        .collect::<Vec<_>>();
    Ok(reported(&problems))
}

/// Finds every drill, looking in the store and on `PATH`.
fn find_drills(store: &Store) -> anyhow::Result<(Drills, Vec<Error>)> {
    Ok(Drills::find(store, &search()?))
}

/// Where to look for programs: the folders of `PATH`, and the current
/// directory for relative ones.
pub(crate) fn search() -> anyhow::Result<Search> {
    Ok(Search {
        path: env::var_os("PATH")
            .map(|path| env::split_paths(&path).collect())
            .unwrap_or_default(),
        cwd: env::current_dir().context("cannot read the current directory")?,
    })
}

/// Today's study day, from the local time.
fn today(config: &Config) -> anyhow::Result<Date> {
    Ok(config.schedule.study_day(now()?.datetime()))
}

/// The local time. Fails if the system's time zone can't be found, rather
/// than guessing UTC.
pub(crate) fn now() -> anyhow::Result<Zoned> {
    let zone = jiff::tz::TimeZone::try_system().context(
        "cannot find the system's time zone, which decides the study day; set the TZ environment variable",
    )?;
    Ok(jiff::Timestamp::now().to_zoned(zone))
}

/// What the commands about reviews need: the configuration, the current
/// time, and every atom's memory on today's study day.
struct Scheduling {
    config: Config,
    now: Zoned,
    memories: Memories,
}

/// Reads the configuration, and the memories through the index cache;
/// reports the log lines that were skipped and any problem with the cache.
fn scheduling(index: &mut Index) -> anyhow::Result<Scheduling> {
    let config = Config::load(index.store())?;
    let now = now()?;
    let memories = index.memories(&config);
    report_cache(index);
    let (memories, problems) = memories?;
    problems.iter().for_each(output::problem);
    let today = config.schedule.study_day(now.datetime());
    let memories = Memories::new(memories, &config, today);
    Ok(Scheduling {
        config,
        now,
        memories,
    })
}

/// The atoms that match `selector`, reading memories only if the selector
/// has `state:` terms.
fn select<'a>(
    index: &mut Index,
    atoms: &'a Atoms,
    selector: &Selector,
) -> anyhow::Result<Vec<&'a Atom>> {
    let memories = if selector.needs_memories() {
        Some(scheduling(index)?.memories)
    } else {
        None
    };
    Ok(atoms.select(selector, memories.as_ref()))
}

/// `srl next`: the next card, if there is one. Problems with drills are
/// reported, but don't change the exit code, so a study loop keeps going.
fn next(
    store: &Store,
    terms: &[String],
    drill: Option<&str>,
    json: bool,
) -> anyhow::Result<Outcome> {
    let selector = Selector::parse(&terms.join(" "))?;
    let mut index = Index::open(store);
    let atoms = load(&mut index)?;
    let scheduling = scheduling(&mut index)?;
    let (drills, search_problems) = find_drills(store)?;
    search_problems.iter().for_each(output::problem);
    let options = NextOptions {
        drill,
        day: scheduling.memories.today(),
        timeout: scheduling.config.drills.timeout(),
    };
    let (card, problems) = srl_core::next(
        &atoms,
        &scheduling.memories,
        &drills,
        selector,
        &scheduling.config,
        options,
    )?;
    problems.iter().for_each(output::problem);
    let Some(card) = card else {
        return Ok(Outcome::Nothing);
    };
    if json {
        output::json_lines(&[card])?;
    } else {
        output::cards(&[card]);
    }
    Ok(Outcome::Done)
}

/// `srl review`: records a review from standard input, or of atoms by ID.
/// Prints nothing, or with `--dry-run` the events it would write.
fn review(store: &Store, args: &ReviewArgs) -> anyhow::Result<Outcome> {
    let atoms = load(&mut Index::open(store))?;
    let now = now()?;
    let dry_run = args.dry_run.dry_run;
    let events = if args.stdin {
        let text = io::read_to_string(io::stdin()).context("cannot read standard input")?;
        let request = ReviewRequest::parse(&text, &atoms)?;
        srl_core::review(store, &atoms, &request, &now, dry_run)?
    } else {
        let ids = args
            .ids
            .iter()
            .map(|id| Id::parse(id))
            .collect::<srl_core::Result<Vec<_>>>()?;
        let rating = args
            .rating
            .context("--rating is required when reviewing atoms by ID")?;
        let by = match args.by {
            By::Human => srl_core::By::Human,
            By::Agent => srl_core::By::Agent,
        };
        srl_core::review_ids(store, &atoms, &ids, rating, by, &now, dry_run)?
    };
    if dry_run {
        output::json_lines(&events)?;
    }
    Ok(Outcome::Done)
}

/// `srl reset`: makes the matching atoms that are not new new again, and
/// prints their IDs.
fn reset(store: &Store, terms: &[String], dry_run: bool) -> anyhow::Result<Outcome> {
    let selector = Selector::parse(&terms.join(" "))?;
    let mut index = Index::open(store);
    let atoms = load(&mut index)?;
    let scheduling = scheduling(&mut index)?;
    let selected = atoms.select(&selector, Some(&scheduling.memories));
    let reset = srl_core::reset(
        store,
        &selected,
        &scheduling.memories,
        &scheduling.now,
        dry_run,
    )?;
    output::ids(&reset);
    Ok(found_anything(!reset.is_empty()))
}

/// `srl log`: the events of the matching atoms, in log order. It needs every
/// event, so it reads the log files rather than the index cache, and
/// replays them itself for `state:` terms.
fn log(store: &Store, terms: &[String], json: bool) -> anyhow::Result<Outcome> {
    let selector = Selector::parse(&terms.join(" "))?;
    let atoms = load(&mut Index::open(store))?;
    let config = Config::load(store)?;
    let log = Log::load(store, &config.schedule)?;
    log.problems.iter().for_each(output::problem);
    let memories = if selector.needs_memories() {
        let today = config.schedule.study_day(now()?.datetime());
        Some(Memories::replay(&log, &config, today)?)
    } else {
        None
    };
    let ids = atoms
        .select(&selector, memories.as_ref())
        .into_iter()
        .map(Atom::id)
        .collect::<Vec<_>>();
    let events = log.of(&ids).map(|entry| &entry.event).collect::<Vec<_>>();
    if json {
        output::json_lines(&events)?;
    } else {
        output::events(&events);
    }
    Ok(found_anything(!events.is_empty()))
}

/// Reports `problems` on standard error. The command failed if there were
/// any.
fn reported(problems: &[Error]) -> Outcome {
    problems.iter().for_each(output::problem);
    if problems.is_empty() {
        Outcome::Done
    } else {
        Outcome::Failed
    }
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
        data_dir: dirs::data_dir(),
    })
}
