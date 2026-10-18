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

/// Prints one line per atom: ID, cue and tags, separated by two spaces.
pub(crate) fn atom_lines(atoms: &[&srl_core::Atom]) {
    for atom in atoms {
        let content = atom.content();
        let cue = content
            .cue()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let line = format!("{}  {cue}  {}", atom.id(), content.tags().join(" "));
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
