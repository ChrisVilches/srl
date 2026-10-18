//! The terminal frontend, `srl study` (see `docs/frontends.md`).
//!
//! It runs the frontend loop with the core directly: `next`, then `grade`,
//! then `review`, the same calls `srl next`, `srl grade` and `srl review`
//! make. It reads lines from the reader it is given and writes to the
//! writer it is given (the terminal, in real use), so that tests can drive
//! it. Problems go to standard error through `output`, like everywhere else.

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};

use anyhow::{Context, bail};
use srl_core::{
    Atoms, By, Card, Config, Drills, GradeRequest, Grading, Id, Log, Memories, NextOptions,
    ReviewRequest, Search, Selector, State, Status, Store,
};

use crate::{Outcome, commands, editor, output};

/// The start and end of highlighted text (`==…==`) in a terminal: bold
/// yellow.
///
// TODO(platform): ANSI escape codes work in terminals on Linux, macOS and
// in Windows Terminal, but not in the old Windows console (conhost) unless
// its virtual terminal mode is turned on. There the codes would show up as
// text. Decide whether to enable that mode, or to print the raw `==…==`
// (see docs/frontends.md).
const HIGHLIGHT: (&str, &str) = ("\x1b[1;33m", "\x1b[0m");

/// What a study session did, for the summary at the end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) reviews: usize,
    pub(crate) new: usize,
    pub(crate) again: usize,
}

/// What came of asking for an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Answer {
    /// You quit, or the input ended.
    Quit,
    /// A `self` card: you asked to see the answer.
    Revealed,
    /// The response, one string per blank.
    Given(Vec<String>),
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
            "`srl study` needs a terminal; in scripts and pipes, use `srl next`, `srl grade` and `srl review`"
        );
    }
    let color = io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none();
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

/// Where a session reads and writes, and whether it may use color.
pub(crate) struct Io<'a> {
    pub(crate) input: &'a mut dyn BufRead,
    pub(crate) out: &'a mut dyn Write,
    pub(crate) color: bool,
}

/// What the loop needs for every card.
struct Session<'a, 'b> {
    store: &'a Store,
    config: &'a Config,
    search: &'a Search,
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
    let mut session = Session {
        store,
        config: &config,
        search: &search,
        io,
        edit,
    };
    let mut summary = Summary::default();
    let mut first = true;
    loop {
        let atoms = Atoms::load(store)?;
        let log = Log::load(store, &config.schedule)?;
        if first {
            // Later rounds read the same files; their problems were
            // reported once already.
            atoms.problems.iter().for_each(output::problem);
            log.problems.iter().for_each(output::problem);
            first = false;
        }
        let today = config.schedule.study_day(commands::now()?.datetime());
        let memories = Memories::replay(&log, &config, today)?;
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
    /// Shows one card, takes the answer and the rating, and records it.
    fn card(&mut self, atoms: &Atoms, card: Card) -> anyhow::Result<Step> {
        let prompt = self.render(card.prompt());
        writeln!(self.io.out, "\n{prompt}")?;
        let response = match self.answer(&card)? {
            Answer::Quit => return Ok(Step::Quit),
            Answer::Revealed => None,
            Answer::Given(response) => Some(response),
        };
        let suggested = self.result(atoms, &card, response.as_ref())?;
        let id = card
            .atoms()
            .first()
            .context("a card should have at least one atom")?
            .clone();
        loop {
            let question = match suggested {
                Some(rating) => {
                    format!("Rating 1-4 (Enter for {rating}), e edit, s suspend, q quit: ")
                }
                None => "Rating 1-4, e edit, s suspend, q quit: ".to_owned(),
            };
            let Some(line) = self.read(&question)? else {
                return Ok(Step::Quit);
            };
            let rating = match (line.trim(), suggested) {
                ("", Some(rating)) => rating,
                ("1", _) => 1,
                ("2", _) => 2,
                ("3", _) => 3,
                ("4", _) => 4,
                ("q", _) => return Ok(Step::Quit),
                ("s", _) => {
                    let atoms = Atoms::load(self.store)?;
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
                ("e", _) => {
                    let atoms = Atoms::load(self.store)?;
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
            let atoms = Atoms::load(self.store)?;
            let request = ReviewRequest::new(card, rating, response, By::Human)?;
            srl_core::review(
                self.store,
                &atoms,
                &request,
                &self.config.grading.cmd,
                &commands::now()?,
                false,
            )?;
            return Ok(Step::Rated { rating });
        }
    }

    /// Collects the answer the card's grading mode asks for.
    fn answer(&mut self, card: &Card) -> anyhow::Result<Answer> {
        match card.grading() {
            Grading::SelfRated => {
                let line = self.read("(Enter to show the answer, q to quit) ")?;
                Ok(match line.as_deref().map(str::trim) {
                    None | Some("q") => Answer::Quit,
                    Some(_) => Answer::Revealed,
                })
            }
            Grading::Exact { accept } => {
                let mut response = Vec::new();
                for blank in 1..=accept.len() {
                    let Some(line) = self.read(&format!("[{blank}] "))? else {
                        return Ok(Answer::Quit);
                    };
                    response.push(line);
                }
                Ok(Answer::Given(response))
            }
            Grading::Choice { choices, .. } => {
                for (number, choice) in choices.iter().enumerate() {
                    let choice = self.render(choice);
                    writeln!(self.io.out, "{}. {choice}", number + 1)?;
                }
                loop {
                    let Some(line) = self.read("> ")? else {
                        return Ok(Answer::Quit);
                    };
                    // Anything but a listed number just asks again.
                    let picked = line
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|number| number.checked_sub(1))
                        .and_then(|index| choices.get(index));
                    match picked {
                        Some(choice) => return Ok(Answer::Given(vec![choice.clone()])),
                        None => {
                            writeln!(self.io.out, "Type a number from 1 to {}.", choices.len())?;
                        }
                    }
                }
            }
            Grading::Cmd => Ok(self
                .read("> ")?
                .map_or(Answer::Quit, |line| Answer::Given(vec![line]))),
        }
    }

    /// Grades the response, if there is one, and shows the answer, which
    /// blanks were right, the feedback and the explanation. Returns the
    /// suggested rating.
    fn result(
        &mut self,
        atoms: &Atoms,
        card: &Card,
        response: Option<&Vec<String>>,
    ) -> anyhow::Result<Option<u8>> {
        let graded = match response {
            Some(response) if !matches!(card.grading(), Grading::SelfRated) => {
                let request = GradeRequest::new(card.clone(), response.clone())?;
                let (graded, problem) =
                    srl_core::grade(&request, atoms, &self.config.grading.cmd, self.search);
                problem.iter().for_each(output::problem);
                Some(graded)
            }
            Some(_) | None => None,
        };
        let answer = self.render(card.answer());
        writeln!(self.io.out, "---\n{answer}")?;
        if let Some(graded) = &graded {
            for (blank, right) in graded.correct.iter().flatten().enumerate() {
                let verdict = if *right { "right" } else { "wrong" };
                writeln!(self.io.out, "[{}] {verdict}", blank + 1)?;
            }
            if let Some(feedback) = &graded.feedback {
                let feedback = self.render(feedback);
                writeln!(self.io.out, "{feedback}")?;
            }
        }
        if let Some(explanation) = card.explanation().filter(|text| !text.trim().is_empty()) {
            let explanation = self.render(explanation.trim_end());
            writeln!(self.io.out, "\n{explanation}")?;
        }
        Ok(graded.and_then(|graded| graded.suggested_rating))
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
    use std::fs;
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
        let log = Log::load(store, &Config::default().schedule).unwrap();
        log.entries.into_iter().map(|entry| entry.event).collect()
    }

    #[test]
    fn a_self_card_is_revealed_and_rated() {
        let (_tmp, store) = store(&[r#"{"title": "鮪", "answer": "tuna", "body": "A fish."}"#]);
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
        assert!(matches!(&events(&store)[..], [Event::Review(review)] if review.rating == 3));
    }

    #[test]
    fn an_exact_card_is_graded_and_enter_takes_the_suggestion() {
        let (_tmp, store) = store(&[r#"{"title": "鮪", "cloze": ["{{鮪}}は魚です。"]}"#]);
        let (summary, out, _) = run(&store, "", Some("cloze"), "ﾏｸﾞﾛ\n\n");
        assert!(out.contains("[1] wrong"), "{out}");
        assert!(out.contains("(Enter for 1)"), "{out}");
        assert_eq!(summary.again, 1);
        let Event::Review(review) = &events(&store)[0] else {
            panic!("expected a review");
        };
        assert_eq!(review.response, Some(vec!["ﾏｸﾞﾛ".to_owned()]));
        assert_eq!(review.grader, "exact");
    }

    #[test]
    fn a_choice_is_picked_by_number() {
        let (_tmp, store) = store(&[
            r#"{"title": "鮪", "answer": "tuna"}"#,
            r#"{"title": "鯖", "answer": "mackerel"}"#,
        ]);
        let atoms = Atoms::load(&store).unwrap();
        let other = atoms.atoms[1].id().clone();
        let path = store.atom_file(atoms.atoms[0].path());
        let text = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            text.replace("---\n\n", "").replacen(
                "answer: tuna\n",
                &format!("answer: tuna\nconfusable: [{other}]\n"),
                1,
            ),
        )
        .unwrap();
        let (_, out, _) = run(
            &store,
            &format!("-id:{other}"),
            Some("confusable"),
            "9\n1\n\n",
        );
        assert!(out.contains("Type a number from 1 to 2."), "{out}");
        assert!(out.contains("[1] "), "{out}");
        assert_eq!(events(&store).len(), 1);
    }

    #[test]
    fn quitting_records_nothing() {
        let (_tmp, store) = store(&[r#"{"title": "x"}"#]);
        let (summary, _, _) = run(&store, "", None, "q\n");
        assert_eq!(summary, Summary::default());
        assert!(events(&store).is_empty());
        let (summary, _, _) = run(&store, "", None, "\n");
        assert_eq!(summary, Summary::default());
        assert!(events(&store).is_empty());
    }

    #[test]
    fn s_suspends_and_e_edits_before_rating() {
        let (_tmp, store) = store(&[r#"{"title": "x"}"#, r#"{"title": "y"}"#]);
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
                r#"{{"atoms": ["{id}"], "drill": "agent", "ref": "agent:{prompt}", "prompt": "{prompt}", "answer": "a", "grading": "self"}}"#
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
