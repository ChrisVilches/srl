# The store

A **store** is a directory that holds one `srs` database. It is owned and
managed by `srs`: commands create and update its files. Because every file is
plain text, you can also read, search and edit them directly, and version the
whole store with git.

You can have several stores, for example one per language or one for work.

## Layout

```
my-store/
  srs.toml               configuration; its presence marks the directory as a store
  atoms/                 one Markdown file per atom, in any subfolders
    japanese/grammar/wo-mono-tomo-sezu.md
  log/                   review events, one JSONL file per month
    2026-10.jsonl
  drills/                drills that belong to this store (optional)
    srs-drill-conj
  generated/             cached cards per atom, written by agents or scripts (disposable)
    a7f3k2.jsonl
  sessions/              prepared lists of cards to play (optional)
    n1-food.jsonl
  .cache/                derived data, safe to delete
    index.sqlite
```

| Path | Source of truth? | Written by |
|---|---|---|
| `srs.toml` | yes | you |
| `atoms/` | yes | `srs add`, `srs new`, `srs edit`, `srs accept`, or any editor |
| `log/` | yes | `srs review`, `srs reset` only. Never edit by hand. |
| `drills/` | yes | you |
| `generated/` | no, disposable | `srs generated put` |
| `sessions/` | yes, but disposable in practice | `srs session put` |
| `.cache/` | no | `srs`, automatically |

## Finding the store

Every command looks for the store in this order and uses the first match:

1. The `--dir DIR` flag.
2. The `SRS_DIR` environment variable.
3. The current directory or its nearest parent that contains `srs.toml`.
4. The default store, `~/.local/share/srs`.

`srs init [DIR]` creates a store with a default `srs.toml` and empty folders.

## Configuration: `srs.toml`

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

- `srs` refreshes it automatically: atom files whose modification time changed
  are re-read, and new log lines are replayed.
- `srs reindex` rebuilds it from scratch.
- Deleting it loses nothing.
- It can be queried directly for exploration (`sqlite3 .cache/index.sqlite`),
  but its schema is not a stable contract. Scripts should use the CLI's
  `--json` output instead.

## Version control

The store is designed to work well with git:

- Atom edits show as ordinary line diffs.
- The log is append-only, so it almost never conflicts on merge.
- Add `.cache/` to `.gitignore`. Whether to track `generated/` and `sessions/`
  is up to you; both can be recreated.
