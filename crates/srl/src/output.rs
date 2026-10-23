//! The only place that writes to standard output and standard error.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "this module is where output happens"
)]

use std::fmt::Display;
use std::path::PathBuf;

use serde::Serialize;
use srl_core::{By, Card, Event, Item, Lint, Stats, Written};

/// Prints one path per line.
pub(crate) fn paths(paths: &[PathBuf]) {
    for path in paths {
        println!("{}", path.display());
    }
}

/// Prints one ID per line.
pub(crate) fn ids<T: Display>(ids: &[T]) {
    for id in ids {
        println!("{id}");
    }
}

/// Prints one line per changed item: its ID and `PATH:LINE`, separated by
/// two spaces.
pub(crate) fn written(written: &[Written]) {
    for done in written {
        println!("{}  {}:{}", done.id, done.path, done.line);
    }
}

/// Prints one line per card: its ID, `PATH:LINE` of its item, and the
/// first line of the item's first field, separated by two spaces.
pub(crate) fn card_lines(cards: &[(&Item, &Card)]) {
    for (item, card) in cards {
        let line = format!(
            "{}  {}:{}  {}",
            card.id(),
            item.path(),
            item.line(),
            item.label()
        );
        println!("{}", line.trim_end());
    }
}

/// Prints items for people: for each, a line `PATH:LINE`, the item as
/// written, and the context after a line `---` if there is one. Items are
/// separated by blank lines.
pub(crate) fn items(items: &[(&Item, Option<String>)]) {
    for (index, (item, context)) in items.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}:{}", item.path(), item.line());
        println!("{}", item.text());
        if let Some(context) = context {
            println!("---");
            print!("{context}");
        }
    }
}

/// Prints cards for people: for each, a line with its ID, the prompt, a
/// line `---`, and the answer. Cards are separated by blank lines.
pub(crate) fn cards(cards: &[&Card]) {
    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}", card.id());
        println!("{}", card.prompt().trim_end());
        println!("---");
        println!("{}", card.answer().trim_end());
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
            Event::Suspend(_) => "suspend".to_owned(),
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
    let states = &report.states;
    for (name, value) in [
        ("cards", report.cards),
        ("new", states.new),
        ("due", states.due),
        ("young", states.young),
        ("mature", states.mature),
        ("leech", states.leech),
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

/// Prints what `srl lint` found, as `PATH:LINE: message`.
pub(crate) fn lints(found: &[Lint]) {
    for lint in found {
        println!("{}:{}: {}", lint.path, lint.line, lint.message);
    }
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

/// Prints a message about something the person should know, on standard
/// error.
#[cfg_attr(
    not(windows),
    expect(dead_code, reason = "only Windows reports that colors don't work")
)]
pub(crate) fn note(message: &str) {
    eprintln!("srl: {message}");
}
