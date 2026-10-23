//! The terminal frontend, `srl study` (see `docs/frontends.md`).
//!
//! It runs the frontend loop with the core directly: `next`, then `review`,
//! the same calls `srl next` and `srl review` make. It reads lines from the
//! reader it is given and writes to the writer it is given (the terminal,
//! in real use), so that tests can drive it. Problems go to standard error
//! through `output`, like everywhere else.

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, bail};
use srl_core::{By, Card, Config, Index, Item, Notes, ReviewRequest, Selector, State, Store};

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

/// `srl study [SELECTOR]`: refuses without a terminal, then studies until
/// nothing is left or you quit.
pub(crate) fn run(store: &Store, selector: &Selector) -> anyhow::Result<Outcome> {
    if !io::stdin().is_terminal() {
        bail!(
            "`srl study` needs a terminal; in scripts and pipes, use `srl next` and `srl review`"
        );
    }
    let color = io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none() && colors_work();
    let mut input = io::stdin().lock();
    let mut out = io::stdout().lock();
    let mut edit = |file: &Path| editor::open(file);
    study(
        store,
        selector,
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
    config: Config,
    io: &'a mut Io<'b>,
    edit: &'a mut dyn FnMut(&Path) -> anyhow::Result<()>,
}

/// Studies the cards `srl next` gives for `selector` until there are no
/// more or you quit, then prints a summary. Notes and the log are read
/// again before every card, so edits and other frontends' reviews count at
/// once.
pub(crate) fn study(
    store: &Store,
    selector: &Selector,
    io: &mut Io<'_>,
    edit: &mut dyn FnMut(&Path) -> anyhow::Result<()>,
) -> anyhow::Result<Summary> {
    let mut index = Index::open(store);
    let mut session = Session {
        store,
        config: Config::load(store)?,
        io,
        edit,
    };
    let mut summary = Summary::default();
    let mut first = true;
    loop {
        let notes = commands::strict(Notes::load(store, &session.config)?)?;
        let (memories, _, log_problems) = commands::memories(&mut index, &session.config)?;
        if first {
            // Later rounds read the same files; their problems were
            // reported once already.
            log_problems.iter().for_each(output::problem);
            first = false;
        }
        let Some(card) = srl_core::next(
            &notes,
            &memories,
            Selector::clone(selector),
            &session.config,
        ) else {
            break;
        };
        let item = notes
            .item(card.id().item())
            .context("a card's item should be in the notes")?;
        let was_new = memories.state(card.id()) == State::New;
        match session.card(item, card, &memories)? {
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
        summary.new,
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
    /// Shows one card's prompt, then its answer and the item's context,
    /// takes your rating, and records it.
    fn card(
        &mut self,
        item: &Item,
        card: &Card,
        memories: &srl_core::Memories,
    ) -> anyhow::Result<Step> {
        let prompt = self.render(card.prompt());
        writeln!(self.io.out, "\n{prompt}")?;
        let line = self.read("(Enter to show the answer, q to quit) ")?;
        if matches!(line.as_deref().map(str::trim), None | Some("q")) {
            return Ok(Step::Quit);
        }
        let answer = self.render(card.answer());
        writeln!(self.io.out, "---\n{answer}")?;
        if let Some(context) = srl_core::context(self.store, item, self.config.study.context)? {
            let context = self.render(context.trim_end());
            writeln!(self.io.out, "--- {}\n{context}", item.context_path())?;
        }
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
                    srl_core::suspend(
                        self.store,
                        &[(item, card)],
                        memories,
                        true,
                        &commands::now()?,
                        false,
                    )?;
                    writeln!(self.io.out, "Suspended {}.", card.id())?;
                    return Ok(Step::Suspended);
                }
                "e" => {
                    if let Err(error) = (self.edit)(&self.store.note_file(item.path())) {
                        output::error(&error);
                    }
                    continue;
                }
                _ => {
                    writeln!(self.io.out, "Type 1, 2, 3 or 4, or e, s or q.")?;
                    continue;
                }
            };
            // Read the notes again: an edit changes the item the review's
            // hash is taken from.
            let notes = commands::strict(Notes::load(self.store, &self.config)?)?;
            let request = ReviewRequest {
                card: card.id().clone(),
                rating,
                response: None,
                by: By::Human,
            };
            srl_core::review(self.store, &notes, &[request], &commands::now()?, false)?;
            return Ok(Step::Rated { rating });
        }
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

    fn store(notes: &[(&str, &str)]) -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        Store::init(tmp.path(), false).unwrap();
        let store = Store::open(&Lookup {
            dir: Some(tmp.path().to_path_buf()),
            ..Lookup::default()
        })
        .unwrap();
        for (path, text) in notes {
            fs::write(store.note_file(path), text).unwrap();
        }
        (tmp, store)
    }

    /// Studies the cards for `selector` with `typed` as the input; returns
    /// the summary, what was written, and how many times the editor was
    /// opened.
    fn run(store: &Store, selector: &str, typed: &str) -> (Summary, String, usize) {
        let selector = Selector::parse(selector).unwrap();
        let mut input = Cursor::new(typed.as_bytes().to_vec());
        let mut out = Vec::new();
        let mut edits = 0;
        let mut edit = |_: &Path| {
            edits += 1;
            Ok(())
        };
        let summary = study(
            store,
            &selector,
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
    fn a_card_is_revealed_with_the_context_and_rated() {
        let (_tmp, store) = store(&[(
            "fish.md",
            "A fish.\n\n<!-- srl id:aaa1 -->\nQ: 鮪\nA: tuna\n",
        )]);
        let (summary, out, _) = run(&store, "", "\n3\n");
        assert_eq!(
            summary,
            Summary {
                reviews: 1,
                new: 1,
                again: 0
            }
        );
        assert!(out.contains("\n鮪\n"), "{out}");
        assert!(out.contains("---\ntuna\n--- fish.md\nA fish.\n"), "{out}");
        assert!(out.ends_with("1 card reviewed, 1 new, 0 Again.\n"), "{out}");
        let Event::Review(review) = &events(&store)[0] else {
            panic!("expected a review");
        };
        assert_eq!((review.rating, review.response.as_deref()), (3, None));
    }

    #[test]
    fn a_cloze_card_shows_its_blank_then_the_sentence_and_no_context_when_there_is_none() {
        let (_tmp, store) = store(&[("a.md", "<!-- srl id:aaa1 -->\nQ: {{c1::鮪}}は魚です。\n")]);
        let (summary, out, _) = run(&store, "", "\n1\n");
        assert!(out.contains("\n==[...]==は魚です。\n"), "{out}");
        assert!(out.contains("---\n==鮪==は魚です。\nRating"), "{out}");
        assert_eq!(summary.again, 1);
    }

    #[test]
    fn enter_alone_at_the_rating_asks_again() {
        let (_tmp, store) = store(&[("a.md", "<!-- srl id:aaa1 -->\nQ: q\nA: a\n")]);
        let (summary, out, _) = run(&store, "", "\n\n2\n");
        assert!(out.contains("Type 1, 2, 3 or 4, or e, s or q."), "{out}");
        assert_eq!(summary.reviews, 1);
    }

    #[test]
    fn quitting_records_nothing() {
        let (_tmp, store) = store(&[("a.md", "<!-- srl id:aaa1 -->\nQ: q\nA: a\n")]);
        let (summary, _, _) = run(&store, "", "q\n");
        assert_eq!(summary, Summary::default());
        assert!(events(&store).is_empty());
        let (summary, _, _) = run(&store, "", "\n");
        assert_eq!(summary, Summary::default());
        assert!(events(&store).is_empty());
    }

    #[test]
    fn s_suspends_and_e_edits_before_rating() {
        let (_tmp, store) = store(&[(
            "a.md",
            "<!-- srl id:aaa1 -->\nQ: q\nA: a\n\n<!-- srl id:aaa2 -->\nQ: r\nA: b\n",
        )]);
        let (summary, out, edits) = run(&store, "", "\ne\nwhat\ns\n\n4\n");
        assert_eq!(edits, 1);
        assert!(out.contains("Type 1, 2, 3 or 4, or e, s or q."), "{out}");
        assert!(out.contains("Suspended aaa1."), "{out}");
        assert_eq!(summary.reviews, 1);
        assert!(
            matches!(&events(&store)[..], [Event::Suspend(_), Event::Review(review)] if review.rating == 4)
        );
    }

    #[test]
    fn siblings_wait_for_the_next_day() {
        let (_tmp, store) = store(&[("a.md", "<!-- srl id:aaa1 -->\nQ: {{c1::x}} {{c2::y}}\n")]);
        let (summary, _, _) = run(&store, "", "\n3\n\n3\n");
        assert_eq!(summary.reviews, 1);
    }

    #[test]
    fn highlights_are_colored_only_in_pairs() {
        assert_eq!(render("a ==b== c", false), "a ==b== c");
        assert_eq!(render("a ==b== c", true), "a \x1b[1;33mb\x1b[0m c");
        assert_eq!(render("x == y", true), "x == y");
    }
}
