//! The only place that writes to standard output and standard error.

use std::fmt::Display;
use std::io::{self, Write as _};
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use srl_core::{
    Card, CardId, CollectionInfo, Event, Item, KindDescription, SourceDescription, Stats, Warning,
    Written,
};

/// A line on standard output, as `println!` writes it, but ending `srl`
/// when it can't be written (see [`wrote`]) instead of panicking.
macro_rules! out {
    ($($arg:tt)*) => {
        wrote(writeln!(io::stdout().lock(), $($arg)*), Stream::Output)
    };
}

/// A line on standard error, as `eprintln!` writes it, but ending `srl`
/// when it can't be written (see [`wrote`]) instead of panicking.
macro_rules! err {
    ($($arg:tt)*) => {
        wrote(writeln!(io::stderr().lock(), $($arg)*), Stream::Error)
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Output,
    Error,
}

/// Ends `srl` if `result`, a write to `stream`, failed: when whoever reads
/// it closed it, such as `head` in `srl find | head -1`, quietly, and
/// otherwise saying why on standard error, if it can. Either way, the
/// programs it runs are stopped, and the exit code is 2 (see `docs/cli.md`,
/// on exit codes).
fn wrote(result: io::Result<()>, stream: Stream) {
    let Err(error) = result else {
        return;
    };
    if stream == Stream::Output && error.kind() != io::ErrorKind::BrokenPipe {
        let _unwritable = writeln!(
            io::stderr().lock(),
            "srl: cannot write to standard output: {error}"
        );
    }
    srl_core::stop_programs();
    #[expect(clippy::exit, reason = "nothing more can be written")]
    std::process::exit(2);
}

/// Prints one path per line.
pub(crate) fn paths(paths: &[PathBuf]) {
    for path in paths {
        out!("{}", path.display());
    }
}

/// Prints one path of the store per line.
pub(crate) fn store_paths(paths: &[String]) {
    for path in paths {
        out!("{path}");
    }
}

/// Prints one ID per line.
pub(crate) fn ids<T: Display>(ids: &[T]) {
    for id in ids {
        out!("{id}");
    }
}

/// Prints the name of each collection, one per line.
pub(crate) fn collections(collections: &[CollectionInfo]) {
    for collection in collections {
        out!("{}", collection.collection);
    }
}

/// Prints one line per changed item: its ID and its
/// collection, separated by two spaces.
pub(crate) fn written(written: &[Written]) {
    for done in written {
        out!("{}  {}", done.id, done.collection);
    }
}

/// Prints one line per card: its ID, its item's
/// collection, and the first line of the item's first field, separated by
/// two spaces.
pub(crate) fn card_lines(cards: &[(&Item, &CardId)]) {
    for (item, card) in cards {
        let line = format!("{}  {}  {}", card, item.collection(), item.label());
        out!("{}", line.trim_end());
    }
}

/// Prints items for people (see `docs/cli.md`, "Items and cards"): each
/// key of its item object, as `srl show --json` gives it, as `KEY: value`.
/// Items are separated by blank lines.
pub(crate) fn items(items: &[Value]) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out!();
        }
        for (key, value) in item.as_object().into_iter().flatten() {
            let value = match value {
                Value::Null => String::new(),
                Value::String(text) => text.clone(),
                Value::Array(values) => values
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map_or_else(|| value.to_string(), str::to_owned)
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                other @ (Value::Bool(_) | Value::Number(_) | Value::Object(_)) => other.to_string(),
            };
            entry(key, &value);
        }
    }
}

/// Prints `NAME: value`, or, for a value of several lines, `NAME:` and
/// then each line indented by two spaces.
fn entry(name: &str, value: &str) {
    if value.contains('\n') {
        out!("{name}:");
        for line in value.lines() {
            out!("{}", format!("  {line}").trim_end());
        }
    } else {
        out!("{}", format!("{name}: {value}").trim_end());
    }
}

/// Prints cards for people: for each, a line with its ID, the prompt, a
/// line `---`, the revealed prompt (the prompt again when there is none),
/// and, if there is an answer, a line `---` and the answer. Cards are
/// separated by blank lines.
pub(crate) fn cards(cards: &[Card]) {
    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            out!();
        }
        out!("{}", card.id());
        out!("{}", card.prompt().trim_end());
        out!("---");
        let revealed = card.prompt_revealed().unwrap_or(card.prompt());
        out!("{}", revealed.trim_end());
        if let Some(answer) = card.answer() {
            out!("---");
            out!("{}", answer.trim_end());
        }
    }
}

/// Prints one line per log event, fields separated by two spaces: the time,
/// the card, and what happened (see `docs/cli.md`, "Analysis").
pub(crate) fn events(events: &[&Event]) {
    for event in events {
        let what = match event {
            Event::Review(review) => format!("review  {}", review.rating),
            Event::Reset(_) => "reset".to_owned(),
            Event::Suspend(suspended) => match &suspended.reason {
                Some(reason) => format!("suspend  {}", reason.lines().next().unwrap_or("")),
                None => "suspend".to_owned(),
            },
            Event::Unsuspend(_) => "unsuspend".to_owned(),
            Event::Move(moved) => format!("move  {}", moved.from),
        };
        out!("{}  {}  {what}", event.t(), event.card());
    }
}

/// Prints statistics, one `NAME  VALUE` line each. Retention is a
/// percentage with the number of reviews it comes from, or `-` without
/// reviews.
pub(crate) fn stats(report: &Stats) {
    let schedule = &report.schedule;
    for (name, value) in [
        ("cards", report.cards),
        ("new", schedule.new),
        ("due", schedule.due),
        ("young", schedule.young),
        ("mature", schedule.mature),
        ("leech", report.leech),
        ("reviews today", report.reviews_today),
        ("new today", report.new_today),
        ("again today", report.again_today),
    ] {
        out!("{name}  {value}");
    }
    match report.retention {
        Some(retention) => out!(
            "retention  {:.1}% of {} reviews in 30 days",
            retention * 100.0,
            report.retention_reviews
        ),
        None => out!("retention  -"),
    }
}

/// Prints kinds for people: for each, a line with its name and
/// description, then a line per field, indented by two spaces, with its
/// name, its label in parentheses if it has one, and its description. Names
/// and descriptions are separated by two spaces.
pub(crate) fn kinds(kinds: &[KindDescription]) {
    for kind in kinds {
        out!("{}  {}", kind.name, kind.description);
        for field in &kind.fields {
            match &field.label {
                Some(label) => out!("  {} ({label})  {}", field.name, field.description),
                None => out!("  {}  {}", field.name, field.description),
            }
        }
    }
}

/// Prints sources for people: for each, a line with its name and
/// description, then a line for its driver and its globs, indented by two
/// spaces, each name and value separated by two spaces.
pub(crate) fn sources(sources: &[SourceDescription]) {
    for source in sources {
        out!("{}  {}", source.name, source.description);
        out!("  driver  {}", source.driver);
        out!("  include  {}", source.creates.include.join(" "));
        if !source.creates.exclude.is_empty() {
            out!("  exclude  {}", source.creates.exclude.join(" "));
        }
    }
}

/// Reports a likely mistake `srl check` found, on standard error, as
/// `warning: COLLECTION: item ID: message`.
pub(crate) fn warning(warning: &Warning) {
    err!(
        "srl: warning: {}: item {}: {}",
        warning.collection,
        warning.id,
        warning.message
    );
}

/// Prints text as it is.
pub(crate) fn text(text: &str) {
    let mut stdout = io::stdout().lock();
    wrote(
        stdout
            .write_all(text.as_bytes())
            .and_then(|()| stdout.flush()),
        Stream::Output,
    );
}

/// Prints each item as one line of JSON.
pub(crate) fn json_lines<T: Serialize>(items: &[T]) -> anyhow::Result<()> {
    for item in items {
        out!("{}", serde_json::to_string(item)?);
    }
    Ok(())
}

/// Reports a failure the command carries on after, on standard error.
pub(crate) fn problem(error: &srl_core::Error) {
    err!("srl: {error}");
}

/// Prints an error as one plain message on standard error.
pub(crate) fn error(error: &anyhow::Error) {
    err!("srl: {error:#}");
}
