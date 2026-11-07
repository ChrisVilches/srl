---
name: setting-up-a-store
description: Create an srl store, choose between several stores, change the configuration (.srl/config.toml), its sources (collection files, source programs) and which files are notes, add kind programs, and keep a store in git or a sync tool. Load when the person sets srl up, has more than one study folder, wants to change how many new cards a day or when a study day starts, asks about backups, wants a kind of item other than qa, or wants items kept somewhere else, such as a database.
---

# Setting up a store

## Creating one

```sh
srl init            # in the current folder
srl init ~/study    # in another folder
```

`srl init` creates `.srl/config.toml` (every setting commented out),
`.srl/.gitignore` and an empty `.srl/log/`, and prints their paths. The
folder may already hold notes: a folder of existing Markdown notes becomes
a store as it is, and `srl` never changes them. Items go in collections of
their own, beside them.

## Which store a command uses

The first that applies:

1. `--dir DIR` on the command.
2. The `SRL_DIR` environment variable.
3. The current folder, or the nearest folder above it with a `.srl/` in it
   (like git).
4. The default store, a folder `srl` in the platform's data folder:
   `~/.local/share/srl` on Linux (or `$XDG_DATA_HOME/srl`),
   `~/Library/Application Support/srl` on macOS, `%APPDATA%\srl` on
   Windows.

So the simplest setup is to `cd` into the store. Someone with one store who
wants it reachable from anywhere sets `SRL_DIR` in their shell profile, or
creates the default store with `srl init` at that path. Several stores (one
per language, one for work) are just several folders; each has its own
schedule, configuration and log.

Every command except `srl init` fails if the store it finds doesn't exist,
and the message says which folder it looked at.

## Collections and notes

A store holds two kinds of files:

- **Collections** hold the items. They come from the store's sources, set
  in `[sources]` of `.srl/config.toml`. Without any, the store has one,
  `items`, with the built-in `filesystem` driver: every `.jsonl` and
  `.xml` file is a collection (see the `writing-items` skill for the
  formats), except files in folders whose name starts with `.` (`.srl/`,
  `.git/`, `.obsidian/`) and files matched by `exclude`, which always
  wins. JSON collections (`.json`) are read when `include` names them,
  such as `include = ["**/*.jsonl", "**/*.xml", "items/**/*.json"]`.
  Every file a source's `include` matches must be a valid collection, so
  don't let a glob catch JSON files meant for other programs. `srl
  sources` describes the sources, and `srl collections` lists every
  collection by its name.
- **Notes** are the person's study material, which items name with `see`
  as their context. They hold no items. `[context]` chooses them, by
  default every `.md` and `.txt` file; `include = ["**/*.md", "**/*.pdf"]`
  makes PDFs notes too. `srl notes` lists them.

Files must be UTF-8 (a note that isn't is not shown as text).

`[sources]` replaces the default source, so a store whose agent-written
items live apart names both, for example `[sources.mine]` with
`exclude = ["generated/**"]` and `[sources.generated]` with
`include = ["generated/**/*.jsonl"]`, both `driver = "filesystem"`. No
collection may be read by two sources.

A source can also be a **source program**, which keeps items its own way,
such as in a database: `program` instead of `driver`, a command as in
`[kinds]`. Every other key of its table is the program's own setting,
which the program checks. It names its collections itself, and says in
`describe` which new ones `srl add` may create in it (`srl sources` shows
them as its `include`). Its items are studied, shown and changed like any
other, with `srl edit --stdin` since they are in no file. The `srl`
repository's `examples/` has one, `source-tinydb` (items in a TinyDB
database, one table per collection, the table `words` being the
collection `words`); its README says how to add it:

```toml
[sources.vocab]
program = ["uv", "run", "--quiet", "--script", "sources/tinydb_source.py"]
database = "vocab/vocab.json"    # the program's own setting; each table is a collection
```

**A new collection goes to the one source that accepts its name**; none
or several is an error, `cannot add to COLLECTION`, that names every
source concerned (and, for none, why each refused). `source-tinydb`
accepts any name without a `/` (its glob is `*`), so beside the default
`filesystem` source a new collection at the top of the store, such as
`words.jsonl`, is accepted by both and refused: put new files in a
folder (`generated/words.jsonl`), and a name without an extension
(`words`) goes to the database.

## Configuration

`.srl/config.toml`. Every setting is optional; these are the defaults:

```toml
[sources.items]              # the one source of a store without [sources]
driver = "filesystem"        # collections that are files of the store
include = ["**/*.jsonl", "**/*.xml"]   # globs of collections; replaces the default
exclude = []                 # globs of files to ignore, e.g. ["templates/**"]

[context]
include = ["**/*.md", "**/*.txt"]      # notes: the files items may name with `see`
exclude = []                 # files to leave out, e.g. ["inbox.md"]

[schedule]
desired_retention = 0.9      # target chance of remembering a card when it is due
day_starts_at = 4            # hour (local time) when a new study day begins
new_per_day = 10             # new cards introduced per day, across all selectors
max_skip = 0                 # srl next skips up to this many cards, at random
leech_lapses = 8             # a card forgotten this many times is a leech
mature_days = 21             # a card with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights; leave out to use the defaults

[kinds]
# ja-words = ["python3", "kinds/ja_words.py"]   # the kind ja-words runs this command

[kind_programs]
card_timeout = 60            # seconds a kind program may take to make a card
```

- An unknown setting, a typo or a value out of range is an error, so a
  mistake is reported instead of ignored.
- `desired_retention`: higher means more reviews and better recall. 0.9 is
  a good default; going higher costs many more reviews for a little more
  recall.
- `new_per_day`: the main knob for daily workload. Each new card brings
  reviews in the following days and weeks, so a beginner should keep it
  low (5–10) at first.
- `day_starts_at`: a session after midnight but before this hour counts as
  the day before.
- Changing `desired_retention` or `weights` applies to the whole history
  at once: schedules are always recomputed from the log.

## Kind programs

`qa` is built in. Any other kind is a program named in `[kinds]`: the key
is the kind's name, as items write it (`"kind": "ja-words"`), and the value the
command, a list of the program and its arguments. A program path with a
folder is relative to the store's folder, and the program runs there. The
`srl` repository's `examples/` has two: `kind-ja-words` (one card per
marked word of a Japanese sentence) and `kind-llm-generated` (an LLM
writes a new question each time the card is shown); their READMEs say how
to add them.

- `srl` runs whatever `[kinds]` and `[sources]` name, with the person's
  rights. Before using a store copied from someone else, read its `[kinds]`
  and `[sources]` with them.
- `srl kinds` describes every kind and its fields; it is the quickest
  check that a program runs.
- After a program is changed in place, run `srl check`: `srl` keeps the
  programs' answers about items and can't see that one changed.
- `card_timeout` limits how long making one card may take, such as a
  program that calls a web API.

## Git and syncing

The store is text, so git and file sync tools work well. Commit everything
git doesn't ignore: the collections and notes, `.srl/config.toml`, `.srl/.gitignore` and
`.srl/log/*.jsonl` (the log is the whole review history). The store needs
no `.gitignore` of its own for `srl`: `.srl/.gitignore` already ignores
what `srl` writes that is not a source of truth (the cache, the log's lock
and temporary files). `.srl/cache/` is only a cache: deleting it loses
nothing.

Some sync tools skip folders whose names start with `.`. Check that
`.srl/` is synced, or the review history stays on one device. Keep the
store on a local disk rather than a network share: locking on network file
systems is unreliable. Keep the collections on the same file system as
`.srl/` too: `srl` writes a collection by renaming a temporary file from
`.srl/tmp/` over it, which fails for one in a folder mounted from
elsewhere.
