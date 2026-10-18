# srs — overview

`srs` is a spaced repetition system for the command line. It stores knowledge as
small plain-text units called **atoms**, turns them into questions called
**cards**, records how well you answered, and schedules each atom for review
with the FSRS algorithm.

It is built to be used by a person with nothing but a terminal and an editor.
An LLM agent can operate it too, through the same commands, but no part of
`srs` depends on an LLM, an agent, or any particular note-taking app.

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
  sentence. Where a smarter rule would be possible, `srs` picks the simple one
  and accepts the trade-off.

## Non-goals

- `srs` is not tied to Obsidian or any other vault. A vault is just one place
  knowledge can come from.
- `srs` does not contain, call or assume an LLM.
- `srs` is not a sync service or a multi-user system.

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

## The pieces

```
 Sources (any): notes · articles · PDFs · LLM knowledge · CSV · your typing
          │
          ▼   srs add / srs new / editing files
 ┌──────────────────────────────────────────────────────────────────────┐
 │ Store (srs's database, plain text)                                   │
 │   atoms/**.md        one atom per file: front-matter + prose body    │
 │   log/*.jsonl        append-only review events                       │
 └──────────────────────────────────────────────────────────────────────┘
          │ atom                                  ▲ review event
          ▼                                       │
   Drills (programs) ──► Cards (fixed format) ──► Frontends ──► srs grade / srs review
   built-in or yours      prompt, answer,         terminal, web,
                          grading, …              scripts, agents
```

## Glossary

| Term | Meaning |
|---|---|
| **Store** | A directory holding one `srs` database: atoms, log, config, caches. |
| **Atom** | One thing you can know or not know. Has an ID, a cue, optional tags, optional facets, and a free prose body. It is the only thing with a memory state. |
| **Facet** | A named block of structured data in an atom's front-matter, such as `cloze:` or `cards:`. It describes one way the atom can be drilled. |
| **Drill** | A program that reads one facet of an atom and outputs cards. A drill and its facet share a name. |
| **Card** | One ready-to-show question in a fixed format. Cards are produced on demand and are not stored (except in caches). |
| **Grading** | How an answer is judged: `self`, `exact`, `choice` or `cmd`. |
| **Frontend** | A program that shows cards to a person and records the results: the terminal `srs study`, a web viewer, a shell prompt. |
| **Review event** | One line in the log: this atom was rated this way, at this time, through this card. |
| **Selector** | A short filter expression that picks atoms, such as `tag:jlpt/n1,kanji/food -status:draft`. |
| **Memory state** | What FSRS knows about an atom (stability, difficulty, due date). Always computed from the log, never stored as truth. |

## The life of a review

1. You write an atom, by hand or with `srs add`. It has a cue, tags, perhaps a
   `cloze:` facet with example sentences, and a prose explanation.
2. `srs next tag:jlpt/n1` replays the log through FSRS, finds the most overdue
   atom matching the selector, and asks every applicable drill for cards. It
   returns the card used least recently for that atom.
3. A frontend shows the card's prompt, collects your answer and, for
   automatically graded cards, asks `srs grade` whether it was right.
4. The frontend shows the answer and explanation and asks you for a rating
   (1–4), suggesting one where it can.
5. `srs review` appends a review event to the log. The atom's next due date
   follows from replaying the log again.

The atom's content can be changed at any time. Its identity is its ID, so
edits never affect its schedule.

## The four contracts

Most of `srs` is pluggable. These four formats are not, because everything else
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
| Card format, grading modes and answer normalization | Frontends (terminal, web, ambient, …) |
| Review event format, FSRS scheduling | External grader command |
| Selector language | Agents and their prompts, tools and skills |
| CLI commands and their JSON output | Ingestion: anything that ends in `srs add` |

## Where LLMs fit

Nowhere inside, and in three places outside:

- **An agent operates the CLI**, the same way it might read and edit a notes
  folder: it adds draft atoms from articles, prepares study sessions, tutors
  you, analyzes your log.
- **A drill may call an LLM.** An `llm` drill is just another drill program.
  The core runs it like any other.
- **The external grader may call an LLM** to judge free-form answers. The core
  only sees a command that reads JSON and writes JSON.

See [agents.md](agents.md).

## Documents

| Document | Contents |
|---|---|
| [store.md](store.md) | Store layout, how a store is found, configuration, the index cache |
| [atoms.md](atoms.md) | The atom file format, identity, status, creating and editing atoms |
| [drills.md](drills.md) | The drill protocol, built-in drills, writing your own |
| [cards.md](cards.md) | The card format and the text formatting allowed in cards |
| [grading.md](grading.md) | Grading modes, answer normalization, suggested ratings, the external grader |
| [scheduling.md](scheduling.md) | FSRS, days, choosing the next card, new-atom limits, states |
| [log.md](log.md) | The review log and its events |
| [selectors.md](selectors.md) | The selector language |
| [cli.md](cli.md) | Command conventions and the command reference |
| [frontends.md](frontends.md) | The frontend contract, the terminal frontend, the web viewer |
| [agents.md](agents.md) | Operating `srs` with an LLM agent, with agi as the example |
| [rationale.md](rationale.md) | Why the main decisions were made |
| [ideas.md](ideas.md) | Mechanics not specified yet, to build on top later |

## Not decided yet

- Alternative accepted answers inside cloze spans.
- Media in cards (audio, images) and furigana markup.
- Separate memory states for different directions of the same atom.
- Giving partial credit to prerequisite atoms (`requires:`).
- The exact schema of the SQLite index.
