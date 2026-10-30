---
name: setting-up-a-store
description: Create an srl store, choose between several stores, change the configuration (.srl/config.toml), add kind programs, and keep a store in git or a sync tool. Load when the person sets srl up, has more than one study folder, wants to change how many new cards a day or when a study day starts, asks about backups, or wants a kind of item other than qa.
---

# Setting up a store

## Creating one

```sh
srl init            # in the current folder
srl init ~/study    # in another folder
```

`srl init` creates `.srl/config.toml` (every setting commented out),
`.srl/.gitignore` and an empty `.srl/log/`, and prints their paths. The folder may already hold notes:
a folder of existing Markdown notes becomes a store as it is, and nothing in
it changes until items are added.

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

## Which files are notes

Every file in the store matched by `include` in the configuration (by
default, every `.md` file), except files in folders whose name starts with
`.` (`.srl/`, `.git/`, `.obsidian/`) and files matched by `exclude`, which
always wins. Files must be UTF-8.

The extension decides how items are written in a note: a note whose name
ends in `.jsonl` holds one item object per line, and every other note
holds srl blocks in Markdown (see the `writing-items` skill). JSONL notes
are read only when `include` names them, for example
`include = ["**/*.md", "generated/**/*.jsonl"]` for items that scripts and
agents write. Every file `include` names must be a valid note, so don't
let a glob catch JSON files meant for other programs.

## Configuration

`.srl/config.toml`. Every setting is optional; these are the defaults:

```toml
[notes]
include = ["**/*.md"]        # globs of notes, e.g. ["**/*.md", "**/*.txt"]; replaces the default
exclude = []                 # globs of notes to ignore, e.g. ["templates/**", "inbox.md"]

[schedule]
desired_retention = 0.9      # target chance of remembering a card when it is due
day_starts_at = 4            # hour (local time) when a new study day begins
new_per_day = 10             # new cards introduced per day, across all selectors
max_skip = 0                 # srl next skips up to this many cards, at random
leech_lapses = 8             # a card forgotten this many times is a leech
mature_days = 21             # a card with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights; leave out to use the defaults

[kinds]
# ja-words = ["python3", "kinds/ja_words.py"]   # kind:ja-words runs this command

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
is the kind's name, as items write it (`kind:ja-words`), and the value the
command, a list of the program and its arguments. A program path with a
folder is relative to the store's folder, and the program runs there. The
`srl` repository's `examples/` has two: `kind-ja-words` (one card per
marked word of a Japanese sentence) and `kind-llm-generated` (an LLM
writes a new question each time the card is shown); their READMEs say how
to add them.

- `srl` runs whatever `[kinds]` names, with the person's rights. Before
  using a store copied from someone else, read its `[kinds]` with them.
- `srl kinds` describes every kind and its fields; it is the quickest
  check that a program runs.
- After a program is changed in place, run `srl check`: `srl` keeps the
  programs' answers about items and can't see that one changed.
- `card_timeout` limits how long making one card may take, such as a
  program that calls a web API.

## Git and syncing

The store is text, so git and file sync tools work well. Commit everything
git doesn't ignore: the notes, `.srl/config.toml`, `.srl/.gitignore` and
`.srl/log/*.jsonl` (the log is the whole review history). The store needs
no `.gitignore` of its own for `srl`: `.srl/.gitignore` already ignores
what `srl` writes that is not a source of truth (the cache, the log's lock
and temporary files). `.srl/cache/` is only a cache: deleting it loses
nothing.

Some sync tools skip folders whose names start with `.`. Check that
`.srl/` is synced, or the review history stays on one device. Keep the
store on a local disk rather than a network share: locking on network file
systems is unreliable. Keep the notes on the same file system as `.srl/`
too: `srl` writes a note by renaming a temporary file from `.srl/tmp/` over
it, which fails for a note in a folder mounted from elsewhere.
