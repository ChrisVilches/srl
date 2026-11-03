# srl

A programmable data layer for spaced repetition.

`srl` reads your questions from wherever they live, keeps your reviews in
a plain-text log, schedules every card with
[FSRS](https://github.com/open-spaced-repetition/fsrs4anki/wiki/ABC-of-FSRS),
and exposes all of it through a CLI with JSON output. Out of the box,
questions live inside your Markdown notes, next to what they ask about.
They can also be lines of JSONL files that programs write, or be kept by a
program of your own: in a database, behind a web service, in any format.

`srl` has no study screen of its own. Instead, it is the one place where
your study data comes together and stays correct, and everything else is
built on top: web and terminal frontends, shell scripts, importers,
programs that make new types of cards, source programs that keep items
their own way, and any other tool you write.

```
   sources: Markdown notes, JSONL, a program of yours (a database, …)
                │ items              ▲ changes
                ▼                    │
 ┌─────────────────────────────────────────────────────┐
 │  srl: items + review log → FSRS → the next card     │
 │  every command non-interactive, every output JSON   │
 └─────────────────────────────────────────────────────┘
                │ srl next --json    ▲ srl review
                ▼                    │
     any frontend: web, terminal, a shell loop, …
```

## Why a data layer

Spaced repetition apps bundle four things: where your cards are kept, the
format they're written in, the scheduler, and the screen you study on. You
can't easily keep cards next to your notes, generate them with a script,
study in a different interface, or analyze what you keep forgetting with
your own tools, because the data is locked inside the app.

`srl` keeps only the part that has to be right: the items, the reviews and
the schedule. Where items are kept and how cards are shown are separate,
so either can be replaced, rewritten or built by you.

- **The CLI is the API.** Every command is non-interactive, with JSON
  output. Frontends and scripts use the same commands you do; there
  is no separate interface to learn.
- **Small fixed contracts.** The item syntaxes, the card, the frontend
  contract (`srl next --json`), the review event, the selector language
  and the calls to source programs are rigid and documented, so the
  programs around them can vary freely.
- **Items can live anywhere.** A store reads its items from its sources:
  Markdown notes by default, JSONL files, or programs you write.
  Scheduling, selectors, card types and frontends work the same whatever
  the source, so a card from a database is studied like a card from a note.
- **One schedule for everyone.** Frontends never compute schedules. Study
  in the browser in the morning and in a terminal at night, and it's the
  same queue.
- **Plain text by default.** Items in notes are lines in files, and
  reviews are always lines in append-only JSONL files. `cat`, `grep`, git
  and any program that reads files can work with them. The SQLite index
  is only a cache.
- **New types of cards are programs.** Beside questions and answers with
  cloze deletions, a type of card can be a program you write in any
  language: conjugation drills, words of a sentence, problems drawn at
  random. `srl` schedules their cards like any other.

## Where your items live

A store is a folder with a `.srl/` folder in it, which holds its
configuration and its review log. Its items come from its **sources**,
each mounted at a place in one tree of addresses, so every group of
items, whether a note or a database table, is reached by an address such
as `japanese/grammar/wo.md` or `vocab/words`. A store without
configuration has one source: every Markdown file in it.

### Markdown notes, the default

````markdown
# 食い倒れ

Eating yourself into ruin. Osaka's motto, lived nightly along Dotonbori:
京の着倒れ、大阪の食い倒れ (Kyoto spends it on clothes, Osaka on food).

```srl
@ id:x7k2m9
Q: What does 食い倒れ mean?
A: Eating until you drop, or go broke.

@ id:p4n8d2
Q: 道頓堀で{{c1::食い倒れ}}の一日を過ごした。
```
````

That's a complete note with two cards in it. The questions live in a
fenced `srl` block, so the note stays a normal Markdown file for your
editor, your viewer or GitHub, which show the block as code; a block can
also hold a single question, with its ID on the fence (```` ```srl
id:x7k2m9 ````). When you study a card, the note around it is shown as
context after the answer.

A card's identity is its ID, so you can reword a question or move its note
without losing its schedule. Your folders are your topics:
`srl next in:grammar` studies every note under a `grammar` folder.

### JSONL notes, for items that programs write

Questions that scripts write can go in a JSONL note instead,
one JSON object per line, each naming the note it asks about with `see`:

```json
{"id": "q8m2k1", "see": "japanese/vocabulary/osaka.md", "Q": "What does 食い倒れ mean?", "A": "Eating yourself into ruin."}
```

A JSONL note holds nothing but items, and has no context of its own, so
each item names its note with `see`. `srl add` and `srl edit` take items
in that same form, whatever source they go to, and `srl id` gives IDs to
the items that have none. JSONL notes are read once your configuration
names a source of them, in `.srl/config.toml`, beside the notes:

```toml
[sources.notes]
driver = "markdown"

[sources.data]
driver = "jsonl"
```

### Source programs, for items kept your own way

Any other source is a program you name in the configuration, written in
any language. It answers a few calls over standard input and output (list
its collections, read its items, show a collection as context, write
changes) and keeps items however it likes: in a database, behind a web
API, in a format `srl` doesn't know.

```toml
[sources.vocab]
program = ["python3", "sources/tinydb_source.py"]
prefix = "vocab/"
database = "vocab/vocab.json"    # the program's own setting
```

Its items are then added, found, edited and studied with the same
commands as any other: `srl add vocab/words --stdin` adds items to the
table `words`, `srl next source:vocab` studies only its cards, and an item
in the database can name a Markdown note with `see` to show it as
context. `srl sources` and `srl collections` describe what a store reads.

## What you can build on it

### Frontends

A frontend is any program that calls `srl next --json` and records ratings
with `srl review`. It knows nothing about where items are kept, how they
are written or what type of card it shows, so any frontend can show any
card. This shell loop is a complete
one:

```sh
while card=$(srl next in:grammar --json); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r '.prompt_revealed // .prompt, .answer // empty, "---", .context // empty' <<<"$card"; read -r -p 'rating 1-4: ' r
  jq -n --argjson card "$card" --argjson rating "$r" '{card: $card.id, rating: $rating}' | srl review --stdin
done
```

The repository has two fuller ones to start from: `srl-web`, a local web
viewer with a card browser and a form for adding items to any collection
of any source, and `srl-tui`, a terminal frontend in Python.

> **Note:** `srl` doesn't include a frontend in its core. These two are
> ready-to-use examples that ship with the repository, in
> [examples/](examples/), to use as they are or to change.

`srl-web`, with the answer of a card revealed and the ratings below it:

![srl-web showing the answer of a card about the Japanese adverb なるべく, with its note as context and the rating buttons](examples/screenshots/images/web-2-study-answer-dark.png)

`srl-tui`, on a card about Ohm's law:

![srl-tui showing the answer of a card about Ohm's law, with its note as context and the rating keys](examples/screenshots/images/tui-6-electrical.png)

### Card types: cards made by your own programs

An item's type decides how its fields become cards. The built-in type,
`qa`, gives questions and answers with cloze deletions, and is what an
item is unless it says otherwise. Any other type is a program you write,
in any language, and name in `.srl/config.toml`. `srl` calls a type of
card a *kind*, so that is the word you'll see in the configuration, in
an item's `kind:` and in `srl kinds`:

```toml
[kinds]
ja-words = ["python3", "kinds/ja_words.py"]
```

Such a program checks the items of its type, says which cards each item
gives and which of them would give each other's answers away (`srl` then
shows at most one of those a day), and makes each card's content when it
is shown, so it can do anything a program can: conjugate a verb, look a word up in a
dictionary, draw a new arithmetic problem each time. The repository has
two examples: `ja-words` makes one card per marked word of a Japanese
sentence, and `llm-generated` asks an LLM to write a new question about a
topic every time the card is shown, while `srl` keeps scheduling it as one
card.

### Sources: items from anywhere

Card types decide what an item's cards are; sources decide where items
are kept. A source program connects `srl` to wherever your items already are,
or would be better kept: a database, a web service, another app's files.
It never schedules anything and never sees the log: it only reads and
writes items, and `srl` does the rest. The repository's
[source-tinydb](examples/source-tinydb/) keeps items in a TinyDB
database, one table per collection, in about a hundred lines of Python.

### Scripts and analysis

Selectors such as `in:rust -is:new` or `is:leech` work the same in
every command, and every command prints JSON with `--json`. Reports,
imports, bulk edits and dashboards are a few lines of shell or any other
language away. Shells built around structured data can work with it
directly: [examples/nushell/](examples/nushell/) is a Nushell wrapper
whose commands give tables and records to filter with `where` and
`sort-by`.

### LLM agents

`srl` knows nothing about agents: an agent is one more program built on
the CLI, and works with a store the way a person does, running commands
and writing items. It can turn a note or an article into questions, sent
as JSONL and kept as drafts until you accept them:

```sh
srl add generated/japanese.md --stdin <<'JSONL'
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "Make a sentence with 〜をものともせず about a football team.", "A": "…"}
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "{{c1::嵐}}をものともせず、漁師たちは海へ出た。"}
JSONL
srl accept path:generated/japanese.md   # after you've read them
```

The same lines can go to a JSONL note instead, `srl add
generated/japanese.jsonl --stdin`, where each item is stored as one line,
just as it was sent. There is then no Markdown for the agent to get
wrong, such as the fences around a field that holds code, and the note
stays easy to read and change with `jq`.

It can also be your frontend, tutoring you in a chat: it takes the next
card, judges your answer in your own words, and records the rating with
`srl review --by agent`. And it can read `srl stats`, `srl log` and
`srl find is:leech` to find what you keep forgetting and propose
rewrites. Giving an agent only a few tools, each running one `srl` command,
keeps it from doing anything `srl` wouldn't accept.

## Predictable by design

One simple rule per decision, no hidden state, and errors that say what to
do about them. A broken note, or a source that fails, stops `srl` loudly
instead of being silently skipped, so the programs built on it can trust
what they read.

## Install

```sh
cargo install srl
```

`srl` works on Linux, macOS and Windows.

## Quick start

```sh
srl init ~/notes          # make a folder of notes into a store
cd ~/notes
$EDITOR rust/ownership.md # write an item: ```srl, Q: and A: lines, then ```
srl id                    # give new items their IDs
srl next                  # the next card to study (frontends use srl next --json)
srl review ID --rating 3  # rate it: 1 Again, 2 Hard, 3 Good, 4 Easy
srl stats                 # what's new, due, young, mature
```

To add items without writing Markdown, send them as JSON lines to any
collection: a Markdown note, where `srl` writes the srl blocks for you, a
JSONL note (once a `jsonl` source reads them, see above), or a collection
of a source program:

```sh
echo '{"see": "rust/ownership.md", "Q": "Can a moved value be used again?", "A": "No."}' \
  | srl add inbox.jsonl --stdin
```

Cloze deletions use Anki's syntax (`{{c1::text}}` and `{{c1::text::hint}}`).
Multi-line fields, code blocks and drafts are supported too. `srl kinds`
describes every type of card and its fields.

## Examples

The repository's [examples/](examples/) are built on `srl` and are not part
of it: copy them and change them.

| Example | What it is |
|---|---|
| [`web-frontend/`](examples/web-frontend/) | `srl-web`: a local web viewer that renders cards and context as Markdown (a JSONL note as a code block), with a card browser and a form that adds items. |
| [`python-frontend/`](examples/python-frontend/) | `srl-tui`: a terminal study frontend in Python with Rich. |
| [`agents/`](examples/agents/) | A system prompt and skills that turn an LLM agent into a guide to `srl`. |
| [`kind-ja-words/`](examples/kind-ja-words/) | A card type made by a program: one card per marked word of a Japanese sentence. |
| [`kind-llm-generated/`](examples/kind-llm-generated/) | A card type made by a program: an LLM writes a new question each time the card is shown. |
| [`source-tinydb/`](examples/source-tinydb/) | A source program that keeps items in a TinyDB database instead of in notes. |
| [`nushell/`](examples/nushell/) | A Nushell wrapper that gives `srl`'s data as tables and records. |
| [`screenshots/`](examples/screenshots/) | Screenshots of `srl-web` and `srl-tui`, and instructions for an agent to take new ones. |

## Documentation

The full specification is in
[docs/](https://github.com/ChrisVilches/srl/tree/main/docs). Start with
[overview.md](https://github.com/ChrisVilches/srl/blob/main/docs/overview.md);
where items come from is in
[sources.md](https://github.com/ChrisVilches/srl/blob/main/docs/sources.md).
Guides to building on `srl`, such as
[writing a frontend](https://github.com/ChrisVilches/srl/blob/main/docs/guides/frontends.md)
or
[operating it with an agent](https://github.com/ChrisVilches/srl/blob/main/docs/guides/agents.md),
are in [docs/guides/](https://github.com/ChrisVilches/srl/tree/main/docs/guides).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
