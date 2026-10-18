# The command line

The CLI is the only interface to a store. People, scripts, frontends and
agents all use the same commands. There is no separate API.

## Conventions

- **`--json` everywhere.** Every command that prints data can print JSON
  instead (JSONL for lists). The default output is for people.
- **Never ask questions when standard input is not a terminal.** Interactive
  commands (`srl study`, `srl new`, `srl edit ID`) refuse in a pipe; every
  action has a non-interactive form.
- **Standard output carries results only.** Errors and warnings go to standard
  error, as one plain message each, saying what to do about it.
- **Exit codes:** 0 success (or something found), 1 nothing found or nothing
  left, 2 error.
- **`--dry-run`** on every command that writes: show what would change, change
  nothing.
- **`--dir DIR`** on every command, to choose the store (see
  [store.md](store.md)).
- **Extension like git.** `srl NAME` runs `srl-NAME` from `PATH` if `NAME` is
  not a built-in command. Frontends and other tools plug in this way. `PATH`
  is searched the way the platform's shell searches it: on Windows,
  `srl-NAME` may have any extension listed in `PATHEXT` (`.exe`, `.cmd`,
  `.bat`, …).
- **Platform independence.** `srl` behaves the same on Linux, macOS and
  Windows. Where it can't, the document says so in a note marked
  **TODO (platform)**.

## Commands

### Store

| Command | Does |
|---|---|
| `srl init [DIR]` | Create a store in `DIR`, `--dir`, `SRL_DIR` or the current directory (see [store.md](store.md)). |
| `srl check [--fix]` | Validate atoms, drills and cards. `--fix` assigns missing IDs. |
| `srl reindex` | Rebuild the index cache. |

### Atoms

| Command | Does |
|---|---|
| `srl new [--draft]` | Write a new atom in `$EDITOR`. |
| `srl add [--draft] --cue C [--answer A] [--tag T]… [--source S]` | Add an atom from flags. |
| `srl add [--draft] --stdin` | Add atoms from JSONL (see [atoms.md](atoms.md)). Prints the new IDs. |
| `srl edit ID` | Edit an atom in `$EDITOR`. |
| `srl edit ID --stdin` | Replace an atom's file with standard input, exactly as given. It must be a valid atom with the same `id`. The file keeps its path, even if the tags or cue change. Prints nothing. `--dry-run` only checks the text. |
| `srl show ID… [--json]` | Print atoms: each file as it is, or JSON (see [atoms.md](atoms.md)). Unknown IDs are reported, and make the exit code 2. |
| `srl find [SELECTOR] [--json]` | List matching atoms, ordered by path: one line each with ID, cue and tags, separated by two spaces; or the atom JSON of [atoms.md](atoms.md). Exit 1 if none matches. |
| `srl tags [--json]` | List every tag in use, sorted, with the number of atoms that have it (any status): `COUNT  TAG` per line, or `{"tag": T, "count": N}` per line. Exit 1 if there are none. |
| `srl drafts [SELECTOR] [--json]` | List drafts: the same as `srl find SELECTOR status:draft`. |
| `srl accept SELECTOR` | Make matching drafts active. |
| `srl suspend SELECTOR` | Make matching active atoms suspended. |
| `srl unsuspend SELECTOR` | Make matching suspended atoms active. |

`srl accept`, `srl suspend` and `srl unsuspend` apply the selector exactly as
written. They change only the matching atoms that have the status they
start from, and leave the rest alone: `srl suspend tag:x` doesn't touch
drafts. Each prints the IDs it changed, one per line, ordered by path, and
exits with 1 if it changed none. With `--dry-run` it prints the IDs it would
change. Only the `status` line of each file changes: its value is replaced,
or, if the file has none, a line is added right after the opening `---`.
Every file is checked before the first one is written. If any can't be
changed safely (see [rust.md](rust.md), "Front-matter editing"), the command
reports it and changes nothing.

> **TODO (platform):** `srl new` and `srl edit ID` open `$EDITOR`, which is
> usually set on Unix but rarely on Windows. Decide what happens when it is
> not set (an error that says to set it, or `VISUAL`, or a platform default
> such as `notepad`), and how `$EDITOR` values with arguments
> (`code --wait`) are split on each platform.

### Studying

| Command | Does |
|---|---|
| `srl study [SELECTOR]` | The built-in terminal frontend (see [frontends.md](frontends.md)). |
| `srl study --session NAME` | Study a prepared session with the same frontend (see below). |
| `srl next [SELECTOR] [--drill NAME] [--json]` | Print the next card. Exit 1 if there is none. |
| `srl cards ID [--json]` | Print every card of an atom, from every applicable drill. |
| `srl grade` | Grade an answer: card and response as JSON on stdin (see [grading.md](grading.md)). |
| `srl review --stdin` | Record a review: `{"card": …, "rating": N, "response": […], "by": "human"}`. |
| `srl review ID… --rating N [--by agent]` | Record a review without a card. |
| `srl reset SELECTOR` | Make matching atoms new again. |

### Sessions

| Command | Does |
|---|---|
| `srl session put NAME` | Save a list of cards (JSONL from stdin) as a session. Cards are validated. |
| `srl session list` / `srl session rm NAME` | Manage sessions. |

### Drills

| Command | Does |
|---|---|
| `srl drills [--json]` | List available drills and where they were found. |

### Analysis

| Command | Does |
|---|---|
| `srl stats [SELECTOR] [--json]` | Atoms per state, reviews today, retention over the last 30 days (share of non-Again ratings on atoms that were not new). |
| `srl log [SELECTOR] [--json]` | Print the log events of matching atoms. |

## Sessions

A session is a prepared list of cards in `sessions/NAME.jsonl`, usually written
by an agent ("15 questions on N1 grammar using food vocabulary") or a script.

- `srl study --session NAME` shows the cards in file order, each once,
  regardless of whether their atoms are due.
- Reviews are recorded normally and update the atoms' schedules.
- A session can be studied again; it is just a file.

> **TODO:** Revisit sessions once they are implemented. Build real scripts
> around `srl` (for example an agent preparing a session), look at how
> sessions work in practice, and decide whether they belong in the core or
> are better handled outside `srl`. Refactor this specification based on
> that.

## The study loop, scripted

`srl study` is a convenience. The same loop can be written with the primitives:

```sh
sel='tag:jlpt/n1,kanji/food'
while card=$(srl next "$sel" --json); do
  # show the card, collect a response, optionally:
  #   result=$(jq -n --argjson card "$card" --argjson r '["…"]' '{card: $card, response: $r}' | srl grade)
  # then record the rating:
  jq -n --argjson card "$card" --argjson rating 3 '{card: $card, rating: $rating}' | srl review --stdin
done
```
