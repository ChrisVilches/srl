//! Runs the built-in commands.
//!
//! Each command gathers its inputs, calls `srl-core` and hands the result to
//! `output`. Memories are read through the index cache.

use std::collections::HashSet;
use std::env;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anyhow::{Context, bail};
use jiff::Zoned;
use jiff::civil::Date;
use srl_core::{
    Card, CardId, Config, ContextMode, Error, Index, Item, Log, Lookup, Memories, Notes,
    ReviewRequest, Selector, State, Status, Store,
};

use crate::args::{AddArgs, By, Cli, Command, ReviewArgs};
use crate::{Outcome, editor, external, output, study};

pub(crate) fn run(cli: Cli) -> anyhow::Result<Outcome> {
    let store = || -> anyhow::Result<Store> { Ok(Store::open(&lookup(cli.dir.clone())?)?) };
    let selector = |terms: &[String]| Selector::parse(&terms.join(" "));
    match cli.command {
        Command::External(args) => external::run(&args),
        Command::Init { dir, dry_run } => init(cli.dir, dir, dry_run.dry_run),
        Command::Check => check(&store()?),
        Command::Lint => lint(&store()?),
        Command::Id { dry_run } => id(&store()?, dry_run.dry_run),
        Command::Reindex => reindex(&store()?),
        Command::Find {
            selector: terms,
            json,
        } => find(&store()?, &selector(&terms.terms)?, json.json),
        Command::Drafts {
            selector: terms,
            json,
        } => find(
            &store()?,
            &selector(&terms.terms)?.and_status(Status::Draft),
            json.json,
        ),
        Command::Show { ids, context, json } => show(&store()?, &ids, context, json.json),
        Command::Cards { ids, json } => cards(&store()?, &ids, json.json),
        Command::Add(args) => add(&store()?, &args),
        Command::Edit { id, stdin, dry_run } => match (stdin, dry_run.dry_run) {
            (true, dry_run) => edit_stdin(&store()?, &id, dry_run),
            (false, false) => edit_in_editor(&store()?, &id),
            (false, true) => {
                bail!("--dry-run only works with --stdin; `srl edit ID` opens the editor")
            }
        },
        Command::Accept {
            selector: terms,
            dry_run,
        } => accept(&store()?, &selector(&terms.terms)?, dry_run.dry_run),
        Command::Suspend {
            selector: terms,
            dry_run,
        } => suspend(&store()?, &selector(&terms.terms)?, true, dry_run.dry_run),
        Command::Unsuspend {
            selector: terms,
            dry_run,
        } => suspend(&store()?, &selector(&terms.terms)?, false, dry_run.dry_run),
        Command::Move { from, to, dry_run } => move_history(&store()?, &from, &to, dry_run.dry_run),
        Command::Study { selector: terms } => study::run(&store()?, &selector(&terms.terms)?),
        Command::Next {
            selector: terms,
            json,
        } => next(&store()?, selector(&terms.terms)?, json.json),
        Command::Review(args) => review(&store()?, &args),
        Command::Reset {
            selector: terms,
            dry_run,
        } => reset(&store()?, &selector(&terms.terms)?, dry_run.dry_run),
        Command::Stats {
            selector: terms,
            json,
        } => stats(&store()?, &selector(&terms.terms)?, json.json),
        Command::Log {
            selector: terms,
            json,
        } => log(&store()?, &selector(&terms.terms)?, json.json),
    }
}

/// What most commands work with: the configuration, every item, every
/// card's memory today, and the time.
struct Loaded {
    config: Config,
    notes: Notes,
    memories: Memories,
    now: Zoned,
}

/// [`load`], then the memories through the index cache, reporting the log
/// lines that were skipped.
fn open(store: &Store) -> anyhow::Result<Loaded> {
    let (config, notes) = load(store)?;
    let (memories, now, problems) = memories(&mut Index::open(store), &config)?;
    problems.iter().for_each(output::problem);
    Ok(Loaded {
        config,
        notes,
        memories,
        now,
    })
}

/// Reads the configuration and every note, and stops if the notes have
/// any problem (see `docs/notes.md`, "Errors").
fn load(store: &Store) -> anyhow::Result<(Config, Notes)> {
    let config = Config::load(store)?;
    let notes = strict(Notes::load(store, &config)?)?;
    Ok((config, notes))
}

/// `notes`, if they have no problems; otherwise reports every problem and
/// fails.
pub(crate) fn strict(notes: Notes) -> anyhow::Result<Notes> {
    notes.problems().iter().for_each(output::problem);
    let count = notes.problems().len();
    if count > 0 {
        return Err(Error::NotesInvalid { count }.into());
    }
    Ok(notes)
}

/// Every card's memory on today's study day, through the index cache, the
/// current time, and the log lines that were skipped. Reports any problem
/// with the cache.
pub(crate) fn memories(
    index: &mut Index,
    config: &Config,
) -> anyhow::Result<(Memories, Zoned, Vec<Error>)> {
    let now = now()?;
    let memories = index.memories(config);
    report_cache(index);
    let (memories, problems) = memories?;
    let today = config.schedule.study_day(now.datetime());
    Ok((Memories::new(memories, config, today), now, problems))
}

/// Reports the problems with the index cache since the last report.
fn report_cache(index: &mut Index) {
    index.problems().iter().for_each(output::problem);
}

/// Exit code 0 if something was found, 1 if not (like `grep`).
fn found_anything(found: bool) -> Outcome {
    if found {
        Outcome::Done
    } else {
        Outcome::Nothing
    }
}

/// `srl init [DIR]`: prints each path it creates.
fn init(flag: Option<PathBuf>, dir: Option<PathBuf>, dry_run: bool) -> anyhow::Result<Outcome> {
    let root = Store::init_target(&lookup(flag)?, dir);
    output::paths(&Store::init(&root, dry_run)?);
    Ok(Outcome::Done)
}

/// `srl check`: reports every problem, and orphans as notices. Fails if
/// there is any problem.
fn check(store: &Store) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    let notes = Notes::load(store, &config)?;
    let (memories, _, log_problems) = memories(&mut Index::open(store), &config)?;
    let long_paths = srl_core::long_paths(store, &notes)?;
    let problems = notes
        .problems()
        .iter()
        .chain(&long_paths)
        .chain(&log_problems)
        .collect::<Vec<_>>();
    problems.iter().copied().for_each(output::problem);
    srl_core::orphans(&notes, &memories)
        .iter()
        .for_each(output::problem);
    Ok(if problems.is_empty() {
        Outcome::Done
    } else {
        Outcome::Failed
    })
}

/// `srl lint`: prints what looks like a mistake. Exit 0 if it found
/// something, 1 if not.
fn lint(store: &Store) -> anyhow::Result<Outcome> {
    let found = srl_core::lint(store, &Config::load(store)?)?;
    output::lints(&found);
    Ok(found_anything(!found.is_empty()))
}

/// `srl id`: gives an ID to every item without one, and prints them. Exit
/// 1 if every item had one.
fn id(store: &Store, dry_run: bool) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    let notes = Notes::load(store, &config)?;
    notes
        .problems()
        .iter()
        .filter(|problem| !matches!(problem, Error::MissingId { .. }))
        .for_each(output::problem);
    let written = srl_core::assign_ids(store, &notes, seed()?, dry_run)?;
    output::written(&written);
    Ok(found_anything(!written.is_empty()))
}

/// `srl reindex`: builds the index cache from scratch.
fn reindex(store: &Store) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    Index::rebuild(store, &config)?
        .iter()
        .for_each(output::problem);
    Ok(Outcome::Done)
}

/// The cards that match `selector`, with their items.
fn select<'a>(loaded: &'a Loaded, selector: &Selector) -> Vec<(&'a Item, &'a Card)> {
    selector.select(loaded.notes.cards(), &loaded.memories)
}

/// `srl find` and `srl drafts`: lists the matching cards.
fn find(store: &Store, selector: &Selector, json: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let found = select(&loaded, selector);
    if json {
        let lines = found
            .iter()
            .map(|(item, card)| {
                let memories = &loaded.memories;
                serde_json::json!({
                    "id": card.id(),
                    "item": item.id(),
                    "path": item.path(),
                    "line": item.line(),
                    "status": Status::of(item, card, memories),
                    "state": state_name(memories.state(card.id())),
                    "due": memories.get(card.id()).due.map(|day| day.to_string()),
                })
            })
            .collect::<Vec<_>>();
        output::json_lines(&lines)?;
    } else {
        output::card_lines(&found);
    }
    Ok(found_anything(!found.is_empty()))
}

fn state_name(state: State) -> &'static str {
    match state {
        State::New => "new",
        State::Due => "due",
        State::Young => "young",
        State::Mature => "mature",
    }
}

/// The items the IDs given to a command stand for, each once, in the order
/// given; and the IDs that name nothing, reported.
fn items_of<'a>(notes: &'a Notes, ids: &[String]) -> (Vec<&'a Item>, Vec<Error>) {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut problems = Vec::new();
    for id in ids {
        match notes.item_of(id) {
            Ok(item) => {
                if seen.insert(item.id().clone()) {
                    items.push(item);
                }
            }
            Err(error) => problems.push(error),
        }
    }
    (items, problems)
}

/// Fails, as exit code 2, if there were problems; they are reported.
fn reported(problems: &[Error]) -> Outcome {
    problems.iter().for_each(output::problem);
    if problems.is_empty() {
        Outcome::Done
    } else {
        Outcome::Failed
    }
}

/// `srl show ID…`: prints each item with its context.
fn show(
    store: &Store,
    ids: &[String],
    mode: Option<crate::args::ContextMode>,
    json: bool,
) -> anyhow::Result<Outcome> {
    let (config, notes) = load(store)?;
    let mode = mode.map_or(config.study.context, |mode| match mode {
        crate::args::ContextMode::Prose => ContextMode::Prose,
        crate::args::ContextMode::Full => ContextMode::Full,
        crate::args::ContextMode::None => ContextMode::None,
    });
    let (items, problems) = items_of(&notes, ids);
    let shown = items
        .into_iter()
        .map(|item| Ok((item, srl_core::context(store, item, mode)?)))
        .collect::<srl_core::Result<Vec<_>>>()?;
    if json {
        let lines = shown
            .iter()
            .map(|(item, context)| item.json(context.as_deref()))
            .collect::<Vec<_>>();
        output::json_lines(&lines)?;
    } else {
        output::items(&shown);
    }
    Ok(reported(&problems))
}

/// `srl cards ID…`: the cards of each item ID, or the card of each card ID.
fn cards(store: &Store, ids: &[String], json: bool) -> anyhow::Result<Outcome> {
    let (_, notes) = load(store)?;
    let mut problems = Vec::new();
    let mut found = Vec::new();
    for id in ids {
        match notes
            .item_of(id)
            .and_then(|item| Ok((item, CardId::parse(id)?)))
        {
            Ok((item, card_id)) => found.extend(
                item.cards()
                    .iter()
                    .filter(|card| card_id.suffix().is_none() || card.id() == &card_id),
            ),
            Err(error) => problems.push(error),
        }
    }
    if json {
        output::json_lines(&found)?;
    } else {
        output::cards(&found);
    }
    Ok(reported(&problems))
}

/// `srl add NOTE --stdin`: appends items to a note, and prints their IDs,
/// or with `--dry-run` the items as they would be written.
fn add(store: &Store, args: &AddArgs) -> anyhow::Result<Outcome> {
    let (config, notes) = load(store)?;
    let input = stdin()?;
    let dry_run = args.dry_run.dry_run;
    let added = srl_core::add(store, &config, &notes, &args.note, &input, seed()?, dry_run)?;
    if dry_run {
        output::text(&added.text);
    } else {
        output::ids(&added.ids);
    }
    Ok(found_anything(!added.ids.is_empty()))
}

/// `srl edit ID`: opens the item's note in the editor. Refuses without a
/// terminal, like every interactive command.
fn edit_in_editor(store: &Store, id: &str) -> anyhow::Result<Outcome> {
    if !io::stdin().is_terminal() {
        bail!("`srl edit ID` needs a terminal; in scripts and pipes, use `srl edit ID --stdin`");
    }
    let (_, notes) = load(store)?;
    let item = notes.item_of(id)?;
    editor::open(&store.note_file(item.path()))?;
    Ok(Outcome::Done)
}

/// `srl edit ID --stdin`: replaces the item with standard input.
fn edit_stdin(store: &Store, id: &str, dry_run: bool) -> anyhow::Result<Outcome> {
    let (_, notes) = load(store)?;
    let item = notes.item_of(id)?;
    srl_core::replace(store, &notes, item, &stdin()?, dry_run)?;
    Ok(Outcome::Done)
}

/// `srl accept`: removes `draft` from the items of the matching draft
/// cards, and prints their IDs.
fn accept(store: &Store, selector: &Selector, dry_run: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let mut seen = HashSet::new();
    let items = select(&loaded, selector)
        .into_iter()
        .map(|(item, _)| item)
        .filter(|item| item.is_draft() && seen.insert(item.id().clone()))
        .collect::<Vec<_>>();
    let written = srl_core::accept(store, &loaded.notes, &items, dry_run)?;
    let ids = written.iter().map(|done| &done.id).collect::<Vec<_>>();
    output::ids(&ids);
    Ok(found_anything(!written.is_empty()))
}

/// `srl suspend` and `srl unsuspend`: prints the IDs of the cards that
/// changed.
fn suspend(
    store: &Store,
    selector: &Selector,
    suspend: bool,
    dry_run: bool,
) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let selected = select(&loaded, selector);
    let changed = srl_core::suspend(
        store,
        &selected,
        &loaded.memories,
        suspend,
        &loaded.now,
        dry_run,
    )?;
    output::ids(&changed);
    Ok(found_anything(!changed.is_empty()))
}

/// `srl move FROM TO`: prints nothing, or with `--dry-run` the event.
fn move_history(store: &Store, from: &str, to: &str, dry_run: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let event = srl_core::move_history(
        store,
        &loaded.notes,
        &loaded.memories,
        &CardId::parse(from)?,
        &CardId::parse(to)?,
        &loaded.now,
        dry_run,
    )?;
    if dry_run {
        output::json_lines(&[event])?;
    }
    Ok(Outcome::Done)
}

/// `srl next`: the next card, if there is one.
fn next(store: &Store, selector: Selector, json: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let Some(card) = srl_core::next(&loaded.notes, &loaded.memories, selector, &loaded.config)
    else {
        return Ok(Outcome::Nothing);
    };
    if json {
        output::json_lines(&[card])?;
    } else {
        output::cards(&[card]);
    }
    Ok(Outcome::Done)
}

/// `srl review`: records a review from standard input, or of cards by ID.
/// Prints nothing, or with `--dry-run` the events it would write.
fn review(store: &Store, args: &ReviewArgs) -> anyhow::Result<Outcome> {
    let (_, notes) = load(store)?;
    let dry_run = args.dry_run.dry_run;
    let requests = if args.stdin {
        vec![ReviewRequest::parse(&stdin()?)?]
    } else {
        let rating = args
            .rating
            .context("--rating is required when reviewing cards by ID")?;
        let by = match args.by {
            By::Human => srl_core::By::Human,
            By::Agent => srl_core::By::Agent,
        };
        args.ids
            .iter()
            .map(|id| {
                Ok(ReviewRequest {
                    card: CardId::parse(id)?,
                    rating,
                    response: None,
                    by,
                })
            })
            .collect::<srl_core::Result<Vec<_>>>()?
    };
    let events = srl_core::review(store, &notes, &requests, &now()?, dry_run)?;
    if dry_run {
        output::json_lines(&events)?;
    }
    Ok(Outcome::Done)
}

/// `srl reset`: makes the matching cards that are not new new again, and
/// prints their IDs.
fn reset(store: &Store, selector: &Selector, dry_run: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let selected = select(&loaded, selector);
    let reset = srl_core::reset(store, &selected, &loaded.memories, &loaded.now, dry_run)?;
    output::ids(&reset);
    Ok(found_anything(!reset.is_empty()))
}

/// `srl stats`: cards per state, reviews today and recent retention, for
/// the matching cards. Exit 1 if no card matches.
fn stats(store: &Store, selector: &Selector, json: bool) -> anyhow::Result<Outcome> {
    let (config, notes) = load(store)?;
    let mut index = Index::open(store);
    let history = index.history(&config);
    report_cache(&mut index);
    let (history, problems) = history?;
    problems.iter().for_each(output::problem);
    let memories = Memories::new(history.memories, &config, today(&config)?);
    let selected = selector.select(notes.cards(), &memories);
    let stats = srl_core::stats(&selected, &memories, &history.reviews);
    if json {
        output::json_lines(&[&stats])?;
    } else {
        output::stats(&stats);
    }
    Ok(found_anything(!selected.is_empty()))
}

/// `srl log`: the events of the matching cards, in log order. It needs every
/// event, so it reads the log files rather than the index cache.
fn log(store: &Store, selector: &Selector, json: bool) -> anyhow::Result<Outcome> {
    let (config, notes) = load(store)?;
    let log = Log::load(store, &config.schedule)?;
    log.problems.iter().for_each(output::problem);
    let memories = Memories::replay(&log, &config, today(&config)?)?;
    let ids = selector
        .select(notes.cards(), &memories)
        .into_iter()
        .map(|(_, card)| card.id())
        .collect::<Vec<_>>();
    let events = log.of(&ids).map(|entry| &entry.event).collect::<Vec<_>>();
    if json {
        output::json_lines(&events)?;
    } else {
        output::events(&events);
    }
    Ok(found_anything(!events.is_empty()))
}

/// The local time. Fails if the system's time zone can't be found, rather
/// than guessing UTC.
pub(crate) fn now() -> anyhow::Result<Zoned> {
    let zone = jiff::tz::TimeZone::try_system().context(
        "cannot find the system's time zone, which decides the study day; set the TZ environment variable",
    )?;
    Ok(jiff::Timestamp::now().to_zoned(zone))
}

/// Today's study day.
fn today(config: &Config) -> anyhow::Result<Date> {
    Ok(config.schedule.study_day(now()?.datetime()))
}

/// All of standard input.
fn stdin() -> anyhow::Result<String> {
    io::read_to_string(io::stdin()).context("cannot read standard input")
}

/// The random seed new item IDs are made from.
fn seed() -> anyhow::Result<u64> {
    getrandom::u64().context("cannot get random numbers for new IDs")
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
