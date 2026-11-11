# Sources

A **source** is a place items come from and are written back to. `srl`
reads every item of every source on every command that reads items, and
everything after reading them (kinds, cards, scheduling, the log,
selectors) is the same whatever their source. Where items are kept, and
how they are written, is each source's business, and no other part of
`srl` knows it.

A store has the sources its configuration names. Without any, it has one,
`items`, which reads the structured files of the store, JSONL and XML, as
collections of items (see [formats.md](formats.md)).

Sources hold items only. The study material items ask about, the notes
they name with `see`, is not read through sources: it is any file of the
store that `[context]` allows (see [items.md](items.md), "Context").

## Terms

| Term | Meaning |
|---|---|
| **Source** | A named place items are read from and written to, configured as `[sources.NAME]`: `items`, `generated`, `vocab`. |
| **Driver** | What a source is: `filesystem`, collections that are files of the store (see [formats.md](formats.md)), or a **source program** that keeps items its own way (see "Source programs"). A source is a driver with its settings; two sources may use the same driver with different settings. |
| **Collection** | A group of items inside a source, named by its source, and by that name everywhere: for `filesystem`, a file, named by its path (`japanese/grammar.jsonl`); for a program, whatever it groups items in, such as a table. Collections are named like paths, with `/` between parts. |

## Configuration

```toml
[sources.items]
driver = "filesystem"
exclude = ["generated/**"]

[sources.generated]
driver = "filesystem"
include = ["generated/**/*.jsonl"]

[sources.vocab]
program = ["python3", "sources/tinydb_source.py"]
database = "vocab/vocab.json"    # every key but program is the program's
```

Each table of `[sources]` is a source, and its key is the source's name:
words of lowercase letters `a`–`z` and digits, joined by single `-`, as
an item ID is. **Each driver has its own keys**, and a key it doesn't take
is an error, so a typo fails instead of being ignored:

| Driver | Keys |
|---|---|
| `filesystem` | `driver = "filesystem"`; `include`, globs of the files it reads, relative to the store's folder (`*` matches within one folder, `**` across folders), by default `["**/*.jsonl", "**/*.xml"]`; and `exclude`, globs of files to leave out, which win over `include`, by default `[]`. |
| source program | `program`, the command of the program, a list of the program and its arguments; every other key is the program's, which `srl` gives it and the program checks (see "Source programs"). |

- **Without `[sources]`, a store has one source**, `items`, with driver
  `filesystem` and the defaults above. `[sources]` replaces it rather
  than adding to it.
- **Which files a `filesystem` source reads** is said in
  [formats.md](formats.md), "Which files are collections".
- A glob that isn't valid is an error, and so is a name that isn't made
  as above, an unknown driver, and a source without a `driver` or a
  `program`, or with both.

## Collections

**Every collection of the store is named by its source**, and that name
is the collection's everywhere in `srl`. A `filesystem` source names a
collection by its path, relative to the store's folder, and a source
program by whatever it chooses, such as the name of a table. `srl
collections` lists every collection, with the source that gives it.

**A source also gives each collection its `in` names**: the values of
`in:` that select it (see [selectors.md](selectors.md)). A `filesystem`
source gives each folder of the path, the file name, and the file name
without its extension: `japanese/grammar.jsonl` has `japanese`,
`grammar.jsonl` and `grammar` (see [formats.md](formats.md), "Which files
are collections"). A source program gives the names it chooses with
`collections` (see "Calls"), or, by default, the collection's name alone.
`in` names may be shared: `in:grammar` selects every collection that has
the name, whatever its source.

The rules, the same for every source:

1. **No collection's name may be given by two sources.** Two
   `filesystem` sources whose globs match the same file, or a program that names a collection
   as a file of another source is named, break it; leave the collection
   out of one of them with `exclude`, or have the program name its
   collections otherwise, such as under a folder of its own, `vocab/`.
2. **A collection's name never names an item.** Every item of a
   collection is in the same collection, and items are told apart by
   their IDs. `see` and `srl add` take collections' names
   alone, and `in:` their `in` names.

A collection that breaks a rule is an error in the store (see
[items.md](items.md), "Errors"): it stops every command that reads items,
and `srl check` reports it, as `COLLECTION: message`.

**A new collection**, for `srl add`, is created in the one source that
accepts its name. Each source decides by its own rule: `filesystem`
accepts a path that `include` matches and `exclude` doesn't, in a format
it knows by its extension; a program accepts the names its `accepts`
call takes (see "Calls"). Exactly one source must accept it; none or
several is an error, which names every source concerned, so that no one
has to guess which source was asked:

```
srl: cannot add to notes/a.md: no source accepts it (items: its `include` doesn't match the path; vocab: a table's name has no `/`); `srl sources` describes each one; nothing was added
srl: cannot add to words.jsonl: more than one source accepts it (items, vocab); `srl sources` describes each one; nothing was added
```

When none accepts it, every source gives its reason, after its name:
its `include` doesn't match the path, its `exclude` matches it, or its
extension names no format it knows, for `filesystem`; the reason its
`accepts` gives, for a program. When several do, they are named. A
program whose `accepts` fails, or prints invalid output, fails `srl add`
as itself (see "What srl checks"), not as a source that doesn't accept
the name.

## What a source does

Every source does the same few things, each its own way:

- **It lists its collections**, including those without items.
- **It reads its items**, each with its collection and its item object,
  and the problems it finds (see
  [items.md](items.md), "Errors"). A problem is about a collection, and
  says where in it, as the source can.
- **It makes changes**: it adds items to a collection, creating it if
  needed, and replaces items. It is given each item to replace as it was
  read, and refuses to change an item that is no longer as it was, since
  someone changed it in between. Every change to a source is made and
  checked before anything is written (see [items.md](items.md), "How srl
  changes items").
- **It decides which new collections it creates**, for `srl add` (see
  "Collections").
- **It describes itself**, for `srl sources`: what it is, and the
  settings it shows, each by its name, as it chooses: the `filesystem`
  source's `include`, and its `exclude` if it has some, or the settings
  a source program's `describe` gives. Settings are not the same for
  every source, since each driver has its own keys, and a program may
  leave out any of its own, such as a secret. `srl sources` gives its
  driver as `filesystem`, or `program` for a source program.

A source doesn't show its collections as context, and doesn't look for
likely mistakes in them: context is a file `see` names (see
[items.md](items.md), "Context"), and a source reports what it finds as
problems of `read`, which `srl check` lists with the likely mistakes it
finds in every item (see [cli.md](cli.md), "Checking").

**IDs are unique in the store**, across its sources, and not across
stores: each store has its own log, which knows card IDs only.

`srl sources` describes the store's sources and `srl collections` lists
their collections (see [cli.md](cli.md)).

## Source programs

A source can be a program, which keeps items its own way: in a database,
behind a web service, in a file format `srl` doesn't know, such as items
written in Markdown notes. It is configured like a kind program (see
[cards.md](cards.md), "Kind programs"):

```toml
[sources.vocab]
program = ["python3", "sources/tinydb_source.py"]
database = "vocab/vocab.json"    # every key but program is the program's
```

- **`program` is a command**, as in `[kinds]`: the same rules for paths,
  for finding a program on `PATH`, and the store's folder as the folder it
  runs in. A store runs the programs its configuration names, with your
  rights: read the `[sources]` of a store you copy from someone else.
- **Every key but `program` is the program's.** `srl` gives them to it,
  as a JSON object, in every call, and checks nothing in them: the
  program checks them, refusing the keys it doesn't take, and a call that
  fails because of them is reported with the program's message.
- **A program names its collections as it likes**, as paths with `/`
  between parts, and `srl` names them so everywhere. A program that may share a
  store with files of a `filesystem` source names its collections so that
  they can't be taken for a file, such as under a folder of its own, or
  takes the folder in its settings.
- **A program gives each collection's `in` names as it likes**, or
  leaves them out to have the collection's name alone. A program whose
  collections are files, such as one that reads items from Markdown
  notes, gives what a `filesystem` source does: `notes/routers.md` has
  `notes`, `routers.md` and `routers`. One that keeps a database may give
  a table's name alone, as the default does, or the topics it knows it
  by.

### Calls

Every call is a batch, run as a kind program's calls are: `srl` starts
the program with the call as its last argument, writes JSON lines to its
standard input and closes it, and reads JSON lines from its standard
output. A status other than 0 is a failure of the whole call, and what
the program printed on standard error is the reason. Every input line
holds `config`, the program's keys. It is stopped as a kind program is,
with every process it started, when `srl` is stopped (see
[cards.md](cards.md), "Stopping a program").

| Call | Input | Output |
|---|---|---|
| `describe` | one line: `{"config": {…}}` | one line: `{"description": "…", "settings": {"database": "vocab/vocab.json"}}`: what the source is, in a sentence, and the settings `srl sources` shows of it (`{}` for none) |
| `accepts` | one line: `{"config": {…}, "collection": "words"}` | one line: `{"ok": true}` if `srl add` may create the collection in it, or `{"error": "…"}`, why not |
| `collections` | one line: `{"config": {…}}` | one line per collection: `{"collection": "words"}`, or `{"collection": "words", "in": ["words", "vocab"]}` to give its `in` names |
| `read` | one line: `{"config": {…}}` | one line per item: `{"collection": "words", "handle": "17", "item": {…}}`; one line per problem: `{"collection": "words", "error": "…"}` |
| `write` | one line per change: `{"config": {…}, "dry_run": false, "op": "add", "collection": "words", "item": {…}}`, `{…, "op": "replace", "collection": "words", "handle": "17", "before": {…}, "after": {…}}`, or `{…, "op": "remove", "collection": "words", "handle": "17", "before": {…}}` | one line per input line: `{"ok": true}`, `{"preview": "…"}` for an `add` with `dry_run`, or `{"error": "…"}` |

- **`read`** gives every item with its collection; its `handle`, what
  names it when it is changed, unique in the source, such as a key of a
  database; and `item`, its item object
  (see [items.md](items.md), "The item object"), read as strictly as `srl
  add` reads one. An item may come without an `id`, for `srl id` to give
  it one: a program never makes IDs itself. A problem is reported as an
  error in the store, at its collection; a program says in its message
  where in the collection it is, if that helps.
- **`collections`** gives every collection, and may give its `in` names
  as a list of at least one, each a value `in:` can take: without
  spaces or commas. A collection without `in` has its name alone.
- **`write`** gets every change one command makes to the source, in one
  call. An `item`, `before` or `after` is an item object as `srl` writes
  one. A program refuses a `replace` or a `remove` when the item under
  the handle is no longer `before`, and **a program that answers `error`
  to any change writes none of them**; `srl` reports the first such line.
  `srl id` and `srl accept` are `replace`s too, whose `after` adds an `id`
  or removes `draft`. `srl move` gives the source its collection's
  `add`s, and the `remove`s of items moved out of its collections (see
  [cli.md](cli.md), "Moving items").
- **`describe`** gives `settings` as an object, each key a setting's
  name, in the order `srl sources` shows them, and each value any JSON:
  usually some of the program's keys as it read them. `srl` checks
  nothing in them.
- **`accepts`** is asked only of a collection that doesn't exist yet, by
  its name, and decides by the program's own rule, such as a folder its
  settings name. Its `error` is shown after the source's name when no
  source accepts the collection (see "Collections").
- `read` and `collections` answer one input with any number of lines;
  `describe`, `accepts` and `write` answer each input line with one
  line, in order.

There is no call for context, since context is a file of the store, nor
for likely mistakes: a program reports what it can as problems of `read`.

### How many calls

A command calls a source program a small, fixed number of times, however
many items or cards it handles, never once per item or per card:

| Command | Calls |
|---|---|
| every command that reads items | 1 `collections` and 1 `read` |
| `add`, `edit`, `move`, `accept`, `id` | 1 `write` for the changes to the source, with 1 `read` before it and 1 after it, to verify it |
| `add` to a new collection | 1 `accepts` |
| `collections` | 1 `collections` |
| `sources` | 1 `describe` |

So studying a card, `srl next` and then `srl review`, starts a program
about four times. `srl` keeps nothing a program answers: it can't know
when a database or a service changed. A program that is slow to answer,
such as one that asks a web service, may keep a cache of its own,
wherever it chooses, and is responsible for its freshness.

### What srl checks

- **Every line a program prints is read strictly**: a line that is not a
  JSON object, a key a call doesn't give, a value of the wrong type, a
  text that is blank, empty or only spaces (every text a call gives, such
  as a `handle` or an `error`, says something), `settings` that aren't
  an object, an item object that isn't valid, a collection's name that isn't parts
  joined by `/` (none empty or starting with `.`), a collection listed
  twice by `collections`, an `in` that is empty, holds a value `in:`
  can't take, or gives a name twice, a handle given twice in one `read`, or a wrong
  number of lines is a failure of the program. So
  is a call that fails as a whole. Either is reported once, as a problem
  of the source, and stops every command that reads items until it is
  fixed.
- **An item in a collection the source doesn't list** is an error in the
  store, as is breaking a rule of "Collections".
- **After `write`, `srl` reads the source again**: every item not changed
  must be as before, every `add` must have added exactly its item, every
  `replace` must give exactly `after`, and every `remove` must have
  removed exactly its item. If not, the command fails,
  saying that the program didn't do what it was asked.
