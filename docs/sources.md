# Sources

A **source** is a place items come from and are written back to. `srl`
reads every item of every source on every command that reads items, and
everything after reading them (kinds, cards, scheduling, the log,
selectors) is the same whatever their source. Where items are kept, and
how they are written, is each source's business, and no other part of
`srl` knows it.

A store has the sources its configuration names. Without any, it has one,
`notes`, which reads every Markdown file of the store as a note of srl
blocks: a store of Markdown notes needs no configuration, and the words
of this document only show once it has more than notes.

## Terms

| Term | Meaning |
|---|---|
| **Source** | A named place items are read from and written to, configured as `[sources.NAME]`: `notes`, `data`, `vocab`. |
| **Driver** | What a source is: `markdown`, notes of srl blocks, or `jsonl`, notes of JSON lines (see [notes.md](notes.md)), or a **source program** that keeps items its own way (see "Source programs"). A source is a driver with its settings; two sources may use the same driver with different settings. |
| **Collection** | A group of items inside a source, named by its source: for the notes' drivers, a note, named by its path; for a program, whatever it groups items in, such as a table. Collections are written like paths, with `/` between parts. |
| **Prefix** | Where a source is mounted in the store's one tree of addresses: `""` by default, or folders ending in `/`, such as `"vocab/"`. |
| **Address** | The name of a collection everywhere outside its source: its source's prefix followed by the collection's name. `japanese/grammar/wo.md` for a note of a source without a prefix. |
| **File** | The file a collection is kept in, relative to the store's folder, when it is a file of its own, as a note is. Only for people and editors. |

## Configuration

```toml
[sources.notes]
driver = "markdown"
include = ["**/*.md"]
exclude = ["templates/**"]

[sources.data]
driver = "jsonl"
include = ["generated/**/*.jsonl"]
```

Each table of `[sources]` is a source, and its key is the source's name:
words of lowercase letters `a`–`z` and digits, joined by single `-`, as an
item ID is. A source's keys:

| Key | Value | Default |
|---|---|---|
| `driver` | `markdown` or `jsonl`. | none: every source has a `driver` or a `program` |
| `program` | Instead of `driver`: the command of a source program, a list of the program and its arguments (see "Source programs"). | none |
| `include` | Globs of the notes the source reads, relative to the store's folder: `*` matches within one folder, `**` across folders. | `["**/*.md"]` for `markdown`, `["**/*.jsonl"]` for `jsonl` |
| `exclude` | Globs of notes to leave out; they win over `include`. | `[]` |
| `prefix` | The source's place among the store's addresses (see "Addresses"). | `""` |

- **Without `[sources]`, a store has one source**, `notes`, with driver
  `markdown` and the defaults above: every Markdown file is a note.
  `[sources]` replaces it rather than adding to it, so a store with notes
  and JSONL notes names both:
  `[sources.notes]` with `driver = "markdown"`, and `[sources.data]` with
  `driver = "jsonl"`.
- **Which files are notes** is said in [notes.md](notes.md), "Which files
  are notes": those `include` matches and `exclude` doesn't, outside
  folders whose name starts with `.`.
- A key that is not listed is an error, so a typo fails instead of being
  ignored, and so is a glob that isn't valid, a name or prefix that isn't
  made as above, and a source without a `driver` or a `program`, or with
  both. A source program's keys are the exception: see "Source programs".
- A table `[notes]` is an error, whose message says how to write it as
  `[sources.NAME]` tables.

## Addresses

**A store is one tree of addresses, and each source is mounted in it at
its prefix**, the way Unix mounts file systems in one tree of folders.
Every collection is reached by an address, whatever its driver, and
`srl collections` lists the tree. The default prefix is `""`, so a store
of notes is mounted at the root and its addresses are the notes' paths.

The rules, the same for every source:

1. **A prefix is `""` or folders ending in `/`**: `"vocab/"`,
   `"db/jp/"`. A prefix that ends inside a name, such as `"voc"`, is an
   error, so a prefix always names whole folders of the tree.
2. **An address belongs to the sources mounted deepest above it**: those
   whose prefix is the longest one the address starts with. **A source
   that gives an address under another source's deeper prefix is an
   error**, naming both: with `notes` at `""` and a source at
   `"vocab/"`, a note `vocab/readme.md` of `notes` is one. Unix would
   quietly hide the folder under the mount point; `srl` says so instead,
   and you leave the note out of `notes` with `exclude`, or mount the
   other source elsewhere.
3. **Sources may share a prefix**, as two sources of notes at `""` do;
   they share that part of the tree, and **no address may be given by two
   sources**. So a file that two sources' globs both match is an error.
4. **No two collections may be the same file.** It catches two sources
   reading one note under different prefixes.
5. **A prefix needs no folder on disk.** A folder with its name may
   exist, and only matters when a source reads notes in it, which rule 2
   reports.
6. **An address never holds a line.** Every item of a collection has the
   same address, and items are told apart by their IDs. Where `srl`
   prints `ADDRESS:LINE` for people, the `:LINE` is added for display;
   `see`, `srl add` and `path:` take addresses alone.

A collection that breaks one of these rules is an error in the store
(see [notes.md](notes.md), "Errors"): it stops every command that reads
items, and `srl check` reports it, as `ADDRESS: message`.

**A new collection**, for `srl add`, is created in the source mounted
deepest above its address that accepts it: for both drivers, a note whose
path, after the prefix, `include` matches and `exclude` doesn't. Exactly
one source must accept it; none or several is an error.

## What a source does

Every source does the same few things, each its own way:

- **It lists its collections**, including those without items: for both
  drivers, its notes.
- **It reads its items**, each with its collection, the line where it
  starts if it can say, and the item as written, and the problems it
  finds (see [notes.md](notes.md), "Errors").
- **It shows a collection as context**, to the collection's own items and
  to the items that name it with `see`, wherever they come from: the
  context of an item is always asked of the source that owns the
  collection shown, found by its address, so each collection is shown by
  its own source's rules (see [notes.md](notes.md), "Context").
- **It makes changes**: it adds items to a collection, creating it if
  needed, and replaces items. It is given each item to replace as it was
  read, and refuses to change an item that is no longer as it was, since
  someone changed it in between. Every change to a source is made and
  checked before anything is written (see [notes.md](notes.md), "How srl
  writes to notes").
- **It looks for likely mistakes**, for `srl lint`.
- **It describes itself**, for `srl sources`: what it is, and which
  collections `srl add` may create in it, as globs: a driver's `include`
  and `exclude`, or the globs a source program's `describe` gives. `srl
  sources` gives its driver as `markdown`, `jsonl`, or `program` for a
  source program.

**IDs are unique in the store**, across its sources, and not across
stores: each store has its own log, which knows card IDs only.

`srl sources` describes the store's sources and `srl collections` lists
their collections (see [cli.md](cli.md)).

## Source programs

A source can be a program, which keeps items its own way: in a database,
behind a web service, in a file format `srl` doesn't know. It is
configured like a kind program (see [cards.md](cards.md), "Kind
programs"):

```toml
[sources.vocab]
program = ["python3", "sources/tinydb_source.py"]
prefix = "vocab/"
database = "vocab/vocab.json"    # every key but program and prefix is the program's
```

- **`program` is a command**, as in `[kinds]`: the same rules for paths,
  for finding a program on `PATH`, and the store's folder as the folder it
  runs in. A store runs the programs its configuration names, with your
  rights: read the `[sources]` of a store you copy from someone else.
- **Every key but `program` and `prefix` is the program's.** `srl` gives
  them to it, as a JSON object, in every call, and checks nothing in them:
  the program checks them, and a call that fails because of them is
  reported with the program's message.

### Calls

Every call is a batch, run as a kind program's calls are: `srl` starts
the program with the call as its last argument, writes JSON lines to its
standard input and closes it, and reads JSON lines from its standard
output. A status other than 0 is a failure of the whole call, and what
the program printed on standard error is the reason. Every input line
holds `config`, the program's keys.

| Call | Input | Output |
|---|---|---|
| `describe` | one line: `{"config": {…}}` | one line: `{"description": "…", "creates": ["*"]}`: what the source is, in a sentence, and the globs of the collections `srl add` may create in it (`[]` for none), which `srl sources` shows as its `include` |
| `collections` | one line: `{"config": {…}}` | one line per collection: `{"collection": "words", "file": null}`, `file` being the file it is kept in, relative to the store's folder, or `null` |
| `read` | one line: `{"config": {…}}` | one line per item: `{"collection": "words", "handle": "17", "line": null, "written": null, "item": {…}}`; one line per problem: `{"collection": "words", "line": null, "error": "…"}` |
| `context` | one line per request: `{"config": {…}, "collection": "words", "mode": "prose", "own": true}` | one line per input line: `{"context": "…"}` or `{"context": null}` |
| `write` | one line per change: `{"config": {…}, "dry_run": false, "op": "add", "collection": "words", "item": {…}}`, or `{…, "op": "replace", "collection": "words", "handle": "17", "before": {…}, "after": {…}}` | one line per input line: `{"ok": true}`, `{"preview": "…"}` for an `add` with `dry_run`, or `{"error": "…"}` |

- **`read`** gives every item with its collection; its `handle`, what
  names it when it is changed, unique in the source, such as a key of a
  database; its `line`, from 1, or `null`; `written`, the item as the
  program writes it, for `srl show`, or `null`, which shows its item
  object; and `item`, its item object (see [notes.md](notes.md), "The
  item object"), read as strictly as `srl add` reads one. An item may
  come without an `id`, for `srl id` to give it one: a program never makes
  IDs itself. A problem is reported as an error in the store, at its
  collection and line.
- **`context`** gives what a collection shows as context, to its own
  items (`own` is `true`) or to the items that name it with `see`, in
  mode `prose` or `full` (see [notes.md](notes.md), "Context"). Mode
  `prose` never shows items, in any source.
- **`write`** gets every change one command makes to the source, in one
  call. An `item`, `before` or `after` is an item object as `srl` writes
  one. A program refuses a `replace` when the item under the handle is no
  longer `before`, and **a program that answers `error` to any change
  writes none of them**; `srl` reports the first such line. `srl id` and
  `srl accept` are `replace`s too, whose `after` adds an `id` or removes
  `draft`.
- `read` and `collections` answer one input with any number of lines;
  `describe`, `context` and `write` answer each input line with one line,
  in order.

There is no `lint` call: a program reports what it can as problems of
`read`.

### How many calls

A command calls a source program a small, fixed number of times, however
many items or cards it handles, never once per item or per card:

| Command | Calls |
|---|---|
| every command that reads items | 1 `collections` and 1 `read` |
| `next`, `show` and `cards`, when a context is shown | 1 `context`, for every item that needs one |
| `add`, `edit`, `accept`, `id` | 1 `write` for the changes to the source, with 1 `read` before it and 1 after it, to verify it |
| `add` to a new collection | 1 `describe` |
| `collections` | 1 `collections` |
| `sources` | 1 `describe` |

So studying a card, `srl next` and then `srl review`, starts a program
about five times. `srl` keeps nothing a program answers: it can't know
when a database or a service changed. A program that is slow to answer,
such as one that asks a web service, may keep a cache of its own,
wherever it chooses, and is responsible for its freshness.

### What srl checks

- **Every line a program prints is read strictly**: a line that is not a
  JSON object, a key a call doesn't give, a value of the wrong type, an
  item object that isn't valid, a collection's name that isn't parts
  joined by `/` (none empty or starting with `.`), a line below 1, a
  handle given twice in one `read`, or a wrong number of lines is a
  failure of the program. So is a call that fails as a whole. Either is
  reported once, as a problem of the source, and stops every command that
  reads items until it is fixed.
- **An item in a collection the source doesn't list** is an error in the
  store, as is breaking a rule of "Addresses".
- **After `write`, `srl` reads the source again**: every item not changed
  must be as before, every `add` must have added exactly its item, and
  every `replace` must give exactly `after`. If not, the command fails,
  saying that the program didn't do what it was asked.
