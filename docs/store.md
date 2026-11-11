# The store

A **store** is a folder that `srl` studies from, usually your study
folder, with the sources of its items, by default its collections,
structured files of items (see [sources.md](sources.md)), and your notes,
the study material items name as their context (see
[items.md](items.md), "Context"). What makes it a store is a `.srl/`
folder in it, which holds everything that belongs to `srl`. Everything
outside `.srl/` is yours.

You can have several stores, for example one per language or one for work.

## Layout

```
study/
  .srl/                    marks the folder as a store; everything srl owns
    config.toml            configuration
    .gitignore             ignores what follows that is not a source of truth
    log/                   review events, one JSONL file per month
      2026-10.jsonl
      .lock                held while an event is appended (see log.md)
    cache/                 derived data, safe to delete
      index.sqlite
    tmp/                   temporary files of collection writes (see formats.md)
  japanese/grammar.jsonl                    collections of items, in any folders
  japanese/grammar/wo-mono-tomo-sezu.md     your notes, in any folders
  rust/ownership.xml
  rust/ownership.md
```

| Path | Source of truth? | Written by |
|---|---|---|
| collections (`**/*.jsonl` and `**/*.xml` by default) | yes | you, any editor, scripts and other programs; `srl id`, `srl accept`, `srl add` and `srl edit --stdin` (see [formats.md](formats.md), "How srl writes to collections") |
| notes (`**/*.md` and `**/*.txt` by default) | yes | you, any editor, scripts and other programs; never `srl` |
| `.srl/config.toml` | yes | you |
| `.srl/log/` | yes | `srl review`, `srl reset`, `srl move`, `srl suspend` and `srl unsuspend` only. Never edit by hand. |
| `.srl/.gitignore` | no | `srl init`; yours to change |
| `.srl/cache/` | no | `srl`, automatically |
| `.srl/tmp/` | no | `srl`, while it writes a collection |

### Which files are read

`srl` chooses files of the store by globs, relative to the store's
folder, in two places: the collections of each `filesystem` source, by
its `include` and `exclude` (see [formats.md](formats.md), "Which files
are collections"), and the notes, by the `include` and `exclude` of
`[context]` (see "Configuration"). Either way, **a file is chosen when a
glob of `include` matches its path, no glob of `exclude` does, and it is
not in a folder whose name starts with `.`**, so `.srl/` itself, `.git/`
and similar folders are left alone. `*` matches within one folder, `**`
across folders.

## Finding the store

Every command looks for the store in this order and uses the first match:

1. The `--dir DIR` flag.
2. The `SRL_DIR` environment variable.
3. The current directory or its nearest parent that contains a `.srl`
   folder.
4. The default store: a folder `srl` in the platform's directory for
   application data.

| Platform | Default store |
|---|---|
| Linux and other Unix | `$XDG_DATA_HOME/srl`, or `~/.local/share/srl` if `XDG_DATA_HOME` is not set |
| macOS | `~/Library/Application Support/srl` |
| Windows | `%APPDATA%\srl` (for example `C:\Users\NAME\AppData\Roaming\srl`) |

Every command except `srl init` requires the store it finds to exist (to
contain `.srl/config.toml`), and fails otherwise.

`srl init [DIR]` creates a store: `.srl/config.toml` (every key commented
out), `.srl/.gitignore` (see "Version control and syncing") and an empty
`.srl/log/`. Like `git init`, it creates the store in
`DIR`, else in `--dir`, else in `SRL_DIR`, else in the current directory.
The folder may already hold notes and other files, but not `.srl/`.
`srl init` prints each path it creates, and changes nothing else.

## File names

Unicode can write some letters in two forms: composed (NFC, `é` as one
character) and decomposed (NFD, `e` plus an accent). Most systems name files
in NFC, but macOS often gives names in NFD, and a store copied between
systems can hold both.

- `srl` sorts collections' names,
  matches their `in` names against `in:`, matches paths against `include` and `exclude`, and compares `see` with
  the paths of notes, in NFC, so the form of a file's name never changes
  which cards a selector picks or their order. Globs and `in:` values are
  read in NFC too.
- Paths are printed as they are on disk (in `--json` output and errors), so
  that they always name the file.
- A collection created by `srl add` gets the path you give it, as given.

## Configuration: `.srl/config.toml`

Every key is optional. These are the defaults:

```toml
[sources.items]              # the one source of a store without [sources]
driver = "filesystem"        # collections that are files of the store; or program = [...]
include = ["**/*.jsonl", "**/*.xml"]   # globs of collections, relative to the store's folder
exclude = []                 # globs of files to ignore; they win over include

[context]
include = ["**/*.md", "**/*.txt"]      # notes: the files items may name with `see`
exclude = []                 # files to leave out; they win over include

[schedule]
desired_retention = 0.9      # FSRS target probability of recall
day_starts_at = 4            # hour (local time) when a new study day begins
new_per_day = 10             # new cards introduced per day, across all selectors
max_skip = 0                 # srl next skips up to this many cards, at random
leech_lapses = 8             # a card with this many lapses is a leech
mature_days = 21             # a card with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights (21, or 19 or 17); default weights if absent

[kinds]
# ja-words = ["python3", "kinds/ja_words.py"]   # kind:ja-words runs this command

[kind_programs]
card_timeout = 60            # seconds a kind program may take to make a card
```

A key or table that is not listed here is an error, so a typo such as
`new_per_dya = 3` fails instead of being ignored; the names of sources
and the keys of `[kinds]` are the exceptions, since they are names you
choose, and so are the keys of a source program, which are the
program's (see [sources.md](sources.md), "Source programs"). So is a value of the
wrong type or out of range: `desired_retention` must be between 0 and 1
(exclusive), `day_starts_at` an hour from 0 to 23, and `card_timeout` at
least 1. Commands that need the configuration fail with a message
naming the problem.

`[kinds]` names the kinds that are programs: each key is a kind's name,
as written in its `kind` entry, and its value the command to run, a non-empty list
of the program and its arguments. `qa` can't be one of them. How kind
programs are run, and what `card_timeout` limits, is in
[cards.md](cards.md), "Kind programs".

`[sources]` names the sources of items, each with its driver and its
globs of collections, or with the command of a source program; without
it, the store has the one source above. How sources are configured, and
what each key means, is in [sources.md](sources.md), "Configuration".

`[context]` chooses the notes: the files of the store that items may name
with `see` as their context, your study material (see
[items.md](items.md), "Context"). It is a setting of the store, not of a
source, since an item of any source may name any note. `srl notes` lists
them.

In both, globs choose files as "Which files are read" says:

- `include` replaces the default rather than adding to it:
  `include = ["**/*.md", "**/*.pdf"]` in `[context]` makes Markdown and
  PDF files notes, and `include = ["notes/**"]` every file under
  `notes/`; in a source, `include = ["items/**/*.jsonl"]` reads only the
  JSONL collections under `items/`.
- `exclude` always wins: `exclude = ["templates/**", "inbox.md"]` ignores
  every file under `templates/` and the file `inbox.md`, whatever
  `include` says.
- The two may choose the same file, though there is rarely a reason to:
  a collection can be a note, such as a JSONL file shown whole as the
  context of the items that name it, when `[context]` includes it.

## The index cache

`.srl/cache/index.sqlite` holds what is slow to compute from the files: the
log replayed through FSRS (each card's memory, every review, and the log
lines that are not valid events), and the answers of kind programs'
`validate` and `groups` calls. It is never a source of truth, only an
optimization: every command gives the same results with it, without it,
or after it was deleted, as long as no kind program changed in place (see
below). Deleting `.srl/cache/` loses nothing; the next command builds it
again.

- **Every log file is still read on every use**, and compared with the
  cache by a hash of its content (BLAKE3). Modification times are never
  trusted. What the cache saves is replaying, not reading.
- **Collections and notes are not cached.** Collections are read and
  parsed on every use, since parsing a file costs about as much as hashing
  it would, and a note is read only when it is shown.
- **The log:** new lines are replayed on top of the cached memories when
  every log file the cache has read still starts with exactly the bytes it
  read (ending with a newline, unless nothing was added to that file), and
  all new bytes are in the last file it read or in files whose names sort
  after it. Anything else (an edited, shortened, removed or earlier file)
  replays the whole log.
- **Kind programs:** an answer of `validate` or `groups` is kept under
  the kind's name and its command as written in `[kinds]`, the call, and
  the item's input line, and used for as long as all four are the same;
  only the items without a kept answer are asked about (see
  [cards.md](cards.md), "How srl calls them"). Each kept answer also
  names its kind, call and item, only for people reading the cache. A
  program changed in place is not noticed: after changing one, run
  `srl check`, which asks again about every item, or delete
  `.srl/cache/`. Old answers are kept until then.
- **Settings:** the whole log is replayed when `day_starts_at`,
  `desired_retention` or `weights` differ from those the cache was built
  with.
- **Versions:** a cache written by another version of `srl` is rebuilt,
  without a report.
- **Damage:** a cache that SQLite can't read, or whose data doesn't make
  sense, is deleted and rebuilt, and this is reported on standard error.
- **No access:** if the cache can't be created, opened or written (for
  example a read-only store, or another `srl` holding it for more than 5
  seconds), this is reported on standard error and the command reads the
  log directly, and asks kind programs about every item.
- `.srl/cache/` is created by the first command that needs it.
- Each update is one SQLite transaction, so several `srl` processes can
  share the cache.
- `srl log` lists events, so it reads the log files directly. `srl check`
  reads the log through the cache, like other commands, since a log file
  that changed is always noticed; but it asks kind programs about every
  item, keeping their new answers, since a program that changed is not.
- It can be queried directly for exploration
  (`sqlite3 .srl/cache/index.sqlite`), but its schema is not a stable
  contract. Scripts should use the CLI's `--json` output instead.

**TODO (platform):** SQLite's file locking is unreliable on network file
systems (NFS, SMB), on every platform. Two `srl` processes updating the same
cache there could damage it; damage is usually detected and the cache
rebuilt, but not always. Keep stores on a local disk, or decide whether to
put the cache outside the store in that case.

## Version control and syncing

The store works well with git or any file syncing tool:

- Item edits show as small diffs in your collections: a line of a JSONL
  file, or an element of an XML file (see [formats.md](formats.md), "How
  srl writes to collections").
- The log is append-only, so it almost never conflicts on merge.
- Everything `srl` writes that is not a source of truth is in `.srl/`, and
  `srl init` writes `.srl/.gitignore` to ignore it: the cache, the log's
  lock (see [log.md](log.md)), and the temporary files of collection
  writes. `srl` never writes anything else outside `.srl/` but your
  collections, so the store needs no `.gitignore` of its own for `srl`:

  ```gitignore
  cache/
  log/.lock
  tmp/
  ```

- Commit everything else: your collections and notes, `.srl/config.toml`,
  `.srl/.gitignore` and `.srl/log/*.jsonl`. The log is your whole review history; without it, a
  clone starts every card as new.
- Some sync tools skip folders whose names start with `.`. Check that yours
  syncs `.srl/`, or your review history stays on one device.
