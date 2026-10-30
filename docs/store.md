# The store

A **store** is a folder of notes, Markdown files by default, that `srl`
studies from: usually your study folder. What makes it a store is a `.srl/`
folder in it, which holds everything that belongs to `srl`. Everything outside `.srl/` is yours.

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
    tmp/                   temporary files of note writes (see notes.md)
  japanese/grammar/wo-mono-tomo-sezu.md     your notes, in any folders
  rust/ownership.md
```

| Path | Source of truth? | Written by |
|---|---|---|
| notes (`**/*.md` by default) | yes | you, any editor, scripts and other programs; `srl id`, `srl accept`, `srl add` and `srl edit --stdin` (see [notes.md](notes.md), "How srl writes to notes") |
| `.srl/config.toml` | yes | you |
| `.srl/log/` | yes | `srl review`, `srl reset`, `srl move`, `srl suspend` and `srl unsuspend` only. Never edit by hand. |
| `.srl/.gitignore` | no | `srl init`; yours to change |
| `.srl/cache/` | no | `srl`, automatically |
| `.srl/tmp/` | no | `srl`, while it writes a note |

Which files are notes is described in [notes.md](notes.md), "Which files
are notes". Folders that start with `.` are never read for notes, so
`.srl/` itself, `.git/` and similar folders are left alone.

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

- `srl` sorts note paths, and matches them against `path:`, `in:` and
  `exclude`, in NFC, so the form of a file's name never changes which cards
  a selector picks or their order. Globs and `in:` values are read in NFC
  too.
- Paths are printed as they are on disk (in `--json` output and errors), so
  that they always name the file.
- A note created by `srl add` gets the path you give it, as given.

## Configuration: `.srl/config.toml`

Every key is optional. These are the defaults:

```toml
[notes]
include = ["**/*.md"]        # globs of notes, relative to the store's folder
exclude = []                 # globs of notes to ignore; they win over include

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
`new_per_dya = 3` fails instead of being ignored; the keys of `[kinds]`
are the exception, since they are names you choose. So is a value of the
wrong type or out of range: `desired_retention` must be between 0 and 1
(exclusive), `day_starts_at` an hour from 0 to 23, and `card_timeout` at
least 1. Commands that need the configuration fail with a message
naming the problem.

`[kinds]` names the kinds that are programs: each key is a kind's name,
as written in its `kind` entry, and its value the command to run, a non-empty list
of the program and its arguments. `qa` can't be one of them. How kind
programs are run, and what `card_timeout` limits, is in
[cards.md](cards.md), "Kind programs".

`include` and `exclude` globs match paths relative to the store's folder:
`*` matches within one folder, `**` across folders. The notes are the files
`include` matches, minus those `exclude` matches (see
[notes.md](notes.md), "Which files are notes"):

- `include` replaces the default rather than adding to it:
  `include = ["**/*.md", "**/*.txt"]` reads Markdown and text files,
  `include = ["notes/**/*.md"]` only the Markdown files under `notes/`,
  and `include = ["**/*.md", "items/**/*.jsonl"]` also the JSONL notes
  under `items/` (see [notes.md](notes.md), "Item syntaxes").
- `exclude` always wins: `exclude = ["templates/**", "inbox.md"]` ignores
  every file under `templates/` and the file `inbox.md`, whatever
  `include` says.

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
- **Notes are not cached.** They are read and parsed on every use: parsing
  a note costs about as much as hashing it would.
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
  files directly, and asks kind programs about every item.
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

- Item edits show as ordinary line diffs in your notes.
- The log is append-only, so it almost never conflicts on merge.
- Everything `srl` writes that is not a source of truth is in `.srl/`, and
  `srl init` writes `.srl/.gitignore` to ignore it: the cache, the log's
  lock (see [log.md](log.md)), and the temporary files of note writes (see
  [notes.md](notes.md), "How srl writes to notes"). `srl` never writes
  anything else outside `.srl/` but your notes, so the store needs no
  `.gitignore` of its own for `srl`:

  ```gitignore
  cache/
  log/.lock
  tmp/
  ```

- Commit everything else: your notes, `.srl/config.toml`,
  `.srl/.gitignore` and `.srl/log/*.jsonl`. The log is your whole review history; without it, a
  clone starts every card as new.
- Some sync tools skip folders whose names start with `.`. Check that yours
  syncs `.srl/`, or your review history stays on one device.
