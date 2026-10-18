//! The only place that writes to standard output and standard error.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "this module is where output happens"
)]

use std::path::PathBuf;

use serde::Serialize;

/// Prints one path per line.
pub(crate) fn paths(paths: &[PathBuf]) {
    for path in paths {
        println!("{}", path.display());
    }
}

/// Prints one ID per line.
pub(crate) fn ids(ids: &[srl_core::Id]) {
    for id in ids {
        println!("{id}");
    }
}

/// Prints the IDs of added atoms, one per line. With `dry_run`, also prints
/// the file each would be written to.
pub(crate) fn added(added: &[srl_core::Added], dry_run: bool) {
    for atom in added {
        if dry_run {
            println!("{}  atoms/{}", atom.id, atom.path);
        } else {
            println!("{}", atom.id);
        }
    }
}

/// Prints one line per atom: ID, title and tags, separated by two spaces.
pub(crate) fn atom_lines(atoms: &[&srl_core::Atom]) {
    for atom in atoms {
        let content = atom.content();
        let title = content
            .title()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let line = format!("{}  {title}  {}", atom.id(), content.tags().join(" "));
        println!("{}", line.trim_end());
    }
}

/// Prints one line per tag: the count, right-aligned, and the tag.
pub(crate) fn tag_counts(counts: &[srl_core::TagCount]) {
    let width = counts
        .iter()
        .map(|count| count.count.to_string().len())
        .max()
        .unwrap_or(0);
    for count in counts {
        println!("{:>width$}  {}", count.count, count.tag);
    }
}

/// Prints one line per drill: name, where it was found (`built-in` or the
/// program's path) and summary, separated by two spaces.
pub(crate) fn drills(drills: &[(&srl_core::Drill, Option<String>)]) {
    for (drill, summary) in drills {
        let place = drill
            .path()
            .map_or_else(|| "built-in".to_owned(), |path| path.display().to_string());
        let line = format!(
            "{}  {place}  {}",
            drill.name(),
            summary.as_deref().unwrap_or("")
        );
        println!("{}", line.trim_end());
    }
}

/// Prints one JSON line per drill: `{"name", "path", "summary"}`, with
/// `path` `null` for a built-in drill and `summary` `null` if its help
/// failed.
pub(crate) fn drills_json(drills: &[(&srl_core::Drill, Option<String>)]) -> anyhow::Result<()> {
    for (drill, summary) in drills {
        let line = serde_json::json!({
            "name": drill.name(),
            "path": drill.path(),
            "summary": summary,
        });
        println!("{}", serde_json::to_string(&line)?);
    }
    Ok(())
}

/// Prints cards for people: for each, a line with its ref and grading mode,
/// the prompt, the choices as a numbered list if it has any, a line `---`,
/// and the answer. Cards are separated by blank lines.
pub(crate) fn cards(cards: &[srl_core::Card]) {
    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}  {}", card.reference(), card.grading().name());
        println!("{}", card.prompt().trim_end());
        if let srl_core::Grading::Choice { choices, .. } = card.grading() {
            for (number, choice) in choices.iter().enumerate() {
                println!("{}. {choice}", number + 1);
            }
        }
        println!("---");
        println!("{}", card.answer().trim_end());
    }
}

/// Prints one line per log event, fields separated by two spaces: the time,
/// the atom, and `reset`, or `review`, the rating, the card's ref (`-`
/// without a card) and who reviewed.
pub(crate) fn events(events: &[&srl_core::Event]) {
    for event in events {
        match event {
            srl_core::Event::Review(review) => {
                let reference = review
                    .card
                    .as_ref()
                    .map_or("-", |card| card.reference.as_str());
                let by = match review.by {
                    srl_core::By::Human => "human",
                    srl_core::By::Agent => "agent",
                };
                println!(
                    "{}  {}  review  {}  {reference}  {by}",
                    review.t, review.atom, review.rating
                );
            }
            srl_core::Event::Reset(reset) => println!("{}  {}  reset", reset.t, reset.atom),
        }
    }
}

/// Prints statistics, one `NAME  VALUE` line each. Retention is a
/// percentage with the number of reviews it comes from, or `-` without
/// reviews.
pub(crate) fn stats(report: &srl_core::Stats) {
    let states = &report.states;
    for (name, value) in [
        ("atoms", report.atoms),
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

/// Prints one line per session: its name and number of cards.
pub(crate) fn sessions(sessions: &[srl_core::SessionInfo]) {
    for session in sessions {
        println!("{}  {}", session.name, session.cards);
    }
}

/// Prints one line per atom that got an ID: the ID and its file.
pub(crate) fn fixed(fixed: &[srl_core::Fixed]) {
    for atom in fixed {
        println!("{}  atoms/{}", atom.id, atom.path);
    }
}

/// Prints whole files, separated by blank lines.
pub(crate) fn files<'a>(texts: impl Iterator<Item = &'a str>) {
    for (index, text) in texts.enumerate() {
        if index > 0 {
            println!();
        }
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
    }
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
