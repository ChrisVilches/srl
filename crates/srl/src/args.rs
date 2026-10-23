//! Command-line arguments (see `docs/cli.md`).
//!
//! This module only describes the arguments. It holds no logic.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "srl",
    version,
    about = "Spaced repetition for the command line"
)]
pub(crate) struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = "The store to use (default: SRL_DIR, then the nearest store, then srl in the platform data directory)"
    )]
    pub(crate) dir: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Command,
}

/// The id of the selector argument; `argv` uses it to find commands that take one.
pub(crate) const SELECTOR: &str = "terms";

/// A selector, given as any number of arguments that are joined with spaces.
///
/// Terms may start with `-` (negation, as in `-in:n5`); `argv` makes sure
/// they reach this argument.
#[derive(Debug, Args)]
pub(crate) struct Selector {
    #[arg(
        id = SELECTOR,
        value_name = "SELECTOR",
        help = "Terms such as in:a,b -state:new or text; joined with spaces"
    )]
    pub(crate) terms: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct Json {
    /// Print JSON (JSONL for lists) instead of text for people.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DryRun {
    /// Show what would change, and change nothing.
    #[arg(long)]
    pub(crate) dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create a store.
    Init {
        dir: Option<PathBuf>,
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// Check the notes, the configuration and the log.
    Check,
    /// Look for text that was probably meant to be an item.
    Lint,
    /// Give an ID to every item without one.
    Id {
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// Rebuild the index cache.
    Reindex,
    /// List matching cards.
    Find {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        json: Json,
    },
    /// Print items, with their context.
    Show {
        /// Card or item IDs.
        #[arg(required = true)]
        ids: Vec<String>,
        /// How to show the context (default: `context` in .srl/config.toml).
        #[arg(long, value_enum, value_name = "MODE")]
        context: Option<ContextMode>,
        #[command(flatten)]
        json: Json,
    },
    /// Print the cards of items or cards.
    Cards {
        /// Card or item IDs.
        #[arg(required = true)]
        ids: Vec<String>,
        #[command(flatten)]
        json: Json,
    },
    /// Append items (JSONL of fields from standard input) to a note.
    Add(AddArgs),
    /// Open an item's note in $EDITOR, or replace the item with standard input.
    Edit {
        /// A card or item ID.
        id: String,
        /// Replace the item with standard input, which must be one item with the same ID.
        #[arg(long)]
        stdin: bool,
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// List drafts.
    Drafts {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        json: Json,
    },
    /// Remove `draft` from the items of matching draft cards.
    Accept {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// Suspend matching active cards.
    Suspend {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// Make matching suspended cards active again.
    Unsuspend {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// Give the history of an orphaned card ID to a card.
    Move {
        from: String,
        to: String,
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// Study in the terminal.
    Study {
        #[command(flatten)]
        selector: Selector,
    },
    /// Print the next card. Exits with 1 if there is none.
    Next {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        json: Json,
    },
    /// Record a review, from JSON on standard input or for cards by ID.
    Review(ReviewArgs),
    /// Make matching cards new again.
    Reset {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        dry_run: DryRun,
    },
    /// Cards per state, reviews today and recent retention.
    Stats {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        json: Json,
    },
    /// Print the log events of matching cards.
    Log {
        #[command(flatten)]
        selector: Selector,
        #[command(flatten)]
        json: Json,
    },
    /// `srl NAME ARGS…` runs `srl-NAME ARGS…` from PATH.
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    /// The note, relative to the store's folder; created if it doesn't exist.
    pub(crate) note: String,
    /// Read the items from standard input: one JSON object of fields per line.
    #[arg(long, required = true)]
    pub(crate) stdin: bool,
    /// The kind of the new items.
    #[arg(long, value_name = "NAME")]
    pub(crate) kind: Option<String>,
    /// The note shown as the new items' context.
    #[arg(long, value_name = "PATH")]
    pub(crate) see: Option<String>,
    /// Add the items as drafts.
    #[arg(long)]
    pub(crate) draft: bool,
    #[command(flatten)]
    pub(crate) dry_run: DryRun,
}

#[derive(Debug, Args)]
pub(crate) struct ReviewArgs {
    /// Read `{"card": "ID", "rating": N, "response": "…", "by": "human"}` from standard input.
    #[arg(long, conflicts_with_all = ["ids", "rating"])]
    pub(crate) stdin: bool,
    /// Cards to give the rating.
    #[arg(required_unless_present = "stdin", requires = "rating")]
    pub(crate) ids: Vec<String>,
    /// The rating: 1 Again, 2 Hard, 3 Good, 4 Easy.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
    pub(crate) rating: Option<u8>,
    /// Who recorded the review.
    #[arg(long, value_enum, default_value_t = By::Human, conflicts_with = "stdin")]
    pub(crate) by: By,
    #[command(flatten)]
    pub(crate) dry_run: DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum By {
    Human,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ContextMode {
    Prose,
    Full,
    None,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("srl").chain(args.iter().copied()))
    }

    #[test]
    fn definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn dir_is_accepted_after_the_command() {
        let cli = parse(&["find", "--dir", "/s"]).unwrap();
        assert_eq!(cli.dir, Some(PathBuf::from("/s")));
    }

    #[test]
    fn rating_must_be_between_1_and_4() {
        assert!(parse(&["review", "a7f3k2", "--rating", "5"]).is_err());
        assert!(parse(&["review", "a7f3k2", "--rating", "4"]).is_ok());
    }

    #[test]
    fn review_needs_ids_or_stdin() {
        assert!(parse(&["review"]).is_err());
        assert!(parse(&["review", "--stdin"]).is_ok());
    }

    #[test]
    fn add_needs_stdin() {
        assert!(parse(&["add", "a.md"]).is_err());
        assert!(parse(&["add", "a.md", "--stdin", "--see", "b.md", "--draft"]).is_ok());
    }

    #[test]
    fn unknown_commands_are_external() {
        let cli = parse(&["web", "--port", "8080"]).unwrap();
        let Command::External(args) = cli.command else {
            panic!("expected an external command, got {:?}", cli.command);
        };
        assert_eq!(args, ["web", "--port", "8080"]);
    }
}
