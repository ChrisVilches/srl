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
| `atoms/` | yes | `srl add`, `srl new`, `srl edit`, `srl accept`, or any editor |
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

## Configuration: `srl.toml`

Every key is optional. These are the defaults:

```toml
[schedule]
desired_retention = 0.9      # FSRS target probability of recall
day_starts_at = 4            # hour (local time) when a new study day begins
new_per_day = 10             # new atoms introduced per day, across all selectors
leech_lapses = 8             # an atom with this many lapses is a leech
mature_days = 21             # an atom with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights; default weights if absent

[grading]
cmd = ""                     # external grader for `cmd` cards; empty means fall back to self-grading

[drills]
timeout_seconds = 10         # a drill that takes longer is skipped
```

## The index cache

`.cache/index.sqlite` holds a parsed copy of the atoms and log, plus each
atom's current memory state, so that commands stay fast with many thousands of
atoms.

- `srl` refreshes it automatically: atom files whose modification time changed
  are re-read, and new log lines are replayed.
- `srl reindex` rebuilds it from scratch.
- Deleting it loses nothing.
- It can be queried directly for exploration (`sqlite3 .cache/index.sqlite`),
  but its schema is not a stable contract. Scripts should use the CLI's
  `--json` output instead.

## Version control

The store is designed to work well with git:

- Atom edits show as ordinary line diffs.
- The log is append-only, so it almost never conflicts on merge.
- Add `.cache/` to `.gitignore`. Whether to track `sessions/` is up to you;
  sessions can be recreated.
