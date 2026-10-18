# srl — overview

`srl` is a spaced repetition system for the command line. It stores knowledge as
small plain-text units called **atoms**, turns them into questions called
**cards**, records how well you answered, and schedules each atom for review
with the FSRS algorithm.

It is built to be used by a person with nothing but a terminal and an editor.
An LLM agent can operate it too, through the same commands, but no part of
`srl` depends on an LLM, an agent, or any particular note-taking app.

This document explains the main ideas. Each area has its own detailed
document, listed at the end.

## Goals

- **Long-term retention.** Spacing and repetition are the point; everything
  else serves them.
- **Plain text.** All data can be read with `cat`, searched with `grep`,
  edited in any editor and versioned with git.
- **Usable alone, extendable by anything.** A human can do everything with the
  CLI. Scripts, other frontends and LLM agents build on top of it.
- **Flexible paradigms.** New ways of asking questions and new ways of studying
  can be added as small programs, without changing the core.
- **Predictable behaviour.** Every rule is simple enough to state in one
  sentence. Where a smarter rule would be possible, `srl` picks the simple one
  and accepts the trade-off.

## Non-goals

- `srl` is not tied to Obsidian or any other vault. A vault is just one place
  knowledge can come from.
- `srl` does not contain, call or assume an LLM.
- `srl` is not a sync service or a multi-user system.

## Principles

1. **The store is the database, and it is made of text files.** Atoms are
   Markdown files with YAML front-matter, and reviews are lines in JSONL
   files. A SQLite index exists only as a cache that can be deleted at any
   time.
2. **The core contains no LLM.** Anything an agent can do, a human can also
   do with the same commands.
3. **The CLI is the API.** Every command has a non-interactive form with JSON
   output. There is no separate interface for scripts or agents.
4. **Flat rules.** One simple, predictable rule per decision. No weights, no
   adaptive policies, no hidden state.
5. **Small fixed contracts, everything else pluggable.** A handful of formats
   are rigid so that the programs around them can vary freely.
6. **Fail loudly.** No error is swallowed. Every failure is reported on
   standard error, saying what to do about it. Where `srl` can carry on
   without the part that failed (a broken drill, an invalid card), it does,
   and still reports the failure. It never quietly substitutes a guess or a
   default for something that went wrong.

## The pieces

```
 Sources (any): notes · articles · PDFs · LLM knowledge · CSV · your typing
          │
          ▼   srl add / srl new / editing files
 ┌──────────────────────────────────────────────────────────────────────┐
 │ Store (srl's database, plain text)                                   │
 │   atoms/**.md        one atom per file: front-matter + prose body    │
 │   log/*.jsonl        append-only review events                       │
 └──────────────────────────────────────────────────────────────────────┘
          │ atom                                  ▲ review event
          ▼                                       │
   Drills (programs) ──► Cards (fixed format) ──► Frontends ──► srl review
   built-in or yours      prompt, answer          terminal, web,
                                                  scripts, agents
```

## Glossary

| Term | Meaning |
|---|---|
| **Store** | A directory holding one `srl` database: atoms, log, config, caches. |
| **Atom** | One thing you can know or not know. Has an ID, a title, optional tags, optional facets, and a free prose body. It is the only thing with a memory state. |
| **Facet** | A named block of structured data in an atom's front-matter, such as `cloze:` or `cards:`. It describes one way the atom can be drilled. An atom's cards come only from its facets. |
| **Drill** | A program that reads one facet of an atom and outputs cards. A drill and its facet share a name. |
| **Card** | One ready-to-show question in a fixed format: a prompt and an answer. Cards are produced on demand and are not stored (except in sessions). |
| **Rating** | How well you knew a card, from 1 (Again) to 4 (Easy). Always given by a person or an agent, never computed by `srl`. |
| **Frontend** | A program that shows cards to a person and records the ratings: the terminal `srl study`, a web viewer, a shell prompt. |
| **Review event** | One line in the log: this atom was rated this way, at this time, through this card. |
| **Selector** | A short filter expression that picks atoms, such as `tag:jlpt/n1,kanji/food -status:draft`. |
| **Memory state** | What FSRS knows about an atom (stability, difficulty, due date). Always computed from the log, never stored as truth. |

## The life of a review

1. You write an atom, by hand or with `srl add`. It has a title, tags, a
   facet or two (`cloze:` sentences, `cards:` questions), and a prose body.
2. `srl next tag:jlpt/n1` replays the log through FSRS, finds the most overdue
   atom matching the selector, and asks every applicable drill for cards. It
   returns the card used least recently for that atom.
3. A frontend shows the card's prompt. When you're ready, it shows the
   answer and the atom's body.
4. You rate yourself from 1 (Again) to 4 (Easy).
5. `srl review` appends a review event to the log. The atom's next due date
   follows from replaying the log again.

The atom's content can be changed at any time. Its identity is its ID, so
edits never affect its schedule.

## The four contracts

Most of `srl` is pluggable. These four formats are not, because everything else
depends on them:

| Contract | Joins | Document |
|---|---|---|
| **Atom file** | Authors (you, agents, imports) and everything that reads atoms | [atoms.md](atoms.md) |
| **Card** | Drills and frontends | [cards.md](cards.md) |
| **Review event** | Frontends and the scheduler or any analysis | [log.md](log.md) |
| **Selector** | People, scripts and agents, and every command that picks atoms | [selectors.md](selectors.md) |

When something new needs more than a contract offers, the contract is extended
for everyone, deliberately. Individual programs never invent private
extensions.

## What is fixed and what is pluggable

| Fixed (in the core) | Pluggable (outside the core) |
|---|---|
| Store layout, atom file format | Facets and the drills that read them |
| Card format | Frontends (terminal, web, ambient, …) |
| Review event format, FSRS scheduling | Agents and their prompts, tools and skills |
| Selector language | Judging answers, by you or an agent |
| CLI commands and their JSON output | Ingestion: anything that ends in `srl add` |

## Where LLMs fit

Nowhere inside, and in two places outside:

- **An agent operates the CLI**, the same way it might read and edit a notes
  folder: it adds draft atoms from articles, prepares study sessions, tutors
  you (judging your free-form answers, if you ask it to), analyzes your log.
- **A drill may call an LLM.** An `llm` drill is just another drill program.
  The core runs it like any other.

See [agents.md](agents.md).

## Documents

| Document | Contents |
|---|---|
| [store.md](store.md) | Store layout, how a store is found, configuration, the index cache |
| [atoms.md](atoms.md) | The atom file format, identity, status, creating and editing atoms |
| [drills.md](drills.md) | The drill protocol, built-in drills, writing your own |
| [cards.md](cards.md) | The card format and the text formatting allowed in cards |
| [scheduling.md](scheduling.md) | Ratings, FSRS, days, choosing the next card, new-atom limits, states |
| [log.md](log.md) | The review log and its events |
| [selectors.md](selectors.md) | The selector language |
| [cli.md](cli.md) | Command conventions and the command reference |
| [frontends.md](frontends.md) | The frontend contract, the terminal frontend, the web viewer |
| [agents.md](agents.md) | Operating `srl` with an LLM agent, with agi as the example |
| [rationale.md](rationale.md) | Why the main decisions were made |
| [ideas.md](ideas.md) | Mechanics not specified yet, to build on top later |

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

- **Sessions in the core or outside it.** Decide once sessions have been
  used in practice ([cli.md](cli.md), "Sessions").
- **Media in cards (audio, images) and furigana markup.** Today card text is
  Markdown with `==highlight==` only ([cards.md](cards.md), "Text
  formatting").
- **Partial credit for prerequisite atoms (`requires:`).** Today there is no
  partial credit ([scheduling.md](scheduling.md), "Cards about several
  atoms"; the idea is in [ideas.md](ideas.md), "Knowledge structure").
- **Unicode forms in tags and other text.** File paths are compared in NFC,
  but tags are compared exactly as written, so `tag:café` doesn't match a
  tag typed in decomposed form ([store.md](store.md), "File names").
