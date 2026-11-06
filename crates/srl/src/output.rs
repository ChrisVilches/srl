//! The only place that writes to standard output and standard error.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "this module is where output happens"
)]

use std::fmt::Display;
use std::path::PathBuf;

use serde::Serialize;
use srl_core::{
    By, Card, CardId, CollectionInfo, Event, Item, KindDescription, SourceDescription, Stats,
    Warning, Written,
};

/// Prints one path per line.
pub(crate) fn paths(paths: &[PathBuf]) {
    for path in paths {
        println!("{}", path.display());
    }
}

/// Prints one path of the store per line.
pub(crate) fn store_paths(paths: &[String]) {
    for path in paths {
        println!("{path}");
    }
}

/// Prints one ID per line.
pub(crate) fn ids<T: Display>(ids: &[T]) {
    for id in ids {
        println!("{id}");
    }
}

/// Prints the address of each collection, one per line.
pub(crate) fn addresses(collections: &[CollectionInfo]) {
    for collection in collections {
        println!("{}", collection.address);
    }
}

/// Prints one line per changed item: its ID and the address of its
/// collection, separated by two spaces.
pub(crate) fn written(written: &[Written]) {
    for done in written {
        println!("{}  {}", done.id, done.address);
    }
}

/// Prints one line per card: its ID, the address of its item's
/// collection, and the first line of the item's first field, separated by
/// two spaces.
pub(crate) fn card_lines(cards: &[(&Item, &CardId)]) {
    for (item, card) in cards {
        let line = format!("{}  {}  {}", card, item.address(), item.label());
        println!("{}", line.trim_end());
    }
}

/// Prints items for people: for each, a line with the address of its
/// collection, the item as written, and, with `context`, if the item
/// names a note with `see`, a line `---` with the note's path, then its
/// text, if it has any. Items are separated by blank lines.
pub(crate) fn items(items: &[(&Item, Option<String>)], context: bool) {
    for (index, (item, text)) in items.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}", item.address());
        println!("{}", item.text());
        if let (true, Some(see)) = (context, item.see()) {
            println!("--- {see}");
            if let Some(text) = text {
                print!("{text}");
            }
        }
    }
}

/// Prints cards for people: for each, a line with its ID, the prompt, a
/// line `---`, the revealed prompt (the prompt again when there is none),
/// and, if there is an answer, a line `---` and the answer. Cards are
/// separated by blank lines.
pub(crate) fn cards(cards: &[Card]) {
    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}", card.id());
        println!("{}", card.prompt().trim_end());
        println!("---");
        let revealed = card.prompt_revealed().unwrap_or(card.prompt());
        println!("{}", revealed.trim_end());
        if let Some(answer) = card.answer() {
            println!("---");
            println!("{}", answer.trim_end());
        }
    }
}

/// Prints one line per log event, fields separated by two spaces: the time,
/// the card, and what happened (see `docs/cli.md`, "Analysis").
pub(crate) fn events(events: &[&Event]) {
    for event in events {
        let what = match event {
            Event::Review(review) => {
                let by = match review.by {
                    By::Human => "human",
                    By::Agent => "agent",
                };
                format!("review  {}  {by}", review.rating)
            }
            Event::Reset(_) => "reset".to_owned(),
            Event::Suspend(suspended) => match &suspended.reason {
                Some(reason) => format!("suspend  {}", reason.lines().next().unwrap_or("")),
                None => "suspend".to_owned(),
            },
            Event::Unsuspend(_) => "unsuspend".to_owned(),
            Event::Move(moved) => format!("move  {}", moved.from),
        };
        println!("{}  {}  {what}", event.t(), event.card());
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
    ] {
        println!("{name}  {value}");
    }
    match report.retention {
        Some(retention) => println!(
            "retention  {:.1}% of {} reviews in 30 days",
            retention * 100.0,
            report.retention_reviews
        ),
        None => println!("retention  -"),
    }
}

/// Prints kinds for people: for each, a line with its name and
/// description, then a line per field, indented by two spaces, with its
/// name, its label in parentheses if it has one, and its description. Names
/// and descriptions are separated by two spaces.
pub(crate) fn kinds(kinds: &[KindDescription]) {
    for kind in kinds {
        println!("{}  {}", kind.name, kind.description);
        for field in &kind.fields {
            match &field.label {
                Some(label) => println!("  {} ({label})  {}", field.name, field.description),
                None => println!("  {}  {}", field.name, field.description),
            }
        }
    }
}

/// Prints sources for people: for each, a line with its name and
/// description, then a line for its driver and its globs, indented by two
/// spaces, each name and value separated by two spaces.
pub(crate) fn sources(sources: &[SourceDescription]) {
    for source in sources {
        println!("{}  {}", source.name, source.description);
        println!("  driver  {}", source.driver);
        println!("  include  {}", source.creates.include.join(" "));
        if !source.creates.exclude.is_empty() {
            println!("  exclude  {}", source.creates.exclude.join(" "));
        }
    }
}

/// Reports a likely mistake `srl check` found, on standard error, as
/// `warning: ADDRESS: item ID: message`.
pub(crate) fn warning(warning: &Warning) {
    eprintln!(
        "srl: warning: {}: item {}: {}",
        warning.address, warning.id, warning.message
    );
}

/// Prints text as it is.
pub(crate) fn text(text: &str) {
    print!("{text}");
}

/// Prints each item as one line of JSON.
pub(crate) fn json_lines<T: Serialize>(items: &[T]) -> anyhow::Result<()> {
    for item in items {
        println!("{}", serde_json::to_string(item)?);
    }
    Ok(())
}

/// Reports a failure the command carries on after, on standard error.
pub(crate) fn problem(error: &srl_core::Error) {
    eprintln!("srl: {error}");
}

/// Prints an error as one plain message on standard error.
pub(crate) fn error(error: &anyhow::Error) {
    eprintln!("srl: {error:#}");
}
