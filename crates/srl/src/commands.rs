//! Runs the built-in commands.
//!
//! Each command gathers its inputs, calls `srl-core` and hands the result to
//! `output`. Memories are read through the index cache.

use std::collections::HashSet;
use std::env;
use std::io;
use std::path::PathBuf;

use anyhow::Context;
use jiff::Zoned;
use jiff::civil::Date;
use serde::Serialize;
use srl_core::{
    Card, CardId, Catalog, Config, Error, Index, Item, ItemId, Log, Lookup, Memories, Place,
    Selector, Status, Store,
};

use crate::args::{AddArgs, Cli, Command, ReviewArgs};
use crate::{Outcome, output};

pub(crate) fn run(cli: Cli) -> anyhow::Result<Outcome> {
    let store = || -> anyhow::Result<Store> { Ok(Store::open(&lookup(cli.dir.clone())?)?) };
    let selector = |terms: &[String]| -> anyhow::Result<Selector> {
        Ok(Selector::parse(
            &terms.join(" "),
            &Config::load(&store()?)?,
        )?)
    };
    match cli.command {
        Command::Init { dir, dry_run } => init(cli.dir, dir, dry_run.dry_run),
        Command::Check => check(&store()?),
        Command::Id { dry_run } => id(&store()?, dry_run.dry_run),
        Command::Collections { json } => collections(&store()?, json.json),
        Command::Notes { json } => notes(&store()?, json.json),
        Command::Kinds { json } => kinds(&store()?, json.json),
        Command::Sources { json } => sources(&store()?, json.json),
        Command::Find {
            selector: terms,
            json,
        } => find(&store()?, &selector(&terms.terms)?, json.json),
        Command::Show { ids, context, json } => {
            show(&store()?, &ids, !context.no_context, json.json)
        }
        Command::Cards { ids, json } => cards(&store()?, &ids, json.json),
        Command::Add(args) => add(&store()?, &args),
        Command::Edit { dry_run, .. } => edit(&store()?, dry_run.dry_run),
        Command::Move {
            ids,
            collection,
            dry_run,
        } => move_items(&store()?, &ids, &collection, dry_run.dry_run),
        Command::Accept {
            selector: terms,
            dry_run,
        } => accept(&store()?, &selector(&terms.terms)?, dry_run.dry_run),
        Command::Suspend {
            selector: terms,
            reason,
            dry_run,
        } => suspend(
            &store()?,
            &selector(&terms.terms)?,
            reason.as_deref(),
            dry_run.dry_run,
        ),
        Command::Unsuspend {
            selector: terms,
            dry_run,
        } => unsuspend(&store()?, &selector(&terms.terms)?, dry_run.dry_run),
        Command::Next {
            selector: terms,
            context,
            json,
        } => next(
            &store()?,
            &selector(&terms.terms)?,
            !context.no_context,
            json.json,
        ),
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
    catalog: Catalog,
    memories: Memories,
    now: Zoned,
}

/// [`load`], then the memories through the index cache, reporting the log
/// lines that were skipped.
fn open(store: &Store) -> anyhow::Result<Loaded> {
    let mut index = Index::open(store);
    let (config, catalog) = load_with(store, &mut index)?;
    let (memories, now, problems) = memories(&mut index, &config)?;
    problems.iter().for_each(output::problem);
    Ok(Loaded {
        config,
        catalog,
        memories,
        now,
    })
}

/// Reads the configuration and every item, and stops if the sources have
/// any problem (see `docs/items.md`, "Errors").
fn load(store: &Store) -> anyhow::Result<(Config, Catalog)> {
    load_with(store, &mut Index::open(store))
}

/// [`load`], with the answers of kind programs kept in `index`. Reports
/// any problem with the cache.
fn load_with(store: &Store, index: &mut Index) -> anyhow::Result<(Config, Catalog)> {
    let config = Config::load(store)?;
    let catalog = Catalog::load(store, &config, index);
    report_cache(index);
    Ok((config, strict(catalog?)?))
}

/// `catalog`, if it has no problems; otherwise reports every problem and
/// fails.
fn strict(catalog: Catalog) -> anyhow::Result<Catalog> {
    catalog.problems().iter().for_each(output::problem);
    let count = catalog.problems().len();
    if count > 0 {
        return Err(Error::ItemsInvalid { count }.into());
    }
    Ok(catalog)
}

/// Every card's memory on today's study day, through the index cache, the
/// current time, and the log lines that were skipped. Reports any problem
/// with the cache.
fn memories(index: &mut Index, config: &Config) -> anyhow::Result<(Memories, Zoned, Vec<Error>)> {
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

/// `srl check`: reports every problem, and orphans and likely mistakes as
/// notices. Fails if there is any problem. It asks kind programs about
/// every item, rather than using the answers kept in the index cache.
fn check(store: &Store) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    let mut index = Index::open(store);
    let catalog = Catalog::load_asking(store, &config, &mut index);
    report_cache(&mut index);
    let catalog = catalog?;
    let (memories, _, log_problems) = memories(&mut index, &config)?;
    let problems = catalog
        .problems()
        .iter()
        .chain(&log_problems)
        .collect::<Vec<_>>();
    problems.iter().copied().for_each(output::problem);
    srl_core::orphans(&catalog, &memories)
        .iter()
        .for_each(output::problem);
    srl_core::warnings(&catalog)
        .iter()
        .for_each(output::warning);
    Ok(if problems.is_empty() {
        Outcome::Done
    } else {
        Outcome::Failed
    })
}

/// `srl id`: gives an ID to every item without one, and prints them. Exit
/// 1 if every item had one.
fn id(store: &Store, dry_run: bool) -> anyhow::Result<Outcome> {
    let config = Config::load(store)?;
    let mut index = Index::open(store);
    let catalog = Catalog::load(store, &config, &mut index);
    report_cache(&mut index);
    let catalog = catalog?;
    catalog
        .problems()
        .iter()
        .filter(|problem| !matches!(problem, Error::MissingId { .. }))
        .for_each(output::problem);
    let written = srl_core::assign_ids(&catalog, seed("for new IDs")?, dry_run)?;
    output::written(&written);
    Ok(found_anything(!written.is_empty()))
}

/// `srl collections`: lists every collection, without reading any item, so
/// errors in them don't stop it. Exit 1 if the store has none.
fn collections(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let found = srl_core::collections(store, &Config::load(store)?)?;
    if json {
        output::json_lines(&found)?;
    } else {
        output::collections(&found);
    }
    Ok(found_anything(!found.is_empty()))
}

/// `srl notes`: lists every file of study material, which items may name
/// with `see`. Exit 1 if the store has none.
fn notes(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let found = srl_core::notes(store, &Config::load(store)?)?;
    if json {
        let lines = found
            .iter()
            .map(|path| serde_json::json!({ "path": path }))
            .collect::<Vec<_>>();
        output::json_lines(&lines)?;
    } else {
        output::store_paths(&found);
    }
    Ok(found_anything(!found.is_empty()))
}

/// `srl sources`: describes every source, without reading any item.
fn sources(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let described = srl_core::sources(store, &Config::load(store)?)?;
    if json {
        output::json_lines(&described)?;
    } else {
        output::sources(&described);
    }
    Ok(Outcome::Done)
}

/// `srl kinds`: describes every kind, `qa` first, without reading any
/// item. Fails at once if a kind program can't describe its kind.
fn kinds(store: &Store, json: bool) -> anyhow::Result<Outcome> {
    let described = srl_core::kinds(store, &Config::load(store)?)?;
    if json {
        output::json_lines(&described)?;
    } else {
        output::kinds(&described);
    }
    Ok(Outcome::Done)
}

/// The cards that match `selector`, with their items.
fn select<'a>(loaded: &'a Loaded, selector: &Selector) -> Vec<(&'a Item, &'a CardId)> {
    selector.select(loaded.catalog.cards(), &loaded.memories)
}

/// `srl find`: lists the matching cards.
fn find(store: &Store, selector: &Selector, json: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let found = select(&loaded, selector);
    if json {
        let lines = found
            .iter()
            .map(|(item, card)| {
                let memories = &loaded.memories;
                serde_json::json!({
                    "id": card,
                    "item": item.id(),
                    "kind": item.kind().as_str(),
                    "see": item.see(),
                    "collection": item.collection(),
                    "source": item.source(),
                    "status": Status::of(item, card, memories),
                    "schedule": place_name(memories.place(card)),
                    "leech": memories.is_leech(card),
                    "due": memories.get(card).due.map(|day| day.to_string()),
                    "suspend_reason": memories.suspend_reason(card),
                })
            })
            .collect::<Vec<_>>();
        output::json_lines(&lines)?;
    } else {
        output::card_lines(&found);
    }
    Ok(found_anything(!found.is_empty()))
}

fn place_name(state: Place) -> &'static str {
    match state {
        Place::New => "new",
        Place::Due => "due",
        Place::Young => "young",
        Place::Mature => "mature",
    }
}

/// The items the IDs given to a command stand for, each once, in the order
/// given; and the IDs that name nothing, reported.
fn items_of<'a>(catalog: &'a Catalog, ids: &[String]) -> (Vec<&'a Item>, Vec<Error>) {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut problems = Vec::new();
    for id in ids {
        match catalog.item_of(id) {
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

/// `srl show ID…`: prints each item, with its context unless `context` is
/// false.
fn show(store: &Store, ids: &[String], context: bool, json: bool) -> anyhow::Result<Outcome> {
    let (_, catalog) = load(store)?;
    let (items, problems) = items_of(&catalog, ids);
    let contexts = if context {
        catalog.contexts(&items)?
    } else {
        vec![None; items.len()]
    };
    let shown = items
        .into_iter()
        .zip(contexts)
        .map(|(item, context)| item.json(context.as_deref()))
        .collect::<Vec<_>>();
    if json {
        output::json_lines(&shown)?;
    } else {
        output::items(&shown);
    }
    Ok(reported(&problems))
}

/// `srl cards ID…`: the cards of each item ID, or the card of each card ID.
fn cards(store: &Store, ids: &[String], json: bool) -> anyhow::Result<Outcome> {
    let (config, catalog) = load(store)?;
    let day = today(&config)?;
    let mut problems = Vec::new();
    let mut found = Vec::new();
    for id in ids {
        match catalog
            .item_of(id)
            .and_then(|item| Ok((item, CardId::parse(id)?)))
        {
            Ok((item, card_id)) => found.extend(
                item.cards()
                    .iter()
                    .filter(|card| card_id.suffix().is_none() || **card == card_id)
                    .map(|card| (item, card)),
            ),
            Err(error) => problems.push(error),
        }
    }
    // A card that can't be made fails the whole command before anything is
    // printed (see `docs/cli.md`, "Cards").
    let found = catalog.make_cards(&found, day)?;
    if json {
        output::json_lines(&found)?;
    } else {
        output::cards(&found);
    }
    Ok(reported(&problems))
}

/// `srl add COLLECTION --stdin`: adds items to a collection, and prints
/// their IDs, or with `--dry-run` the items as their source would add them.
fn add(store: &Store, args: &AddArgs) -> anyhow::Result<Outcome> {
    let (_, catalog) = load(store)?;
    let input = stdin()?;
    let dry_run = args.dry_run.dry_run;
    let added = srl_core::add(
        &catalog,
        &args.collection,
        &input,
        seed("for new IDs")?,
        dry_run,
    )?;
    if dry_run {
        output::text(&added.text);
    } else {
        output::ids(&added.ids);
    }
    Ok(found_anything(!added.ids.is_empty()))
}

/// `srl edit --stdin`: replaces items with standard input, and prints
/// their IDs.
fn edit(store: &Store, dry_run: bool) -> anyhow::Result<Outcome> {
    let (_, catalog) = load(store)?;
    let ids = srl_core::edit(&catalog, &stdin()?, dry_run)?;
    output::ids(&ids);
    Ok(found_anything(!ids.is_empty()))
}

/// `srl move ID... COLLECTION`: moves the items, and prints the IDs of
/// those moved.
fn move_items(
    store: &Store,
    ids: &[String],
    collection: &str,
    dry_run: bool,
) -> anyhow::Result<Outcome> {
    let ids = ids
        .iter()
        .map(|id| ItemId::parse(id))
        .collect::<srl_core::Result<Vec<_>>>()?;
    let (_, catalog) = load(store)?;
    let moved = srl_core::move_items(&catalog, &ids, collection, dry_run)?;
    output::ids(&moved);
    Ok(found_anything(!moved.is_empty()))
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
    let written = srl_core::accept(&loaded.catalog, &items, dry_run)?;
    let ids = written.iter().map(|done| &done.id).collect::<Vec<_>>();
    output::ids(&ids);
    Ok(found_anything(!written.is_empty()))
}

/// `srl suspend`: prints the IDs of the cards that changed. An empty
/// reason is no reason.
fn suspend(
    store: &Store,
    selector: &Selector,
    reason: Option<&str>,
    dry_run: bool,
) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let selected = select(&loaded, selector);
    let changed = srl_core::suspend(
        store,
        &selected,
        &loaded.memories,
        reason.filter(|reason| !reason.is_empty()),
        &loaded.now,
        dry_run,
    )?;
    output::ids(&changed);
    Ok(found_anything(!changed.is_empty()))
}

/// `srl unsuspend`: prints the IDs of the cards that changed.
fn unsuspend(store: &Store, selector: &Selector, dry_run: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let selected = select(&loaded, selector);
    let changed = srl_core::unsuspend(store, &selected, &loaded.memories, &loaded.now, dry_run)?;
    output::ids(&changed);
    Ok(found_anything(!changed.is_empty()))
}

/// `srl next`: the next card, if there is one. With `--json`, the card
/// with what a frontend shows around it: its state, and its item's kind,
/// collection and its context, unless `context` is false (see
/// `docs/cli.md`, "Studying").
fn next(store: &Store, selector: &Selector, context: bool, json: bool) -> anyhow::Result<Outcome> {
    let loaded = open(store)?;
    let next = srl_core::next(
        &loaded.catalog,
        &loaded.memories,
        selector,
        &loaded.config,
        loaded.memories.today(),
        seed("to pick the next card")?,
    )?;
    let Some((item, card)) = next else {
        return Ok(Outcome::Nothing);
    };
    if json {
        /// The card's own fields, then what is shown around it.
        #[derive(Serialize)]
        struct Next<'a> {
            #[serde(flatten)]
            card: &'a Card,
            schedule: &'static str,
            leech: bool,
            kind: &'a str,
            collection: &'a str,
            source: &'a str,
            see: Option<&'a str>,
            context: Option<String>,
        }
        let context = if context {
            loaded.catalog.context(item)?
        } else {
            None
        };
        output::json_lines(&[Next {
            card: &card,
            schedule: place_name(loaded.memories.place(card.id())),
            leech: loaded.memories.is_leech(card.id()),
            kind: item.kind().as_str(),
            collection: item.collection(),
            source: item.source(),
            see: item.see(),
            context,
        }])?;
    } else {
        output::cards(&[card]);
    }
    Ok(Outcome::Done)
}

/// `srl review`: records a rating for a card. Prints nothing, or with
/// `--dry-run` the event it would write.
fn review(store: &Store, args: &ReviewArgs) -> anyhow::Result<Outcome> {
    let (_, catalog) = load(store)?;
    let dry_run = args.dry_run.dry_run;
    let card = CardId::parse(&args.id)?;
    let event = srl_core::review(store, &catalog, &card, args.rating, &now()?, dry_run)?;
    if dry_run {
        output::json_lines(&[event])?;
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
    let mut index = Index::open(store);
    let (config, catalog) = load_with(store, &mut index)?;
    let history = index.history(&config);
    report_cache(&mut index);
    let (history, problems) = history?;
    problems.iter().for_each(output::problem);
    let memories = Memories::new(history.memories, &config, today(&config)?);
    let selected = selector.select(catalog.cards(), &memories);
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
    let (config, catalog) = load(store)?;
    let log = Log::load(store, &config.schedule)?;
    log.problems.iter().for_each(output::problem);
    let memories = Memories::replay(&log, &config, today(&config)?)?;
    let ids = selector
        .select(catalog.cards(), &memories)
        .into_iter()
        .map(|(_, card)| card)
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
fn now() -> anyhow::Result<Zoned> {
    let zone = jiff::tz::TimeZone::try_system().context(
        "cannot find the system's time zone, which decides the study day; set the TZ environment variable",
    )?;
    let instant = match env_value("SRL_NOW")? {
        None => jiff::Timestamp::now(),
        Some(value) => value
            .parse()
            .with_context(|| format!("SRL_NOW is not a time like 2026-01-05T09:00:00Z: {value}"))?,
    };
    Ok(instant.to_zoned(zone))
}

/// Today's study day.
fn today(config: &Config) -> anyhow::Result<Date> {
    Ok(config.schedule.study_day(now()?.datetime()))
}

/// All of standard input.
fn stdin() -> anyhow::Result<String> {
    io::read_to_string(io::stdin()).context("cannot read standard input")
}

/// A random seed: `SRL_SEED` if it is set, else random numbers.
fn seed(purpose: &str) -> anyhow::Result<u64> {
    match env_value("SRL_SEED")? {
        None => getrandom::u64().with_context(|| format!("cannot get random numbers {purpose}")),
        Some(value) => value.parse().with_context(|| {
            format!("SRL_SEED is not a whole number from 0 to 18446744073709551615: {value}")
        }),
    }
}

/// The value of an environment variable, or `None` if it is unset or empty.
fn env_value(name: &str) -> anyhow::Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value).filter(|value| !value.is_empty())),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => anyhow::bail!("{name} is not valid Unicode"),
    }
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
