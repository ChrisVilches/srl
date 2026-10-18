//! The terminal frontend, `srl study` (see `docs/frontends.md`).
//!
//! It runs the frontend loop with the core directly: `next`, then `review`,
//! the same calls `srl next` and `srl review` make. It reads lines from the
//! reader it is given and writes to the writer it is given (the terminal,
//! in real use), so that tests can drive it. Problems go to standard error
//! through `output`, like everywhere else.

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};

use anyhow::{Context, bail};
use srl_core::{
    Atoms, By, Card, Config, Drills, Id, Index, Memories, NextOptions, ReviewRequest, Selector,
    State, Status, Store,
};

use crate::{Outcome, commands, editor, output};

/// The start and end of highlighted text (`==…==`) in a terminal: bold
/// yellow. Only used where [`colors_work`].
const HIGHLIGHT: (&str, &str) = ("\x1b[1;33m", "\x1b[0m");

/// What a study session did, for the summary at the end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) reviews: usize,
    pub(crate) new: usize,
    pub(crate) again: usize,
}

/// How one card ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Rated and recorded.
    Rated {
        rating: u8,
    },
    /// Suspended, so not recorded.
    Suspended,
    Quit,
}

/// Where the cards of a study session come from.
pub(crate) enum Cards<'a> {
    /// `srl next`'s choice, card after card: due atoms, then new ones.
    Scheduled {
        selector: &'a Selector,
        drill: Option<&'a str>,
    },
    /// A prepared session, in file order, each card once.
    Prepared(std::vec::IntoIter<Card>),
}

/// `srl study [SELECTOR] [--drill NAME]`: refuses without a terminal, then
/// studies until nothing is left or you quit.
pub(crate) fn run(store: &Store, terms: &[String], drill: Option<&str>) -> anyhow::Result<Outcome> {
    let selector = Selector::parse(&terms.join(" "))?;
    in_terminal(
        store,
        Cards::Scheduled {
            selector: &selector,
            drill,
        },
    )
}

/// `srl study --session NAME`: the cards of the session, in file order. A
/// card that is no longer valid is reported and skipped.
pub(crate) fn run_session(store: &Store, name: &str) -> anyhow::Result<Outcome> {
    let atoms = Atoms::load(store)?;
    let (cards, problems) = srl_core::load_session(store, &atoms, name)?;
    problems.iter().for_each(output::problem);
    in_terminal(store, Cards::Prepared(cards.into_iter()))
}

fn in_terminal(store: &Store, cards: Cards<'_>) -> anyhow::Result<Outcome> {
    if !io::stdin().is_terminal() {
        bail!(
            "`srl study` needs a terminal; in scripts and pipes, use `srl next` and `srl review`"
        );
    }
    let color = io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none() && colors_work();
    let mut input = io::stdin().lock();
    let mut out = io::stdout().lock();
    let mut edit = |atoms: &Atoms, id: &Id| editor::edit(store, atoms, id).map(|_| ());
    study(
        store,
        cards,
        &mut Io {
            input: &mut input,
            out: &mut out,
            color,
        },
        &mut edit,
    )?;
    Ok(Outcome::Done)
}

/// Whether the terminal shows ANSI colors. Windows consoles only do once
/// their virtual terminal mode is on, so it is turned on here; where that
/// fails (an old Windows, or standard error going to a file, which the mode
/// is also set for), highlights are printed raw, and the reason is
/// reported.
///
// TODO(platform): Not run on Windows yet. Windows Terminal has the mode on
// already; the old console (conhost) on Windows 10 and later should accept
// it. See docs/frontends.md.
#[cfg(windows)]
fn colors_work() -> bool {
    match anstyle_query::windows::enable_virtual_terminal_processing() {
        Ok(()) => true,
        Err(error) => {
            output::note(&format!(
                "this console can't show colors ({error}); highlights are shown as ==…=="
            ));
            false
        }
    }
}

/// Whether the terminal shows ANSI colors: terminals on Unix do.
#[cfg(not(windows))]
fn colors_work() -> bool {
    true
}

/// Where a session reads and writes, and whether it may use color.
pub(crate) struct Io<'a> {
    pub(crate) input: &'a mut dyn BufRead,
    pub(crate) out: &'a mut dyn Write,
    pub(crate) color: bool,
}

/// What the loop needs for every card.
struct Session<'a, 'b> {
    store: &'a Store,
    index: &'a mut Index,
    io: &'a mut Io<'b>,
    edit: &'a mut dyn FnMut(&Atoms, &Id) -> anyhow::Result<()>,
}

/// Studies `cards` until there are no more or you quit, then prints a
/// summary. Atoms and the log are read again before every card, so edits
/// and other frontends' reviews count at once.
pub(crate) fn study(
    store: &Store,
    mut cards: Cards<'_>,
    io: &mut Io<'_>,
    edit: &mut dyn FnMut(&Atoms, &Id) -> anyhow::Result<()>,
) -> anyhow::Result<Summary> {
    let config = Config::load(store)?;
    let search = commands::search()?;
    let (drills, problems) = Drills::find(store, &search);
    problems.iter().for_each(output::problem);
    let mut index = Index::open(store);
    let mut session = Session {
        store,
        index: &mut index,
        io,
        edit,
    };
    let mut summary = Summary::default();
    let mut first = true;
    loop {
        let atoms = session.atoms()?;
        let memories = session.index.memories(&config);
        commands::report_cache(session.index);
        let (memories, log_problems) = memories?;
        if first {
            // Later rounds read the same files; their problems were
            // reported once already.
            atoms.problems.iter().for_each(output::problem);
            log_problems.iter().for_each(output::problem);
            first = false;
        }
        let today = config.schedule.study_day(commands::now()?.datetime());
        let memories = Memories::new(memories, &config, today);
        let card = match &mut cards {
            Cards::Scheduled { selector, drill } => {
                let options = NextOptions {
                    drill: *drill,
                    day: today,
                    timeout: config.drills.timeout(),
                };
                let (card, problems) = srl_core::next(
                    &atoms,
                    &memories,
                    &drills,
                    Selector::clone(selector),
                    &config,
                    options,
                )?;
                problems.iter().for_each(output::problem);
                card
            }
            Cards::Prepared(list) => list.next(),
        };
        let Some(card) = card else {
            break;
        };
        let was_new = card
            .atoms()
            .first()
            .is_some_and(|id| memories.state(id) == State::New);
        match session.card(&atoms, card)? {
            Step::Rated { rating } => {
                summary = Summary {
                    reviews: summary.reviews + 1,
                    new: summary.new + usize::from(was_new),
                    again: summary.again + usize::from(rating == 1),
                };
            }
            Step::Suspended => {}
            Step::Quit => break,
        }
    }
    writeln!(
        session.io.out,
        "\n{} reviewed, {} new, {} Again.",
        plural(summary.reviews, "card"),
        plural(summary.new, "atom"),
        summary.again
    )?;
    Ok(summary)
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

impl Session<'_, '_> {
    /// Every atom, through the index cache. Problems with the cache are
    /// reported; those of the files are left to the caller.
    fn atoms(&mut self) -> anyhow::Result<Atoms> {
        let atoms = self.index.atoms();
        commands::report_cache(self.index);
        Ok(atoms?)
    }

    /// Shows one card's prompt, then its answer and the body of its atoms,
    /// takes your rating, and records it.
    fn card(&mut self, atoms: &Atoms, card: Card) -> anyhow::Result<Step> {
        let prompt = self.render(card.prompt());
        writeln!(self.io.out, "\n{prompt}")?;
        let line = self.read("(Enter to show the answer, q to quit) ")?;
        if matches!(line.as_deref().map(str::trim), None | Some("q")) {
            return Ok(Step::Quit);
        }
        self.reveal(atoms, &card)?;
        let id = card
            .atoms()
            .first()
            .context("a card should have at least one atom")?
            .clone();
        loop {
            let Some(line) = self.read("Rating 1-4, e edit, s suspend, q quit: ")? else {
                return Ok(Step::Quit);
            };
            let rating = match line.trim() {
                "1" => 1,
                "2" => 2,
                "3" => 3,
                "4" => 4,
                "q" => return Ok(Step::Quit),
                "s" => {
                    let atoms = self.atoms()?;
                    let atom = atoms
                        .get(&id)
                        .with_context(|| format!("atom {id} is gone from the store"))?;
                    srl_core::change_status(
                        self.store,
                        &[atom],
                        Status::Active,
                        Status::Suspended,
                        false,
                    )?;
                    writeln!(self.io.out, "Suspended {id}.")?;
                    return Ok(Step::Suspended);
                }
                "e" => {
                    let atoms = self.atoms()?;
                    if let Err(error) = (self.edit)(&atoms, &id) {
                        output::error(&error);
                    }
                    continue;
                }
                _ => {
                    writeln!(self.io.out, "Type 1, 2, 3 or 4, or e, s or q.")?;
                    continue;
                }
            };
            // Read the atoms again: an edit changes the file the review's
            // hash is taken from.
            let atoms = self.atoms()?;
            let request = ReviewRequest::new(card, rating, None, By::Human)?;
            srl_core::review(self.store, &atoms, &request, &commands::now()?, false)?;
            return Ok(Step::Rated { rating });
        }
    }

    /// Shows the card's answer, then the body of each of its atoms that has
    /// one.
    fn reveal(&mut self, atoms: &Atoms, card: &Card) -> anyhow::Result<()> {
        let answer = self.render(card.answer());
        writeln!(self.io.out, "---\n{answer}")?;
        for id in card.atoms() {
            let atom = atoms
                .get(id)
                .with_context(|| format!("atom {id} is gone from the store"))?;
            let body = atom.content().body().trim_end();
            if !body.trim().is_empty() {
                let body = self.render(body);
                writeln!(self.io.out, "\n{body}")?;
            }
        }
        Ok(())
    }

    /// Asks `question` and reads one line, without its line ending. `None`
    /// at the end of input.
    fn read(&mut self, question: &str) -> anyhow::Result<Option<String>> {
        write!(self.io.out, "{question}")?;
        self.io.out.flush()?;
        let mut line = String::new();
        if self.io.input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        Ok(Some(line.trim_end_matches(['\n', '\r']).to_owned()))
    }

    fn render(&self, text: &str) -> String {
        render(text, self.io.color)
    }
}

/// Card text for the terminal: with `color`, each `==…==` pair in bold
/// yellow; a `==` without a partner, and all text without `color`, as it is.
fn render(text: &str, color: bool) -> String {
    if !color {
        return text.to_owned();
    }
    let parts = text.split("==").collect::<Vec<_>>();
    let last = parts.len() - 1;
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| match (index % 2, index == last) {
            (1, false) => format!("{}{part}{}", HIGHLIGHT.0, HIGHLIGHT.1),
            (1, true) => format!("=={part}"),
            _ => (*part).to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use srl_core::{Event, Lookup};
    use tempfile::TempDir;

    use super::*;

    fn store(lines: &[&str]) -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        let contents = srl_core::Content::from_json_lines(&lines.join("\n")).unwrap();
        let atoms = Atoms::load(&store).unwrap();
        srl_core::add(&store, &atoms, contents, false, 0, false).unwrap();
        (tmp, store)
    }

    /// Studies the scheduled cards for `selector` with `typed` as the input;
    /// returns the summary, what was written, and how many times the editor
    /// was opened.
    fn run(
        store: &Store,
        selector: &str,
        drill: Option<&str>,
        typed: &str,
    ) -> (Summary, String, usize) {
        let selector = Selector::parse(selector).unwrap();
        drive(
            store,
            Cards::Scheduled {
                selector: &selector,
                drill,
            },
            typed,
        )
    }

    /// Studies `cards` with `typed` as the input, as [`run`] does.
    fn drive(store: &Store, cards: Cards<'_>, typed: &str) -> (Summary, String, usize) {
        let mut input = Cursor::new(typed.as_bytes().to_vec());
        let mut out = Vec::new();
        let mut edits = 0;
        let mut edit = |_: &Atoms, _: &Id| {
            edits += 1;
            Ok(())
        };
        let summary = study(
            store,
            cards,
            &mut Io {
                input: &mut input,
                out: &mut out,
                color: false,
            },
            &mut edit,
        )
        .unwrap();
        (summary, String::from_utf8(out).unwrap(), edits)
    }

    fn events(store: &Store) -> Vec<Event> {
        let log = srl_core::Log::load(store, &Config::default().schedule).unwrap();
        log.entries.into_iter().map(|entry| entry.event).collect()
    }

    #[test]
    fn a_card_is_revealed_with_the_body_and_rated() {
        let (_tmp, store) = store(&[
            r#"{"title": "Fish", "cards": [{"q": "鮪", "a": "tuna"}], "body": "A fish."}"#,
        ]);
        let (summary, out, _) = run(&store, "", None, "\n3\n");
        assert_eq!(
            summary,
            Summary {
                reviews: 1,
                new: 1,
                again: 0
            }
        );
        assert!(out.contains("\n鮪\n"), "{out}");
        assert!(out.contains("---\ntuna\n"), "{out}");
        assert!(out.contains("\nA fish."), "{out}");
        assert!(
            out.ends_with("1 card reviewed, 1 atom new, 0 Again.\n"),
            "{out}"
        );
        let Event::Review(review) = &events(&store)[0] else {
            panic!("expected a review");
        };
        assert_eq!((review.rating, review.response.as_deref()), (3, None));
    }

    #[test]
    fn a_cloze_card_shows_its_blanks_then_the_sentence() {
        let (_tmp, store) = store(&[r#"{"title": "鮪", "cloze": ["{{鮪}}は魚です。"]}"#]);
        let (summary, out, _) = run(&store, "", Some("cloze"), "\n1\n");
        assert!(out.contains("\n==[1]==は魚です。\n"), "{out}");
        assert!(out.contains("---\n==鮪==は魚です。\n"), "{out}");
        assert_eq!(summary.again, 1);
    }

    #[test]
    fn enter_alone_at_the_rating_asks_again() {
        let (_tmp, store) = store(&[r#"{"title": "x", "cards": [{"q": "q", "a": "a"}]}"#]);
        let (summary, out, _) = run(&store, "", None, "\n\n2\n");
        assert!(out.contains("Type 1, 2, 3 or 4, or e, s or q."), "{out}");
        assert_eq!(summary.reviews, 1);
    }

    #[test]
    fn atoms_without_cards_are_not_studied() {
        let (_tmp, store) = store(&[r#"{"title": "x", "body": "Notes only."}"#]);
        let (summary, out, _) = run(&store, "", None, "");
        assert_eq!(summary, Summary::default());
        assert!(!out.contains("Notes only."), "{out}");
    }

    #[test]
    fn quitting_records_nothing() {
        let (_tmp, store) = store(&[r#"{"title": "x", "cards": [{"q": "q", "a": "a"}]}"#]);
        let (summary, _, _) = run(&store, "", None, "q\n");
        assert_eq!(summary, Summary::default());
        assert!(events(&store).is_empty());
        let (summary, _, _) = run(&store, "", None, "\n");
        assert_eq!(summary, Summary::default());
        assert!(events(&store).is_empty());
    }

    #[test]
    fn s_suspends_and_e_edits_before_rating() {
        let (_tmp, store) = store(&[
            r#"{"title": "x", "cards": [{"q": "q", "a": "a"}]}"#,
            r#"{"title": "y", "cards": [{"q": "q", "a": "a"}]}"#,
        ]);
        let (summary, out, edits) = run(&store, "", None, "\ne\nwhat\ns\n\n4\n");
        assert_eq!(edits, 1);
        assert!(out.contains("Type 1, 2, 3 or 4, or e, s or q."), "{out}");
        assert!(out.contains("Suspended"), "{out}");
        assert_eq!(summary.reviews, 1);
        let atoms = Atoms::load(&store).unwrap();
        assert_eq!(atoms.atoms[0].content().status(), Status::Suspended);
        assert!(matches!(&events(&store)[..], [Event::Review(review)] if review.rating == 4));
    }

    #[test]
    fn a_session_shows_its_cards_in_order_whether_due_or_not() {
        let (_tmp, store) = store(&[r#"{"title": "鮪"}"#, r#"{"title": "鯖"}"#]);
        let atoms = Atoms::load(&store).unwrap();
        let ids = atoms
            .atoms
            .iter()
            .map(|atom| atom.id().to_string())
            .collect::<Vec<_>>();
        let card = |id: &str, prompt: &str| {
            format!(
                r#"{{"atoms": ["{id}"], "drill": "agent", "ref": "agent:{prompt}", "prompt": "{prompt}", "answer": "a"}}"#
            )
        };
        // The second atom first, and the first one twice.
        let text = [
            card(&ids[1], "one"),
            card(&ids[0], "two"),
            card(&ids[0], "three"),
        ]
        .join("\n");
        srl_core::put_session(&store, &atoms, "mixed", &text, false).unwrap();
        let (cards, problems) = srl_core::load_session(&store, &atoms, "mixed").unwrap();
        assert!(problems.is_empty());
        let (summary, out, _) = drive(
            &store,
            Cards::Prepared(cards.into_iter()),
            "\n3\n\n3\n\n1\n",
        );
        let one = out.find("\none\n").unwrap();
        let two = out.find("\ntwo\n").unwrap();
        let three = out.find("\nthree\n").unwrap();
        assert!(one < two && two < three, "{out}");
        assert_eq!(summary.reviews, 3);
        assert_eq!(summary.new, 2);
        assert_eq!(summary.again, 1);
        assert_eq!(events(&store).len(), 3);
    }

    #[test]
    fn highlights_are_colored_only_in_pairs() {
        assert_eq!(render("a ==b== c", false), "a ==b== c");
        assert_eq!(render("a ==b== c", true), "a \x1b[1;33mb\x1b[0m c");
        assert_eq!(render("x == y", true), "x == y");
    }
}
