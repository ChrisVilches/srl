//! Lets selector terms start with `-` without clap reading them as options.
//!
//! The rule: on a command that takes a selector, every argument that is not
//! one of the command's options (or an option's value) is a selector term,
//! except, before a `--`, one that starts with `--`, which clap reports.
//! The arguments are reordered so that the options come first and the terms
//! follow a `--`, which clap then reads as plain values. Which options exist,
//! and whether they take a value, comes from the clap definition in `args`.

use std::ffi::{OsStr, OsString};
use std::iter;

use clap::{CommandFactory, Parser};

use crate::args::{Cli, SELECTOR};

/// Parses the process arguments.
pub(crate) fn parse() -> Cli {
    Cli::parse_from(separate_selector(std::env::args_os().collect()))
}

/// One argument, or an option together with its value.
#[derive(Debug)]
enum Token {
    Option(Vec<OsString>),
    Other(OsString),
    /// A `--` given on the command line.
    Separator,
}

fn separate_selector(args: Vec<OsString>) -> Vec<OsString> {
    let mut cli = Cli::command();
    // Building propagates global options such as `--dir` to every subcommand.
    cli.build();
    let Some((program, rest)) = args.split_first() else {
        return args;
    };
    let top = tokens(&cli, rest.iter().cloned());
    let Some(position) = top
        .iter()
        .position(|token| matches!(token, Token::Other(_)))
    else {
        return args;
    };
    let (top_options, rest) = top.split_at(position);
    let Some(Token::Other(name)) = rest.first() else {
        return args;
    };
    let Some(subcommand) = name
        .to_str()
        .and_then(|name| cli.find_subcommand(name))
        .filter(|subcommand| takes_selector(subcommand))
    else {
        return args;
    };
    let sub = tokens(subcommand, rest.iter().skip(1).flat_map(flatten));
    let (sub_options, terms): (Vec<_>, Vec<_>) = sub
        .into_iter()
        .filter(|token| !matches!(token, Token::Separator))
        .partition(|token| matches!(token, Token::Option(_)));
    iter::once(program.clone())
        .chain(top_options.iter().flat_map(flatten))
        .chain(iter::once(name.clone()))
        .chain(sub_options.iter().flat_map(flatten))
        .chain(iter::once(OsString::from("--")))
        .chain(terms.iter().flat_map(flatten))
        .collect()
}

fn takes_selector(command: &clap::Command) -> bool {
    command
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == SELECTOR)
}

/// Splits arguments into tokens, as seen by `command`. Everything after a
/// `--` is an `Other`; the `--` itself is kept, so that splitting the
/// tokens again, for the subcommand, still sees it.
fn tokens(command: &clap::Command, args: impl Iterator<Item = OsString>) -> Vec<Token> {
    let mut args = args;
    let mut tokens = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--" {
            tokens.push(Token::Separator);
            tokens.extend(args.by_ref().map(Token::Other));
        } else if let Some(takes_value) = option_takes_value(command, &arg) {
            let value = if takes_value { args.next() } else { None };
            tokens.push(Token::Option(iter::once(arg).chain(value).collect()));
        } else if arg.as_encoded_bytes().starts_with(b"--") {
            // Not one of `command`'s options, such as a typo: kept with the
            // options, for clap to report, instead of becoming a term.
            tokens.push(Token::Option(vec![arg]));
        } else {
            tokens.push(Token::Other(arg));
        }
    }
    tokens
}

/// Whether `arg` is one of `command`'s options (`--name`, `--name=value` or
/// `-x`), and if so whether its value is the next argument.
fn option_takes_value(command: &clap::Command, arg: &OsStr) -> Option<bool> {
    let text = arg.to_str()?;
    let found = if let Some(long) = text.strip_prefix("--") {
        let (name, inline_value) = long
            .split_once('=')
            .map_or((long, false), |(name, _)| (name, true));
        command
            .get_arguments()
            .find(|candidate| candidate.get_long() == Some(name))
            .map(|candidate| (candidate, inline_value))
    } else {
        let short = text
            .strip_prefix('-')
            .filter(|rest| rest.chars().count() == 1)
            .and_then(|rest| rest.chars().next())?;
        command
            .get_arguments()
            .find(|candidate| candidate.get_short() == Some(short))
            .map(|candidate| (candidate, false))
    };
    found.map(|(option, inline_value)| !inline_value && option.get_action().takes_values())
}

fn flatten(token: &Token) -> Vec<OsString> {
    match token {
        Token::Option(args) => args.clone(),
        Token::Other(arg) => vec![arg.clone()],
        Token::Separator => vec![OsString::from("--")],
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::args::Command;

    fn try_parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let args = iter::once("srl").chain(args.iter().copied());
        Cli::try_parse_from(separate_selector(args.map(OsString::from).collect()))
    }

    fn parse(args: &[&str]) -> Cli {
        try_parse(args).unwrap()
    }

    /// Asserts that `args` is refused because of `unexpected`, and that
    /// nothing would run.
    fn refused(args: &[&str], unexpected: &str) {
        let Err(error) = try_parse(args) else {
            panic!("accepted {args:?}");
        };
        let message = error.to_string();
        assert!(
            message.contains(&format!("'{unexpected}'")),
            "{args:?}: {message}"
        );
    }

    /// Every command that takes a selector.
    const SELECTOR_COMMANDS: [&str; 8] = [
        "find",
        "next",
        "stats",
        "log",
        "accept",
        "suspend",
        "unsuspend",
        "reset",
    ];

    fn find(args: &[&str]) -> (Vec<String>, bool, Option<PathBuf>) {
        let cli = parse(args);
        let Command::Find { selector, json } = cli.command else {
            panic!("expected find, got {:?}", cli.command);
        };
        (selector.terms, json.json, cli.dir)
    }

    #[test]
    fn negated_terms_are_terms() {
        let (terms, json, _) = find(&["find", "in:japanese", "-in:n5", "-ものともせず"]);
        assert_eq!(terms, ["in:japanese", "-in:n5", "-ものともせず"]);
        assert!(!json);
    }

    #[test]
    fn options_after_negated_terms_are_options() {
        let (terms, json, dir) = find(&["find", "-in:a", "--json", "--dir", "/s", "b"]);
        assert_eq!(terms, ["-in:a", "b"]);
        assert!(json);
        assert_eq!(dir, Some(PathBuf::from("/s")));
    }

    #[test]
    fn options_before_the_command_are_kept() {
        let (terms, _, dir) = find(&["--dir=/s", "find", "-in:a"]);
        assert_eq!(terms, ["-in:a"]);
        assert_eq!(dir, Some(PathBuf::from("/s")));
    }

    #[test]
    fn option_values_are_not_terms() {
        let cli = parse(&["next", "-is:new", "--dir", "/store", "in:a"]);
        let Command::Next { selector, .. } = cli.command else {
            panic!("expected next, got {:?}", cli.command);
        };
        assert_eq!(selector.terms, ["-is:new", "in:a"]);
        assert_eq!(cli.dir, Some(PathBuf::from("/store")));
    }

    #[test]
    fn everything_after_a_separator_is_a_term() {
        let (terms, json, dir) = find(&["find", "--json", "--", "-h", "--dir", "/s"]);
        assert_eq!(terms, ["-h", "--dir", "/s"]);
        assert!(json);
        assert_eq!(dir, None);
    }

    #[test]
    fn a_mistyped_option_is_an_error_on_every_command_with_a_selector() {
        for command in SELECTOR_COMMANDS {
            for typo in [
                "--jsonn",
                "--dryrun",
                "--dry_run",
                "--dry-run-",
                "--JSON",
                "--Dry-Run",
                "--reasn",
                "---json",
                "--in:a",
                "--is:new",
                "--ものともせず",
            ] {
                refused(&[command, typo], typo);
                refused(&[command, "in:a", typo], typo);
                refused(&[command, typo, "in:a", "-is:new"], typo);
                refused(&[command, "-in:a", typo, "b", "--dir", "/s"], typo);
            }
        }
    }

    #[test]
    fn an_option_of_another_command_is_an_error() {
        refused(&["accept", "--json"], "--json");
        refused(&["find", "--dry-run"], "--dry-run");
        refused(&["reset", "--reason", "x"], "--reason");
        refused(&["next", "--rating", "3"], "--rating");
        refused(&["log", "--stdin"], "--stdin");
    }

    #[test]
    fn a_mistyped_option_with_an_inline_value_is_an_error() {
        refused(&["reset", "--reason=x", "in:a"], "--reason");
        refused(&["accept", "--dryrun=true"], "--dryrun");
        refused(&["find", "--=x"], "--");
        refused(&["find", "--json=x"], "x");
    }

    #[test]
    fn a_mistyped_option_is_not_mistaken_for_an_options_value() {
        // `--reasn` is no option, so "too long" is not its value: the typo
        // is reported, instead of suspending what contains "too" and "long".
        refused(&["suspend", "--reasn", "too long", "in:x"], "--reasn");
        // `--reason` takes the next argument as its value, but a value can't
        // start with `--`, so this fails too, instead of suspending in:a
        // with the reason "--dryrun".
        refused(&["suspend", "--reason", "--dryrun", "in:a"], "--dryrun");
    }

    #[test]
    fn a_mistyped_option_before_the_command_is_an_error() {
        refused(&["--jsonn", "find", "in:a"], "--jsonn");
        refused(&["--dryrun", "reset"], "--dryrun");
    }

    #[test]
    fn a_value_written_inline_may_start_with_dashes() {
        let cli = parse(&["suspend", "--reason=--x -in:y", "in:a"]);
        let Command::Suspend {
            selector, reason, ..
        } = cli.command
        else {
            panic!("expected suspend, got {:?}", cli.command);
        };
        assert_eq!(reason.as_deref(), Some("--x -in:y"));
        assert_eq!(selector.terms, ["in:a"]);
    }

    #[test]
    fn after_a_separator_dashes_are_text_terms() {
        let (terms, json, _) = find(&["find", "--", "--jsonn", "--dry-run", "---"]);
        assert_eq!(terms, ["--jsonn", "--dry-run", "---"]);
        assert!(!json);
        let (terms, json, dir) = find(&["find", "--json", "in:a", "--", "--json"]);
        assert_eq!(terms, ["in:a", "--json"]);
        assert!(json);
        assert_eq!(dir, None);
        let (terms, _, _) = find(&["find", "--", "--"]);
        assert_eq!(terms, ["--"]);
    }

    #[test]
    fn terms_with_one_dash_or_none_are_still_terms() {
        let (terms, json, _) = find(&[
            "find",
            "-in:n5",
            "-is:new",
            "-jsonn",
            "-x",
            "-",
            "-ものともせず",
            "asdf",
            "\"--verbose\"",
            "-\"--verbose\"",
            "—json",
            "--json",
        ]);
        assert_eq!(
            terms,
            [
                "-in:n5",
                "-is:new",
                "-jsonn",
                "-x",
                "-",
                "-ものともせず",
                "asdf",
                "\"--verbose\"",
                "-\"--verbose\"",
                "—json",
            ]
        );
        assert!(json);
    }

    #[test]
    fn commands_without_a_selector_are_untouched() {
        let args: Vec<OsString> = ["srl", "add", "-ない.md", "--stdin", "--dir", "-store"]
            .map(OsString::from)
            .into();
        assert_eq!(separate_selector(args.clone()), args);
    }
}
