# srl — overview

`srl` is a spaced repetition system for the command line. You write
questions and answers inside your Markdown study notes, `srl` turns them
into **cards**, records how well you answered, and schedules each card for
review with the FSRS algorithm.

This document explains the main ideas. Each area has its own detailed
document, listed at the end.

## The idea

`srl` is two things, and nothing else:

1. **A data layer, and a CLI to work with it.** A store is your notes, with
   items in them, and a log of reviews. Every command reads or changes that
   data: adding, finding and editing items, accepting drafts, suspending,
   resetting, checking, analyzing. Scheduling is part of the data layer
   too: a card's memory state is computed from the log.
2. **One fixed mapping from that data to frontends.** `srl next --json`
   turns the store into one card ready to show, with its state, kind and
   context. This is the **frontend contract** (see
   [frontends.md](frontends.md)), which every frontend reads. The rating
   goes back with `srl review`, an ordinary command of the data layer: it
   takes a card ID and a rating, nothing that belongs to frontends.

Studying happens in a **frontend**: a separate program of your choice that
reads the frontend contract and records ratings, such as the web viewer or
the terminal frontend in this repository's `examples/`. Frontends, scripts
and LLM agents are all built on top of the CLI, outside `srl`. No part of
`srl` depends on a frontend, an LLM, an agent, or any particular
note-taking app.

## Goals

- **Long-term retention.** Spacing and repetition are the point; everything
  else serves them.
- **Questions live in your notes.** You write a question next to the
  knowledge it asks about, in the same file, in a few keystrokes. The note
  around it is the question's context.
- **Notes stay notes.** What `srl` adds to a note is hidden when the note is
  rendered, and reads naturally when it is not. A note with questions in it
  is still an ordinary Markdown file for every other program.
- **Plain text.** All data can be read with `cat`, searched with `grep`,
  edited in any editor and versioned with git.
- **All of the data through the CLI.** A person can read and change
  everything in a store with the CLI. Studying goes through a frontend,
  which is a separate program; frontends, scripts and LLM agents build on
  the same commands.
- **Predictable behaviour.** Every rule is simple enough to state in one
  sentence. Where a smarter rule would be possible, `srl` picks the simple one
  and accepts the trade-off.

## Non-goals

- `srl` is not tied to Obsidian or any other app. Notes are plain Markdown,
  as common to most Markdown flavours.
- `srl` does not contain, call or assume an LLM.
- `srl` is not a sync service or a multi-user system.
- `srl` is not a frontend, and does not launch other programs for you. The
  one exception is `srl edit ID`, which opens a note in your editor.

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
 Your study folder (the store): the data layer
 ┌──────────────────────────────────────────────────────────────────────┐
 │   **/*.md            your notes (by default); items inside them      │
 │   .srl/config.toml   configuration                                   │
 │   .srl/log/*.jsonl   append-only review events                       │
 └──────────────────────────────────────────────────────────────────────┘
          │ item                                  ▲ review event
          ▼                                       │
   Kind ─────────────► Card ──► srl next --json   srl review
   qa, or a program             └─ frontend ──┘   │
                                    contract      │
                                          │       ▲
                                          ▼       │ rating
                                  Frontends (outside srl): terminal,
                                  web, scripts, agents
```

## Glossary

| Term | Meaning |
|---|---|
| **Store** | A folder of notes (Markdown files by default) with a `.srl/` folder in it, which holds the configuration, the log and caches. |
| **Note** | A file in the store that `srl` reads for items: by default, any Markdown file. A note may hold items; the rest of it is yours. |
| **Item** | A question written in a note: a header line `<!-- srl id:x7k2m9 -->` followed by fields such as `Q:` and `A:`. It has an ID, and gives one or more cards. |
| **Kind** | How an item's fields become cards. `qa`, a question and an answer with optional cloze deletions, is built in and the default; any other kind is a program named in `.srl/config.toml`. A kind also decides which of an item's cards are siblings, and describes its fields (`srl kinds`). |
| **Card** | One question, ready to show: an ID, a prompt, and, when the kind gives them, the prompt as revealed with the answer and an answer. It is the unit that gets scheduled: every card has its own memory state. Cards are produced on demand and are not stored. |
| **Context** | The note an item belongs to, or the note its `see:` names, which frontends show after the answer of its cards. |
| **Rating** | How well you knew a card, from 1 (Again) to 4 (Easy). Always given by a person or an agent, never computed by `srl`. |
| **Frontend** | A program outside `srl` that shows cards to a person and records the ratings: it reads cards through the frontend contract and records ratings with `srl review`. `srl` has none built in. |
| **Review event** | One line in the log: this card was rated this way, at this time. |
| **Selector** | A short filter expression that picks cards, such as `in:grammar state:due`. |
| **Memory state** | What FSRS knows about a card (stability, difficulty, due date). Always computed from the log, never stored as truth. |

## The life of a review

1. In a note, you write an item: a header line, a question and an answer.
   `srl id` gives the header an ID.
2. A frontend runs `srl next in:grammar --json`, which replays the log
   through FSRS, finds the most overdue card matching the selector, and
   prints it with its context.
3. The frontend shows the card's prompt. When you're ready, it reveals the
   prompt (the blanks of a cloze filled in) and shows the answer and the
   context: the note the item is in.
4. You rate yourself from 1 (Again) to 4 (Easy).
5. The frontend runs `srl review`, which appends a review event to the
   log. The card's next due date follows from replaying the log again.

The item's text can be changed at any time, and the note can be renamed or
moved. A card's identity is its ID, so edits never affect its schedule.

## The contracts

These five formats are fixed, because everything else depends on them:

| Contract | Joins | Document |
|---|---|---|
| **Item syntax** | Authors (you, agents, imports) and everything that reads notes | [notes.md](notes.md) |
| **Card** | Kinds and everything that shows a card | [cards.md](cards.md) |
| **Frontend contract** | `srl` and frontends: `srl next --json` | [frontends.md](frontends.md) |
| **Review event** | `srl review` and the scheduler or any analysis | [log.md](log.md) |
| **Selector** | People, scripts and agents, and every command that picks cards | [selectors.md](selectors.md) |

When something new needs more than a contract offers, the contract is extended
for everyone, deliberately. Individual programs never invent private
extensions.

## What is fixed and what is pluggable

| Fixed (in the core) | Pluggable (outside the core) |
|---|---|
| Store layout, item syntax, the `qa` kind | Frontends (terminal, web, ambient, …) |
| Card format, frontend contract | Agents and their prompts, tools and skills |
| Review event format, FSRS scheduling, the sibling rule | Judging answers, by you or an agent |
| Selector language | Writing notes and items: by hand, by script, by agent |
| CLI commands and their JSON output | Scripts and tools that run the CLI |
| How kind programs are called | Kind programs: every kind but `qa` |

## Where LLMs fit

Nowhere inside. An agent operates the CLI and writes notes, the same way a
person does: it writes new items (as drafts), tutors you (judging your
free-form answers, if you ask it to), and analyzes your log. See
[guides/agents.md](guides/agents.md).

## Specification, guides and examples

What is written about `srl` lives in one of three places, by one rule: **is
it what `srl` is and does, or something done with it?**

- **The specification**, the documents directly in `docs/`: `srl`'s data,
  its commands, its contracts, the reasons for them, ideas for changing
  them, and how the implementation is written. Something belongs here if
  changing it changes what `srl` stores, accepts or prints, or how it is
  built. The specification never depends on a guide or an example.
- **Guides**, in `docs/guides/`: ways of using `srl` and of building on it,
  such as writing a frontend or operating `srl` with an agent, and ideas
  for such things. They are not part of the specification: they use its
  commands and formats like any other program would, and are updated when
  the specification changes.
- **Examples**, in the repository's `examples/`: programs and files built
  on `srl`, to run, copy and change. `examples/README.md` lists them:
  - `examples/agents/`: a system prompt and skills that turn an LLM agent
    into a guide to `srl` (see [guides/agents.md](guides/agents.md)).
  - `examples/python-frontend/`: `srl-tui`, a study frontend for the
    terminal in Python with Rich (see
    [guides/frontends.md](guides/frontends.md)).
  - `examples/web-frontend/`: `srl-web`, the web viewer, with a local
    server and a React page, a card browser that shows any card
    without studying it, and a form that adds items (see
    [guides/frontends.md](guides/frontends.md)).

A guide says how to use or build something; an example is the thing itself.
The details of an example (its options, keys, internals) are in its own
README, not in `docs/`.

The rules for examples:

- **`srl` never depends on an example.** Delete `examples/` and `srl`
  builds, tests and runs exactly as it did. Nothing in it is built, tested
  or installed with `srl`.
- **An example may be anything**: a program of its own, a script, or files
  to give another program, such as prompts.
- **The rules for `srl`'s code don't apply to it**, including platform
  independence. An example that works on some platforms only says so in a
  comment at its top.
- **An example follows the specification.** It is written against the
  same commands and formats as anything else built on `srl`, and is updated
  when they change.

## Documents

The specification:

| Document | Contents |
|---|---|
| [store.md](store.md) | Store layout, how a store is found, configuration, the index cache |
| [notes.md](notes.md) | Notes and the item syntax: headers, fields, IDs, status, context, how `srl` writes to notes |
| [cards.md](cards.md) | The `qa` kind, cloze deletions, kind programs and groups of siblings, card IDs, the card format and its text formatting |
| [scheduling.md](scheduling.md) | Ratings, FSRS, days, choosing the next card, siblings, new-card limits, states |
| [log.md](log.md) | The review log and its events |
| [selectors.md](selectors.md) | The selector language |
| [cli.md](cli.md) | Command conventions and the command reference |
| [frontends.md](frontends.md) | The frontend contract: what `srl next --json` gives, the loop every frontend runs, rendering |
| [rationale.md](rationale.md) | Why the main decisions were made |
| [ideas.md](ideas.md) | Changes to `srl` not specified yet |
| [rust.md](rust.md) | How the Rust implementation is organized and written |

Guides, not part of the specification:

| Document | Contents |
|---|---|
| [guides/frontends.md](guides/frontends.md) | Writing a frontend, and the example frontends |
| [guides/agents.md](guides/agents.md) | Operating `srl` with an LLM agent, with agi as the example |
| [guides/ideas.md](guides/ideas.md) | Ideas for programs and workflows built on `srl` |

## Open questions

What the specification has not settled yet comes in three kinds, each kept
in one place:

- **Design decisions still to take** are notes marked **TODO (design):**
  where the specification touches them. The main ones are listed below.
- **Platform gaps** are notes marked **TODO (platform):** in the document
  they concern, with a matching `TODO(platform):` comment in the code. The
  table in [rust.md](rust.md), "Platform independence", lists them all.
- **Ideas** that may be built later, without being promised, are in
  [ideas.md](ideas.md) when they would change `srl`, and in
  [guides/ideas.md](guides/ideas.md) when they would be built on top.

`grep -rn 'TODO (design)\|TODO (platform)\|TODO(platform)' docs crates`
finds every marked note.

Main design decisions still to take:

- **Caching kind programs.** Today a kind program's `ids` call runs for
  every item of its kind on every command; a cache would make that faster
  without changing any result ([cards.md](cards.md), "Kind programs").
- **Media in cards (audio, images) and furigana markup.** Today card text is
  Markdown with `==highlight==` only ([cards.md](cards.md), "Text
  formatting").
- **Opening the editor at an item's line.** Today `srl edit ID` opens the
  note at its start, because editors take a line number in different ways
  ([cli.md](cli.md), "Editing in the editor").
- **Making strictness less annoying.** Today every error in a note stops
  every command that reads items ([notes.md](notes.md), "Errors"), and a
  card its kind program can't make stops `srl next`
  ([cli.md](cli.md), "Studying"). That stays the rule: `srl` never works
  with broken data. More configuration and tools are wanted so that it
  gets in the way less, a combination of: health checks that troubleshoot
  a store and say what to fix; more linters and checkers that find
  problems before studying does, such as a card that can't be made; and
  settings that turn some errors into warnings. How is not decided.
- **A helper for moving history.** Today `srl move FROM TO` is the only
  way, and running it by hand takes more knowledge than a person should
  need. A safer helper is wanted, in `srl` or built on top
  ([scheduling.md](scheduling.md), "Moving history").
