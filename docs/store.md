# The store

A **store** is a directory that holds one `srl` database. It is owned and
managed by `srl`: commands create and update its files. Because every file is
plain text, you can also read, search and edit them directly, and version the
whole store with git.

You can have several stores, for example one per language or one for work.

## Layout

```
my-store/
  srl.toml               configuration; its presence marks the directory as a store
  atoms/                 one Markdown file per atom, in any subfolders
    japanese/grammar/wo-mono-tomo-sezu.md
  log/                   review events, one JSONL file per month
    2026-10.jsonl
    .lock                held while an event is appended (see log.md)
  drills/                drills that belong to this store (optional)
    srl-drill-conj
  sessions/              prepared lists of cards to study (optional)
    n1-food.jsonl
  .cache/                derived data, safe to delete
    index.sqlite
```

| Path | Source of truth? | Written by |
|---|---|---|
| `srl.toml` | yes | you |
| `atoms/` | yes | `srl add`, `srl new`, `srl edit`, `srl accept`, `srl suspend`, `srl unsuspend`, or any editor |
| `log/` | yes | `srl review`, `srl reset` only. Never edit by hand. |
| `drills/` | yes | you |
| `sessions/` | yes, but disposable in practice | `srl session put` |
| `.cache/` | no | `srl`, automatically |

## Finding the store

Every command looks for the store in this order and uses the first match:

1. The `--dir DIR` flag.
2. The `SRL_DIR` environment variable.
3. The current directory or its nearest parent that contains `srl.toml`.
4. The default store: a folder `srl` in the platform's directory for
   application data.

| Platform | Default store |
|---|---|
| Linux and other Unix | `$XDG_DATA_HOME/srl`, or `~/.local/share/srl` if `XDG_DATA_HOME` is not set |
| macOS | `~/Library/Application Support/srl` |
| Windows | `%APPDATA%\srl` (for example `C:\Users\NAME\AppData\Roaming\srl`) |

Every command except `srl init` requires the store it finds to exist (to
contain `srl.toml`), and fails otherwise.

`srl init [DIR]` creates a store with a default `srl.toml` (every key
commented out) and empty `atoms/` and `log/` folders. Like `git init`, it
creates the store in `DIR`, else in `--dir`, else in `SRL_DIR`, else in the
current directory. The directory may already hold other files, but not
`srl.toml`. `srl init` prints each path it creates.

## File names

Unicode can write some letters in two forms: composed (NFC, `é` as one
character) and decomposed (NFD, `e` plus an accent). Most systems name files
in NFC, but macOS often gives names in NFD, and a store copied between
systems can hold both.

- `srl` sorts atom paths, and matches them against `path:` globs, in NFC,
  so the form of a file's name never changes which atoms a selector picks
  or their order. A `path:` glob is read in NFC too.
- Paths are printed as they are on disk (in `--json` output and errors), so
  that they always name the file.
- New files (`srl add`, `srl new`) get names in NFC.

## Configuration: `srl.toml`

Every key is optional. These are the defaults:

```toml
[schedule]
desired_retention = 0.9      # FSRS target probability of recall
day_starts_at = 4            # hour (local time) when a new study day begins
new_per_day = 10             # new atoms introduced per day, across all selectors
leech_lapses = 8             # an atom with this many lapses is a leech
mature_days = 21             # an atom with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights (21, or 19 or 17); default weights if absent

[grading]
cmd = ""                     # external grader for `cmd` cards; empty means fall back to self-grading

[drills]
timeout_seconds = 10         # a drill that takes longer is skipped
```

A key or table that is not listed here is an error, so a typo such as
`timeout = 3` fails instead of being ignored. So is a value of the wrong
type or out of range: `desired_retention` must be between 0 and 1
(exclusive), `day_starts_at` an hour from 0 to 23, and `timeout_seconds` at
least 1. Commands that need the configuration fail with a message naming the
problem.

## The index cache

`.cache/index.sqlite` holds what is slow to compute from the files: each atom
file's parsed front-matter, and the log replayed through FSRS (each atom's
memory, every review, and the log lines that are not valid events). It is
only ever an optimization: every command gives the same results with it,
without it, or after it was deleted.

- **Every file is still read on every use**, and compared with the cache by
  a hash of its content (BLAKE3). Modification times are never trusted. What
  the cache saves is parsing and replaying, not reading.
- **Atoms:** an atom file whose content is cached reuses its parsed
  front-matter; any other file is parsed again. Files that are not valid
  atoms are not cached, so they are read and reported every time.
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
- `.cache/` is created by the first command that needs it, with a
  `.gitignore` that ignores everything in it.
- Each update is one SQLite transaction, so several `srl` processes can
  share the cache.
- `srl reindex` deletes it and builds it again. Deleting it by hand loses
  nothing.
- `srl log` lists events, so it reads the log files directly; `srl check`
  checks the files, so it reads them directly too.
- It can be queried directly for exploration (`sqlite3 .cache/index.sqlite`),
  but its schema is not a stable contract. Scripts should use the CLI's
  `--json` output instead.

**TODO (platform):** SQLite's file locking is unreliable on network file
systems (NFS, SMB), on every platform. Two `srl` processes updating the same
cache there could damage it; damage is usually detected and the cache
rebuilt, but not always. Keep stores on a local disk, or decide whether to
put the cache outside the store in that case.

## Version control

The store is designed to work well with git:

- Atom edits show as ordinary line diffs.
- The log is append-only, so it almost never conflicts on merge.
- `.cache/` ignores itself (it holds its own `.gitignore`). Add `log/.lock`
  to the store's `.gitignore`. Whether to track `sessions/` is up to you;
  sessions can be recreated.
