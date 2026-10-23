# The store

A **store** is a folder of Markdown notes that `srl` studies from: usually
your study folder. What makes it a store is a `.srl/` folder in it, which
holds everything that belongs to `srl`. Everything outside `.srl/` is yours.

You can have several stores, for example one per language or one for work.

## Layout

```
study/
  .srl/                    marks the folder as a store; everything srl owns
    config.toml            configuration
    log/                   review events, one JSONL file per month
      2026-10.jsonl
      .lock                held while an event is appended (see log.md)
    cache/                 derived data, safe to delete
      index.sqlite
  japanese/grammar/wo-mono-tomo-sezu.md     your notes, in any folders
  rust/ownership.md
```

| Path | Source of truth? | Written by |
|---|---|---|
| notes (`**/*.md`) | yes | you, any editor; `srl id`, `srl accept`, `srl add` and `srl edit ID --stdin` (see [notes.md](notes.md), "How srl writes to notes") |
| `.srl/config.toml` | yes | you |
| `.srl/log/` | yes | `srl review`, `srl reset`, `srl move`, `srl suspend` and `srl unsuspend` only. Never edit by hand. |
| `.srl/cache/` | no | `srl`, automatically |

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
out) and an empty `.srl/log/`. Like `git init`, it creates the store in
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
exclude = []                 # globs of notes to ignore, relative to the store's folder

[schedule]
desired_retention = 0.9      # FSRS target probability of recall
day_starts_at = 4            # hour (local time) when a new study day begins
new_per_day = 10             # new cards introduced per day, across all selectors
leech_lapses = 8             # a card with this many lapses is a leech
mature_days = 21             # a card with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights (21, or 19 or 17); default weights if absent

[study]
context = "prose"            # context in srl next and srl show: prose, full or none (see notes.md)
```

A key or table that is not listed here is an error, so a typo such as
`new_per_dya = 3` fails instead of being ignored. So is a value of the
wrong type or out of range: `desired_retention` must be between 0 and 1
(exclusive), `day_starts_at` an hour from 0 to 23, and `context` one of the
three modes. Commands that need the configuration fail with a message
naming the problem.

`exclude` globs match note paths: `*` matches within one folder, `**`
across folders. `exclude = ["templates/**", "inbox.md"]` ignores every note
under `templates/` and the note `inbox.md`.

## The index cache

`.srl/cache/index.sqlite` holds what is slow to compute from the files: the
log replayed through FSRS (each card's memory, every review, and the log
lines that are not valid events). It is only ever an optimization: every
command gives the same results with it, without it, or after it was
deleted.

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
  files directly.
- `.srl/cache/` is created by the first command that needs it, with a
  `.gitignore` that ignores everything in it.
- Each update is one SQLite transaction, so several `srl` processes can
  share the cache.
- `srl reindex` deletes it and builds it again. Deleting it by hand loses
  nothing.
- `srl log` lists events, so it reads the log files directly; `srl check`
  checks the files, so it reads them directly too.
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
- `.srl/cache/` ignores itself (it holds its own `.gitignore`). Add the
  two other files `srl` may leave in the store to the store's
  `.gitignore`: the log's lock (see [log.md](log.md)), and the temporary
  file of a note write that was killed before it finished (see
  [notes.md](notes.md), "How srl writes to notes"):

  ```gitignore
  .srl/log/.lock
  .srl-*.tmp
  ```

- Commit everything else: your notes, `.srl/config.toml` and
  `.srl/log/*.jsonl`. The log is your whole review history; without it, a
  clone starts every card as new.
- Some sync tools skip folders whose names start with `.`. Check that yours
  syncs `.srl/`, or your review history stays on one device.
