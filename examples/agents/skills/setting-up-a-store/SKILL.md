---
name: setting-up-a-store
description: Create an srl store, choose between several stores, change the configuration (.srl/config.toml), and keep a store in git or a sync tool. Load when the person sets srl up, has more than one study folder, wants to change how many new cards a day or when a study day starts, or asks about backups.
---

# Setting up a store

## Creating one

```sh
srl init            # in the current folder
srl init ~/study    # in another folder
```

`srl init` creates `.srl/config.toml` (every setting commented out) and an
empty `.srl/log/`, and prints both paths. The folder may already hold notes:
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

Every `.md` file in the store, in any folder, except files in folders whose
name starts with `.` (`.srl/`, `.git/`, `.obsidian/`) and files matched by
`exclude` in the configuration. Files must be UTF-8.

## Configuration

`.srl/config.toml`. Every setting is optional; these are the defaults:

```toml
[notes]
exclude = []                 # globs of notes to ignore, e.g. ["templates/**", "inbox.md"]

[schedule]
desired_retention = 0.9      # target chance of remembering a card when it is due
day_starts_at = 4            # hour (local time) when a new study day begins
new_per_day = 10             # new cards introduced per day, across all selectors
leech_lapses = 8             # a card forgotten this many times is a leech
mature_days = 21             # a card with at least this stability (days) is mature
# weights = [...]            # custom FSRS weights; leave out to use the defaults
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

## Git and syncing

The store is text, so git and file sync tools work well. Commit the notes,
`.srl/config.toml` and `.srl/log/*.jsonl` (the log is the whole review
history). Add this to the store's `.gitignore`:

```gitignore
.srl/log/.lock
.srl-*.tmp
```

`.srl/cache/` ignores itself. It is only a cache: deleting it, or
`srl reindex`, loses nothing.

Some sync tools skip folders whose names start with `.`. Check that
`.srl/` is synced, or the review history stays on one device. Keep the
store on a local disk rather than a network share: locking on network file
systems is unreliable.
