# Plan: sources, data access as adapters

**Status: proposed, for review.** Nothing here is implemented. Every
decision marked **Decision** is open to change; the recommended choice is
given first, and the others after it. The steps at the end are in the
order to carry them out. Each one is a commit, and updates the
specification, the guides and the examples as it lands (see
[../../AGENTS.md](../../AGENTS.md)). When the last step lands, this
document is deleted.

## Why

`srl` reads items from two formats today, srl blocks in Markdown notes and
JSONL notes, chosen by a note's extension. Each new format has blurred the
boundary of what `srl` is:

- The `Syntax` trait (see [../rust.md](../rust.md), "The item syntax")
  assumes every format is a text file whose items take whole lines. A
  format that is not a file (a database, a web service) can't implement
  it, and a file format that doesn't fit lines can only be bent to it.
- Files leak into the rest of the specification: `path` and `line` in
  every JSON output, `PATH:LINE` in messages, `path:` and `in:` selectors,
  `include` and `exclude` in `[notes]`, `srl add NOTE`, `srl notes`, `see`
  as a note path, and the review `hash` defined by how an item is written.
- Rules of one format reach into another: a JSONL item's `see` names a
  Markdown note, and its context is that note with its srl blocks removed,
  a Markdown rule.

The core of `srl` is what happens to items once they have been read:
their model (entries, fields, IDs, status), kinds and cards, selectors,
scheduling and the log, reviewing, and the frontend contract. Where items
come from, and how they are written back, is **data access**, and it
should sit behind one boundary, as a database sits behind a repository in
Clean Architecture or hexagonal architecture: the core defines a port, and
each format is an adapter that implements it.

This plan draws that boundary, turns Markdown and JSONL into two adapters
behind it, and adds a third kind of adapter, a program, with two proofs of
concept very unlike files: a document database and a web API.

## Goals

- **The core knows no format.** Nothing outside an adapter knows fences,
  JSON lines, files or databases. The core gets items, with where they
  came from, and asks the adapter that owns them to change them.
- **One port, small and fixed.** An adapter answers a handful of
  operations. Markdown, JSONL and programs all implement the same ones.
- **Adapters are independent.** No adapter depends on another. When an
  item of one adapter needs something of another (its context, for
  example), the core asks the other, through the port.
- **Formats are freer.** Without a shared `Syntax`, each built-in adapter
  reads and writes its format its own way. Some code is duplicated between
  them; that is accepted, since the port, not shared scaffolding, is what
  holds them together. Helpers that are not about a format (atomic writes,
  globs, reading UTF-8 with either line ending) stay shared.
- **New formats without changing `srl`.** A program named in the
  configuration can be an adapter, as kind programs are kinds.
- **The CLI stays easy.** A store with Markdown notes and no configuration
  works as today, and keeps today's commands, paths and messages. The
  new ideas (sources, collections) only show once a store uses more than
  notes.
- **Frontends keep working**, including adding items and opening an
  item for editing.

Not goals:

- A plugin system beyond programs run as processes, like kind programs.
  No dynamic libraries, no embedded language.
- Transactions across adapters. A command that changes items in two
  sources may leave the first changed if the second fails, as a command
  that changes two notes may today.
- Moving an item between sources or collections with one command.

## The model

### Terms

| Term | Meaning |
|---|---|
| **Source** | A named place items are read from and written to, configured in `.srl/config.toml` as `[sources.NAME]`: `notes`, `generated`, `vocab`. |
| **Driver** | What a source is: one of the built-in drivers, `markdown` or `jsonl`, or a **source program**, named by its command, as a kind program is. The driver is the adapter; a source is a driver with its settings. Two sources may use the same driver with different settings. |
| **Collection** | A group of items inside a source, named by a string the driver chooses: a note's path for the built-in drivers, a table or a collection name in a database, a resource in a web API. Collections are written like paths, with `/` between parts (`japanese/grammar/wo.md`, `words`, `decks/n5`), so that selectors can match them the same way for every driver (see "Selectors"). |
| **Address** | The text that names a collection on the command line, in `see` and in messages: **the collection's file, when it has one, or else `SOURCE:COLLECTION`**. `japanese/grammar/wo.md`, `vocab:words`. |
| **Handle** | An opaque string a driver gives with each item it reads, and gets back when it is asked to change that item: a database's own key, a URL, a position in a note. The core never reads it. |

The current term **note** stays for what the built-in drivers read: a
note is a file that a `markdown` or `jsonl` source reads, and it is one
collection. People who only use Markdown never need the word collection.

### Where an item comes from

Every item the core holds carries its **origin**: its source, its
collection, its handle, and, when the driver can give them, the file it
is in (relative to the store's folder) and the line it starts on. File
and line are for people and for editors only, as `line` is today (see
[../rust.md](../rust.md), "Lines are for people"): nothing finds an item
by them.

So an item is named three ways, each for its own use:

- **By its ID**, everywhere a person or a program names one item: `srl
  show ID`, `srl edit`, `id:` in selectors, the log. IDs stay unique in
  the whole store, across sources: the log knows card IDs only, and a
  card must belong to one item.
- **By source, collection and handle**, between the core and a driver,
  to change it. The core finds these from the ID.
- **By its address and line**, for people: `japanese/grammar/wo.md:12`,
  `vocab:words`.

### The core holds every item in memory

As today, a command that reads items reads all of them, from every
source, into memory first: the **catalog**. Selectors, scheduling,
`srl find` and everything else work on the catalog, never on a source.
So a source never has to answer a query, only "give me all your items",
which keeps the port small enough for any driver, including a program.

## Architecture

```
                    ┌──────────────────────────────────────────────┐
  srl (CLI)  ─────► │ application: one function per command        │
                    │   load the catalog, apply selectors, decide  │
                    │   what changes, ask the sources to make them │
                    │ ┌──────────────────────────────────────────┐ │
                    │ │ domain (pure, no I/O)                    │ │
                    │ │   items, IDs, status, the qa kind, cloze,│ │
                    │ │   cards, selectors, FSRS replay, next,   │ │
                    │ │   siblings, stats                        │ │
                    │ └──────────────────────────────────────────┘ │
                    │ ports: Source, Kinds, Log, Cache             │
                    └──────┬───────────┬──────────┬──────────┬─────┘
                           │           │          │          │
        adapters:   markdown, jsonl,   kind       .srl/log/  .srl/cache/
                    source programs    programs   files      SQLite
```

- **The domain** is pure: values in, values out. It doesn't read files,
  start processes or know a clock. It is what most of
  [../audit.md](../audit.md), L7 asks for.
- **The application** runs a command: it loads the configuration, builds
  the sources and the catalog, calls the domain, and calls the ports to
  change things. It holds the rules that cross sources: IDs unique in the
  store, every item checked before anything is written, `see` resolved to
  a source.
- **The ports** are traits in `srl-core`; **the adapters** implement
  them. `Source` is new. Kinds, the log and the index cache already sit
  behind modules of their own; this plan only moves them under the same
  layout, without changing what they do.

The exact module layout is decided in step 2.2 and written into
[../rust.md](../rust.md) then. A sketch:

```
srl-core/src/
  domain/     item.rs, card.rs, kind.rs (qa), cloze.rs, selector.rs,
              schedule.rs, next.rs, stats.rs
  app/        catalog.rs, find, show, add, edit, accept, id, check, …
  source.rs   the Source port and its types
  source/     markdown.rs, jsonl.rs, program.rs, files.rs (shared file helpers)
  kind/       program.rs (kind programs)
  log.rs, index.rs, config.rs, store.rs
```

Splitting the domain into a crate of its own, so that the compiler
guarantees it does no I/O, is a possible last step (step 5.1).

## The Source port

Six operations. The Rust trait and the program protocol are the same
operations with the same meaning, so a built-in driver is exactly what a
program could be.

| Operation | Gives |
|---|---|
| `describe` | What the source is, for `srl sources`: a description, and what its collections are ("Markdown files matched by `**/*.md`", "tables of vocab.json"). Also whether `srl add` may create a collection that doesn't exist yet, and what new collection names look like, for forms (see "Frontends"). |
| `collections` | Every collection of the source, including those without items, each with its file when it has one. |
| `read` | Every item of the source, each with its collection, handle, entries and fields, and optionally its file, line and text as written; and every problem found, each with its collection and optionally a line. |
| `context` | For a list of (collection, mode, own or named by `see`), the text to show as context, or nothing. |
| `write` | Applies a list of changes, or, with `dry_run`, says what it would write. |
| `lint` | Likely mistakes in the source, each with its collection and optionally a line. Built-in drivers only at first (see "Source programs"). |

### Changes

There are two changes, and every command that writes is made of them:

- **`add`**: append an item object to a collection, creating the
  collection if the driver allows it.
- **`replace`**: replace the item with this handle, which was read as
  item object `before`, with item object `after`.

`srl id` and `srl accept` are `replace`s whose `after` differs from
`before` only in `id` or `draft`; `srl edit --stdin` is a `replace` of
anything. A driver decides how much of its format to rewrite. The
`markdown` driver keeps today's rule (see [../notes.md](../notes.md),
"How srl writes to notes"): a change of `id` or `draft` alone changes the
header line only, and anything else rewrites the item's block.

**`before` makes changes safe without a lock.** A driver must refuse a
`replace` when the item under the handle is no longer `before`, since
someone changed it after the command read it. The built-in drivers already
do more: they compare the whole file. A program compares however it can,
for example a document's revision.

**The core checks every change before the first is written**, as today:
item objects valid, IDs unique in the store, kinds accept the fields,
`see` resolves. Sources are then written one by one. A source program's
work is verified by reading it again after `write`: the items must be
the ones read before, with exactly the changes asked for (see "What srl
checks").

### Context

**The context of an item is asked of the source that owns the collection
shown**, never of the item's own source. An item without `see` gets its
own collection's context from its own source (`own: true`); an item with
`see` gets the named collection's context from the source that owns that
collection (`own: false`). So an item of a database can name a Markdown
note, and the Markdown driver alone removes srl blocks from it: no driver
knows another's rules, and today's behaviour is kept.

This answers the worry that context makes drivers depend on each other:
the dependency goes through the core, which only routes the question.

Modes stay `prose`, `full` and `none`, and each driver says what they
mean for its collections, as each syntax does today. `none` never asks a
source.

> **Decision: keep removing items from context, per driver
> (recommended).** Alternatives: drop the rule, so every context is its
> collection as stored, items included; simpler, but every Markdown
> context then shows the answers of the other items of the note, which
> is the problem of [../audit.md](../audit.md), M2, made universal. Or
> keep only files as contexts (`see` a path, shown as is); same
> objection, and a database item could never have context of its own.

## Built-in drivers

`markdown` and `jsonl` keep everything they do today: syntax, errors,
context, how they write, `srl lint`. What changes is how a note gets one:
**the source's driver decides, not the file's extension.**

```toml
[sources.notes]
driver = "markdown"
include = ["**/*.md"]
exclude = ["templates/**"]

[sources.generated]
driver = "jsonl"
include = ["generated/**/*.jsonl"]
```

- `include` and `exclude` become settings of the two file drivers, with
  the same meaning as today in `[notes]`.
- **A file belongs to at most one source.** Two sources whose globs both
  match a file are an error naming both, so a path of the store always
  names one collection. That is what makes addresses work (a file path
  names its source without saying it) and what keeps `srl add
  japanese/new.md` working.
- **Without `[sources]`, a store has one source, `notes`**, with driver
  `markdown` and `include = ["**/*.md"]`: today's default.
- `[notes]` is removed. A configuration that still has it is an error
  that says how to rewrite it, including how to split a mixed `include`
  of `.md` and `.jsonl` globs into two sources.
- A `.txt` or `.qmd` note works as today, as long as a `markdown` source
  includes it.

Each driver is one module that implements the whole port with no help but
the shared file helpers. The `Syntax` trait, `syntax::of` and `Edit` as
the unit every syntax gives are gone; `markdown.rs` and `jsonl.rs` may
still compute line edits internally, and use the shared helpers to apply
them atomically, but that is their choice, not a contract.

The handle of an item of a built-in driver is its position among the
note's items, as `write.rs` names it to a syntax today; it is still found
again by its ID, or, for `srl id`, by having none.

## Source programs

A source can be a program, configured like a kind program:

```toml
[sources.vocab]
program = ["python3", "sources/tinydb_source.py"]
database = "vocab.json"          # every other key is the program's
```

- `program` is a command, as in `[kinds]`: same rules for paths, `PATH`
  and the store's folder as the working directory, same warning that a
  store runs the programs its configuration names (see
  [../cards.md](../cards.md), "Kind programs", and
  [../audit.md](../audit.md), H3).
- **Every key but `program` is the program's.** `srl` passes the whole
  table, as JSON, in every call, and checks nothing in it. The program
  checks it, and a call that fails because of it is reported as a problem
  of the source, with the program's message. So the configuration of a
  web API source can be checked by the program locally, or by asking the
  service: that is the program's choice, not `srl`'s.

### Calls

The same batch protocol as kind programs (see [../cards.md](../cards.md),
"How srl calls them"): the program is started with the call as its last
argument, reads JSON lines on standard input up to its end, prints JSON
lines on standard output, and exits with 0; any other status is a
failure of the whole call, with standard error as the reason.

| Call | Input | Output |
|---|---|---|
| `describe` | one line: `{"config": {…}}` | one line: `{"description": "…", "collections": "…", "creates": true}` |
| `collections` | one line: `{"config": {…}, "cache_dir": "…"}` | one line per collection: `{"collection": "words", "file": null}` |
| `read` | one line: `{"config": {…}, "cache_dir": "…"}` | one line per item: `{"collection": "words", "handle": "17", "line": null, "file": null, "item": {item object}}`; one line per problem: `{"collection": "words", "line": null, "error": "…"}` |
| `context` | one line per request: `{"config": {…}, "cache_dir": "…", "collection": "words", "mode": "prose", "own": true}` | one line per input line: `{"context": "…"}` or `{"context": null}` |
| `write` | one line per change: `{"config": {…}, "cache_dir": "…", "dry_run": false, "op": "add", "collection": "words", "item": {…}}` or `{…, "op": "replace", "collection": "words", "handle": "17", "before": {…}, "after": {…}}` | one line per input line: `{"ok": true}`, `{"preview": "…"}` with `dry_run`, or `{"error": "…"}` (the item changed since it was read, the collection can't be created, …) |

`read` and `collections` answer one input with any number of lines, so
the rule "one output line per input line" holds only for `context` and
`write`. `item` is the item object of [../notes.md](../notes.md), "The
item object", as `srl add` takes it, and is checked as strictly. An item
may come without an `id`, for `srl id` to give it one; the program never
makes IDs itself, so the rules for IDs stay in one place.

`lint` is left out of the protocol at first: a program reports what it
can as problems of `read`. It can be added once a program needs it.

### How many calls

This is what decides whether a program, or a web service behind one, is
fast enough. **A command calls a source program a small, fixed number of
times, however many items or cards it handles**: never once per item or
per card.

| Command | Calls per source program |
|---|---|
| any command that reads items (`find`, `show`, `cards`, `next`, `review`, `stats`, `log`, `accept`, `suspend`, `unsuspend`, `reset`, `check`, `id`, `add`, `edit`) | 1 `read` |
| `next`, `show`, `cards`, when a context is shown | + 1 `context`, for every item at once; `srl next` asks one |
| `add`, `edit`, `accept`, `id` | + 1 `write` for the changes to this source, + 1 `read` to verify them |
| `collections` | 1 `collections` |
| `sources` | 1 `describe` |

A study loop is `srl next` and `srl review`: 2 `read`s and 1 `context`
per card, so 3 process starts. With Python (about 20 ms each) that is
about 60 ms per card; with a web API behind it, 3 round trips, a few
hundred milliseconds unless the program keeps a cache. That is usable, and
the price is visible to whoever chooses the program.

### Caching

**`srl` doesn't cache what sources read.** It can't know when a database
or a service changed, and the index cache must never change a result
(see [../store.md](../store.md), "The index cache"). A program may cache
for itself: every call gets `cache_dir`, a folder
`.srl/cache/sources/NAME/` that the program owns, that is ignored by git
with the rest of `.srl/cache/`, and that may be deleted at any time. A web
API program can keep an ETag and the last answer there, and ask the
service only "has it changed?". Whether a cached answer is fresh is the
program's responsibility.

What `srl` keeps today stays: the answers of kind programs, under the
item's input line, which doesn't depend on where the item came from.

### What srl checks

`srl` trusts a program no more than a kind program:

- Every output line is parsed strictly: unknown keys, wrong types, item
  objects that aren't valid, a collection that isn't a valid collection
  name, a line below 1, a handle given twice in one `read`, and a wrong
  number of lines for `context` and `write` are failures of the program,
  reported once as problems of the source. Like an error in a note, they
  stop every command that reads items.
- An ID given by two items, in one source or in two, is an error, as
  today.
- After `write`, `srl` reads the source again and compares: every item
  not changed must be as before, every `add` must have added exactly its
  item, every `replace` must give exactly `after`. If not, the command
  fails, saying that the source program didn't do what it was asked; this
  is the rule "it verifies its own work" of
  [../rust.md](../rust.md), "Note editing", moved to the core so that it
  covers every program.
- A test kit for program authors, `srl-test-source` (see step 4.2), is
  the reference for the protocol in `srl`'s own tests.

## What changes for users

### Configuration

- `[sources.NAME]` with `driver` or `program`, as above. Source names are
  made like kind names, and can't contain `:`.
- `[notes]` is removed (see "Built-in drivers").

### Commands

| Today | After |
|---|---|
| `srl notes [--json]` | `srl collections [--json]`: one address per line, sorted by source name, then collection; with `--json`, `{"source": "notes", "collection": "japanese/grammar/wo.md", "file": "japanese/grammar/wo.md"}`. |
| | `srl sources [--json]`: each source, its driver or program, and its description, as `srl kinds` describes kinds. |
| `srl add NOTE --stdin` | `srl add ADDRESS --stdin`: a file of the store (the source whose globs match it gets the items, and creates the file if it doesn't exist), or `SOURCE:COLLECTION`. `srl add japanese/new.md` keeps working. |
| `srl edit --stdin` | Unchanged: items are named by `id`, and the core finds their source. |
| `srl id`, `srl accept` | Unchanged for people; `srl id` gives IDs to items of any source, through `replace` and the handle. |
| `srl show` | The first line is the item's address and line (`japanese/grammar/wo.md:12`, or `vocab:words` without a line). The item as written is the text its driver gives, or, when a source program gives none, its item object on one line. |
| `srl lint` | Asks every source that lints; programs don't, at first. The `qa` lints that look at `Q` move to the core and are reported at the item's address. |

### Output and messages

- **An item's place is written as its address, with `:LINE` when the
  driver gives a line**: `japanese/grammar/wo.md:12` exactly as today for
  notes, `vocab:words` for a database. This one rule is used in `srl
  find`, `srl show`, `srl id` and every error, so editors that read
  `FILE:LINE:` messages, such as Neovim's quickfix list, keep working for
  every note.
- JSON outputs (`srl find`, `srl show`, `srl next`, the item object's
  keys only `srl` gives, `make`'s input):

  | Today | After |
  |---|---|
  | `path` | `source` and `collection`, plus `file` (`null` when the collection is no file) |
  | `line` | `line`, which may be `null` |
  | `context_path` | `context_source` and `context_collection` |

  A frontend decides how to render a context from `context_collection`,
  as it does from `context_path` today (an extension, or nothing, then
  plain text).

> **Decision: the address rule, file or `SOURCE:COLLECTION`
> (recommended).** Alternative: always `SOURCE:COLLECTION`
> (`notes:japanese/grammar/wo.md:12`): one form instead of two, but
> noisier for the common store, and no longer a path editors can open.

> **Decision: `collection` and `source` as separate keys
> (recommended).** Alternatives: keep the key `path` for the collection,
> which stays right for notes but reads oddly for a table; or one key
> `source` holding a display string, which is the trap of a string made
> for people that programs then can't use. Every value `srl` prints must
> be usable as input: a collection works in `srl add`, `see` and
> `path:`.

### `see`

**`see` is an address**: a file of the store that some source reads, or
`SOURCE:COLLECTION`. Every `see` written today is a path of a note, so
it keeps working unchanged, as long as some source reads that note. A
`see` that names no collection of any source is an error, as today.

The rule that tells the two apart: **an address whose text before its
first `:` is the name of a source is `SOURCE:COLLECTION`**; anything else
is a file path.

> **Decision: `see` naming files that no source reads, and URLs, is left
> out.** Both would need the core to read things itself (a plain file,
> the network), which is the data access this plan moves out of it. An
> `https://` link can still be written in a field.

### Selectors

| Term | After |
|---|---|
| `source:NAME` | New: cards whose item comes from source `NAME`. |
| `path:GLOB` | Matches the item's collection, for every driver: `path:japanese/**`, `path:decks/*`. |
| `in:NAME` | A part of the collection between `/`, or its last part without its extension: as today for notes, and `in:n5` matches the collection `decks/n5` of a database. |

Collections are written like paths precisely so that `path:` and `in:`
keep meaning one thing for every driver.

> **Decision: no selectors of a driver's own (recommended).** The idea
> was to let each driver add terms, run by every driver that knows them.
> But selectors work on the catalog, in memory, after reading; a term a
> driver alone understands would have to ask the driver for every
> command, would mean different things in different sources, and would
> break "a selector means the same thing everywhere". If a driver has
> something worth selecting on, it can put it in the collection name.

### The log

The review `hash` is defined today as the hash of "the item as written",
which depends on the syntax ([../audit.md](../audit.md), L1). It becomes
the hash of the item object as `srl` writes it (entries and fields, no
keys that only `srl` gives), the same for every driver, so that moving an
item between sources or formats doesn't change it. Old events keep their
hashes; `srl` never uses them.

## Frontends

What a frontend needs, and how it gets it after the change:

- **Studying**: unchanged, but for the keys renamed above.
- **Adding an item**: `srl-web`'s form picks a kind from `srl kinds` and a
  note from `srl notes`. It picks an address from `srl collections`
  instead, and gives it to `srl add`. A source's `describe` says whether
  new collections may be created (`creates`) and what they look like, so
  a form can offer a "new collection" box for the sources that allow it,
  without knowing any driver. No other metadata is needed: a source
  program's settings live in the configuration, not in each item.
- **Opening an item for editing**: a frontend opens `file` at `line` in an
  editor when `file` isn't `null`. Otherwise, or always if it prefers, it
  uses the round trip that works for every source: `srl show ID --json`,
  then `srl edit --stdin`. `srl-web` could offer that as a form, with
  the fields of the item's kind from `srl kinds`.
- **Browsing**: group by source and collection instead of by path.

## Proofs of concept

### A document database, in `examples/`

`examples/source-tinydb/`: a source program in Python over
[TinyDB](https://tinydb.readthedocs.io/), a small document database kept
in one JSON file. It is about as unlike a note as a store can get while
still being a file a person can back up.

- **Collections are TinyDB tables**, and `creates` is true: `srl add
  vocab:words` creates the table `words`.
- **The handle is TinyDB's document ID.** `before` is compared with the
  stored document before each `replace`.
- **Items are documents** holding the item object, plus the database's
  own keys, which the program leaves out of `read`.
- **Context**: a table may hold one document `{"_context": "…"}`, whose
  text is the context of the table's items in modes `prose` and `full`;
  as a `see` target, the same.
- Its README shows a store that mixes it with Markdown notes: database
  items whose `see` names a note, and notes whose items `see` a table.

Other engines considered: SQLite with JSON columns (not really a document
database, and `srl` already uses SQLite for its cache, which would blur
what the example shows), PoloDB or `sled` from Rust (more code for the
same point). TinyDB keeps the example short enough to read in one sitting.

### A web API, in `srl`'s tests

Not an example but an integration test, so that the plugin system has
coverage that runs on every `cargo test`:

- `srl-test-source`, a test program like `srl-test-kind`, gets an `http`
  mode: it is a client of a small REST API (`GET /items`, `GET
  /collections`, `POST /collections/NAME/items`, `PUT /items/HANDLE` with
  the `before` as a precondition, `GET /collections/NAME/context`).
- The test starts that API itself, a small server on `127.0.0.1` built
  with the standard library, so the test needs no network and no service
  on the internet. `srl` itself still calls no service: the program does.
- The test checks what this plan promises: the number of requests per
  command (one `read` for a `find`, three calls for a study step), a
  `replace` refused because the server's copy changed, the configuration
  checked by the program against the server, and a cache in `cache_dir`
  that saves requests when nothing changed.

## Decisions for review

All in one place; each is explained where it comes up.

| Decision | Recommended | Alternatives |
|---|---|---|
| Names | source, driver, collection, address, handle | strategy for driver; store or resource for collection |
| Address | a file, or `SOURCE:COLLECTION` | always `SOURCE:COLLECTION` |
| JSON keys | `source`, `collection`, `file`, nullable `line` | keep `path`; one display string |
| `line` | kept, optional, for people | dropped everywhere |
| Context | each driver keeps its own rules; the core routes `see` to the owning source | context always shown as stored |
| `see` | an address; no plain files, no URLs | any file of the store; URLs |
| Selectors | `source:` added; `path:` and `in:` on collections | selectors of each driver |
| Configuration | `[sources.NAME]`, `driver` or `program`; `[notes]` removed | keep `[notes]` as the default source's table |
| A file in two sources | an error | first source wins |
| Order of `srl find` | by source name, then collection, then items as read | by address |
| Caching reads | by programs only, in `cache_dir` | by `srl`, with a freshness token from `read` |
| Plugins | processes, batch calls, as kind programs | daemon, HTTP, embedded language (all as postponed for kinds, see [../ideas.md](../ideas.md)) |
| Verifying writes | `srl` reads a program's source again after `write` | trust the program's `ok` |
| Crate split | optional, last | first |

## Steps

Each step is a commit that builds, passes the four checks of
[../rust.md](../rust.md), "After every change", and leaves the
specification, guides and examples describing what it does. The first
phase changes only names; the second only the code's structure; the
specification changes for real from the third on.

### Phase 1: names, nothing else

Small, breaking renames that the end state needs anyway, done while the
rest of `srl` stays as it is. Each defines its new term for notes alone,
so the specification stays true in between.

- **1.1 `path` becomes `collection`, `context_path` becomes
  `context_collection`**, in every JSON output and in `make`'s input. A
  note's collection is its path, so values don't change. Docs:
  [../notes.md](../notes.md), [../cli.md](../cli.md),
  [../frontends.md](../frontends.md), [../cards.md](../cards.md).
  Examples: `srl-web`, `srl-tui`, the Nushell module, the kind programs
  that read `context_path`, the agent skills.
- **1.2 `srl notes` becomes `srl collections`**, with `{"collection":
  …}` in JSON. Examples: `srl-web`'s add form, Nushell, agent skills.
- **1.3 The review `hash` is the hash of the item object** (see "The
  log"). Removes [../audit.md](../audit.md), L1.
- **1.4 Selectors are described on collections**: `path:` and `in:` say
  "the item's collection" (a note's path, for now). Docs only.

### Phase 2: the boundary, in the code

No change to what `srl` stores, accepts or prints. The existing tests are
the safety net: none of them changes in this phase, except unit tests
that move with their code.

- **2.1 Items carry an origin**: `Item`'s `path` and `line` become an
  `Origin` (collection, handle, file, line), filled from the note as
  today.
- **2.2 Domain and application layout**: move the pure modules under
  `domain/`, and the command functions that orchestrate under `app/`;
  no logic changes. [../rust.md](../rust.md)'s layout and the rule "one
  module per document" are rewritten to name layers and areas.
- **2.3 The catalog**: `Notes` becomes `Catalog`, built from what
  sources read, and holding the rules that cross sources (unique IDs,
  order, lookups). Still one source inside.
- **2.4 The `Source` trait, with one adapter `Files`** that wraps today's
  `Syntax` dispatch by extension. Reading goes through it.
- **2.5 Context through the port**: `context.rs` resolves `see` to a
  source and asks it; `Files` answers with the syntaxes' rules.
- **2.6 Writes through the port**: `write.rs` splits into the
  application's part (check everything, choose IDs, build `add` and
  `replace` changes) and `Files`'s part (find the item, apply, verify,
  write atomically).
- **2.7 Lint through the port.** The `qa` lints move to the core, looking
  at items rather than lines.
- **2.8 Two adapters, `markdown` and `jsonl`**, each implementing the
  whole port; `Files` and the `Syntax` trait are deleted, and the shared
  file helpers stay in `source/files.rs`. Inside, still chosen by
  extension from `[notes]`, so nothing visible changes.
  [../rust.md](../rust.md), "The item syntax", becomes "Sources".

### Phase 3: sources in the specification

The breaking revamp. Each step changes the specification, the tests, the
guides and the examples together.

- **3.1 `[sources]`**: the configuration, the default `notes` source,
  drivers chosen by configuration instead of by extension, one source per
  file, `[notes]` refused with a migration message, `srl sources`. New
  spec document `docs/sources.md` (sources, drivers, collections,
  addresses, the port's meaning); [../notes.md](../notes.md) narrows to
  the two built-in drivers; [../store.md](../store.md)'s configuration
  and layout; [../rationale.md](../rationale.md), "Why JSONL notes, and
  why the extension decides", rewritten as why sources and drivers;
  [../overview.md](../overview.md)'s principles ("the store is made of
  text files" becomes: the log and configuration are, and items are
  wherever their sources keep them, text files by default), glossary,
  pieces and contracts.
- **3.2 Sources in outputs**: `source` and `file` keys, `line` may be
  `null`, addresses in human output and messages, the `source:` selector,
  `srl collections` across sources, `srl find`'s order.
- **3.3 `see` is an address**, and context is asked of the source that
  owns the collection; `context_source`. The open question "one context
  rule for both item syntaxes" in [../overview.md](../overview.md) and
  [../notes.md](../notes.md) is answered by it (each driver's own rule,
  routed by the core), and [../audit.md](../audit.md), M1, is rewritten
  or removed: two drivers are two sources, not two models inside one.
- **3.4 `srl add ADDRESS`.**
- **3.5 Examples and guides**: `srl-web` (browse by source and
  collection, add form from `srl collections` and `srl sources`, open
  `file` only when there is one), `srl-tui`, Nushell, the agent skills,
  the screenshots' demo store, [../guides/frontends.md](../guides/frontends.md)
  and [../guides/agents.md](../guides/agents.md).

### Phase 4: source programs

- **4.1 The protocol and the `program` driver**: the five calls, strict
  parsing, `cache_dir`, verifying writes by reading again. Specified in
  `docs/sources.md`; [../ideas.md](../ideas.md), "Item syntaxes for
  other file formats, as plugins" and "Items without a line", are
  replaced by what was built, keeping what is still only an idea.
- **4.2 `srl-test-source`** and its tests in `tests/cli.rs`: every rule
  of the protocol, invalid output, failing calls, call counts, a scenario
  in `tests/scenarios.rs` with a program source beside notes.
- **4.3 The web API proof of concept**, as an integration test (see
  "Proofs of concept").
- **4.4 `examples/source-tinydb/`**, listed in `examples/README.md`,
  [../overview.md](../overview.md) and the README.
- **4.5 `srl-web`'s edit form** for items without a file, through `srl
  show --json` and `srl edit --stdin`.

### Phase 5: optional

- **5.1 The domain as a crate of its own**, so that the compiler, not a
  convention, keeps it free of I/O, as the split between `srl` and
  `srl-core` keeps clap out of the core.
- **5.2 `lint` in the program protocol**, if a program needs it.
