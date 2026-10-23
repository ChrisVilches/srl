# srl — overview

`srl` is a spaced repetition system for the command line. You write
questions and answers inside your Markdown study notes, `srl` turns them
into **cards**, records how well you answered, and schedules each card for
review with the FSRS algorithm.

It is built to be used by a person with a terminal, an editor and a frontend
to study with: a small program of their choice that shows cards, such as the
web viewer in this repository's `examples/`. An LLM agent can operate it
too, through the same commands, but no part of `srl` depends on an LLM, an
agent, or any particular note-taking app.

This document explains the main ideas. Each area has its own detailed
document, listed at the end.

## Goals

- **Long-term retention.** Spacing and repetition are the point; everything
  else serves them.
- **Questions live in your notes.** You write a question next to the
  knowledge it asks about, in the same file, in a few keystrokes. The note
  around it is shown after the answer.
- **Notes stay notes.** What `srl` adds to a note is hidden when the note is
  rendered, and reads naturally when it is not. A note with questions in it
  is still an ordinary Markdown file for every other program.
- **Plain text.** All data can be read with `cat`, searched with `grep`,
  edited in any editor and versioned with git.
- **Usable alone, extendable by anything.** A human can do everything with the
  CLI. Frontends, scripts and LLM agents build on top of it.
- **Predictable behaviour.** Every rule is simple enough to state in one
  sentence. Where a smarter rule would be possible, `srl` picks the simple one
  and accepts the trade-off.

## Non-goals

- `srl` is not tied to Obsidian or any other app. Notes are plain Markdown,
  as common to most Markdown flavours.
- `srl` does not contain, call or assume an LLM.
- `srl` is not a sync service or a multi-user system.

## Principles

1. **The store is the database, and it is made of text files.** Items are
   lines in Markdown notes, and reviews are lines in JSONL files. A SQLite
   index exists only as a cache that can be deleted at any time.
2. **The core contains no LLM.** Anything an agent can do, a human can also
   do with the same commands.
3. **The CLI is the API.** Every command has a non-interactive form with JSON
   output. There is no separate interface for scripts or agents.
4. **Flat rules.** One simple, predictable rule per decision. No weights, no
   adaptive policies, no hidden state.
5. **Small fixed contracts.** A handful of formats are rigid so that the
   programs around them can vary freely.
6. **Fail loudly.** No error is swallowed. Every failure is reported on
   standard error, saying what to do about it. `srl` never quietly
   substitutes a guess or a default for something that went wrong, and a
   note with an error stops every command that reads items until it is
   fixed.

## The pieces

```
 Your study folder (the store)
 ┌──────────────────────────────────────────────────────────────────────┐
 │   **/*.md            your notes; items inside them                   │
 │   .srl/config.toml   configuration                                   │
 │   .srl/log/*.jsonl   append-only review events                       │
 └──────────────────────────────────────────────────────────────────────┘
          │ item                                  ▲ review event
          ▼                                       │
   Kind (built in) ──► Cards (fixed format) ──► Frontends ──► srl review
   qa                   id, prompt, answer      terminal, web,
                                                scripts, agents
```

## Glossary

| Term | Meaning |
|---|---|
| **Store** | A folder of Markdown notes with a `.srl/` folder in it, which holds the configuration, the log and caches. |
| **Note** | Any Markdown file in the store. A note may hold items; the rest of it is ordinary Markdown. |
| **Item** | A question written in a note: a header line `<!-- srl id:x7k2m9 -->` followed by fields such as `Q:` and `A:`. It has an ID, and gives one or more cards. |
| **Kind** | How an item's fields become cards. There is one, `qa`: a question and an answer, with optional cloze deletions. |
| **Card** | One question, ready to show: an ID, a prompt and an answer. It is the unit that gets scheduled: every card has its own memory state. Cards are produced on demand and are not stored. |
| **Context** | The note an item belongs to, or the note its `see:` names, shown after the answer of its cards. |
| **Rating** | How well you knew a card, from 1 (Again) to 4 (Easy). Always given by a person or an agent, never computed by `srl`. |
| **Frontend** | A program that shows cards to a person and records the ratings: the web viewer `srl web` and the terminal `srl tui` (both examples), a shell prompt. `srl` has none built in. |
| **Review event** | One line in the log: this card was rated this way, at this time. |
| **Selector** | A short filter expression that picks cards, such as `in:grammar state:due`. |
| **Memory state** | What FSRS knows about a card (stability, difficulty, due date). Always computed from the log, never stored as truth. |

## The life of a review

1. In a note, you write an item: a header line, a question and an answer.
   `srl id` gives the header an ID.
2. `srl next in:grammar` replays the log through FSRS, finds the most
   overdue card matching the selector, and prints it.
3. A frontend shows the card's prompt. When you're ready, it shows the
   answer and the context: the note the item is in.
4. You rate yourself from 1 (Again) to 4 (Easy).
5. `srl review` appends a review event to the log. The card's next due date
   follows from replaying the log again.

The item's text can be changed at any time, and the note can be renamed or
moved. A card's identity is its ID, so edits never affect its schedule.

## The four contracts

These four formats are fixed, because everything else depends on them:

| Contract | Joins | Document |
|---|---|---|
| **Item syntax** | Authors (you, agents, imports) and everything that reads notes | [notes.md](notes.md) |
| **Card** | The core and frontends | [cards.md](cards.md) |
| **Review event** | Frontends and the scheduler or any analysis | [log.md](log.md) |
| **Selector** | People, scripts and agents, and every command that picks cards | [selectors.md](selectors.md) |

When something new needs more than a contract offers, the contract is extended
for everyone, deliberately. Individual programs never invent private
extensions.

## What is fixed and what is pluggable

| Fixed (in the core) | Pluggable (outside the core) |
|---|---|
| Store layout, item syntax, kinds | Frontends (terminal, web, ambient, …) |
| Card format | Agents and their prompts, tools and skills |
| Review event format, FSRS scheduling | Judging answers, by you or an agent |
| Selector language | Writing notes and items: by hand, by script, by agent |
| CLI commands and their JSON output | Commands of your own, as `srl-NAME` programs |

## Where LLMs fit

Nowhere inside. An agent operates the CLI and writes notes, the same way a
person does: it writes new items (as drafts), tutors you (judging your
free-form answers, if you ask it to), and analyzes your log. See
[agents.md](agents.md).

## Examples

The repository's `examples/` folder holds things built on `srl` that are
not part of it: ways to learn it, conveniences, starting points for your own
tools. `examples/README.md` lists them:

- `examples/agents/`: a system prompt and skills that turn an LLM agent
  into a guide to `srl` (see [agents.md](agents.md)).
- `examples/python-frontend/`: `srl tui`, a study frontend for the
  terminal in Python with Rich, a uv project, meant for every platform but
  tried on Linux only (see [frontends.md](frontends.md), "Small examples").
- `examples/web-frontend/`: `srl web`, the web viewer, an npm package with
  a local server and a React page, meant for every platform but tried on
  Linux only (see [frontends.md](frontends.md), "The web viewer").

- **`srl` never depends on an example.** Delete `examples/` and `srl`
  builds, tests and runs exactly as it did. Nothing in it is built, tested
  or installed with `srl`.
- **An example may be anything**: a program of its own, a script, or files
  to give another program, such as prompts.
- **The rules for `srl`'s code don't apply to it**, including platform
  independence. An example that works on some platforms only says so in a
  comment at its top.
- **An example follows the specifications.** It is written against the
  same commands and formats as anything else built on `srl`, and is updated
  when they change.

## Documents

| Document | Contents |
|---|---|
| [store.md](store.md) | Store layout, how a store is found, configuration, the index cache |
| [notes.md](notes.md) | Notes and the item syntax: headers, fields, IDs, status, context, how `srl` writes to notes |
| [cards.md](cards.md) | The `qa` kind, cloze deletions, card IDs, the card format and its text formatting |
| [scheduling.md](scheduling.md) | Ratings, FSRS, days, choosing the next card, siblings, new-card limits, states |
| [log.md](log.md) | The review log and its events |
| [selectors.md](selectors.md) | The selector language |
| [cli.md](cli.md) | Command conventions and the command reference |
| [frontends.md](frontends.md) | The frontend contract, rendering, the web viewer (an example) |
| [agents.md](agents.md) | Operating `srl` with an LLM agent, with agi as the example |
| [rationale.md](rationale.md) | Why the main decisions were made |
| [ideas.md](ideas.md) | Mechanics not specified yet, to build on top later |
| [rust.md](rust.md) | How the Rust implementation is organized and written |

## Open questions

What the specification has not settled yet comes in three kinds, each kept
in one place:

- **Design decisions still to take** are listed below. Where the
  specification touches one, a note marked **TODO (design):** says what is
  open.
- **Platform gaps** are notes marked **TODO (platform):** in the document
  they concern, with a matching `TODO(platform):` comment in the code. The
  table in [rust.md](rust.md), "Platform independence", lists them all.
- **Ideas** that may be built on top later, without being promised, are in
  [ideas.md](ideas.md).

`grep -rn 'TODO (design)\|TODO (platform)\|TODO(platform)' docs crates`
finds every marked note.

Design decisions still to take:

- **Kinds as plugins.** Today `qa` is the only kind, and `kind:` accepts
  nothing else. Whether other kinds should be programs (`srl-kind-NAME`)
  is undecided, and so is whether such a program can work with one call
  instead of two (one for card IDs, one for card content), which would
  need a restructuring ([cards.md](cards.md), "Kinds"; the idea is in
  [ideas.md](ideas.md), "Kinds").
- **Media in cards (audio, images) and furigana markup.** Today card text is
  Markdown with `==highlight==` only ([cards.md](cards.md), "Text
  formatting").
- **Opening the editor at an item's line.** Today `srl edit ID` opens the
  note at its start, because editors take a line number in different ways
  ([cli.md](cli.md), "Editing in the editor").
- **A non-strict mode.** Today every error in a note stops every command
  that reads items. A `strict = false` setting could turn errors into
  warnings and skip the broken items ([notes.md](notes.md), "Errors").
