# The command line

The CLI is the only interface to a store. People, scripts, frontends and
agents all use the same commands. There is no separate API.

## Conventions

- **`--json` everywhere.** Every command that prints data can print JSON
  instead (JSONL for lists). The default output is for people.
- **Never ask questions when standard input is not a terminal.** Interactive
  commands (`srs study`, `srs new`, `srs edit ID`) refuse in a pipe; every
  action has a non-interactive form.
- **Standard output carries results only.** Errors and warnings go to standard
  error, as one plain message each, saying what to do about it.
- **Exit codes:** 0 success (or something found), 1 nothing found or nothing
  left, 2 error.
- **`--dry-run`** on every command that writes: show what would change, change
  nothing.
- **`--dir DIR`** on every command, to choose the store (see
  [store.md](store.md)).
- **Extension like git.** `srs NAME` runs `srs-NAME` from `PATH` if `NAME` is
  not a built-in command. Frontends and other tools plug in this way.

## Commands

### Store

| Command | Does |
|---|---|
| `srs init [DIR]` | Create a store. |
| `srs check [--fix]` | Validate atoms, drills and cards. `--fix` assigns missing IDs. |
| `srs reindex` | Rebuild the index cache. |

### Atoms

| Command | Does |
|---|---|
| `srs new [--draft]` | Write a new atom in `$EDITOR`. |
| `srs add [--draft] --cue C [--answer A] [--tag T]… [--source S]` | Add an atom from flags. |
| `srs add [--draft] FILE…` | Add atoms from Markdown files. |
| `srs add [--draft] --stdin` | Add atoms from JSONL (see [atoms.md](atoms.md)). Prints the new IDs. |
| `srs edit ID` | Edit an atom in `$EDITOR`. |
| `srs edit ID --stdin` | Replace an atom's file with standard input. The `id` must not change. |
| `srs show ID… [--json]` | Print atoms. |
| `srs find [SELECTOR] [--json]` | List matching atoms. |
| `srs tags [--json]` | List every tag in use, with counts. |
| `srs inbox [SELECTOR] [--json]` | List drafts. |
| `srs accept SELECTOR` | Make matching drafts active. |
| `srs suspend SELECTOR` / `srs unsuspend SELECTOR` | Change status between `active` and `suspended`. |

### Studying

| Command | Does |
|---|---|
| `srs study [SELECTOR]` | The built-in terminal frontend (see [frontends.md](frontends.md)). |
| `srs next [SELECTOR] [--drill NAME] [--json]` | Print the next card. Exit 1 if there is none. |
| `srs cards ID [--json]` | Print every card of an atom, from every applicable drill. |
| `srs grade` | Grade an answer: card and response as JSON on stdin (see [grading.md](grading.md)). |
| `srs review --stdin` | Record a review: `{"card": …, "rating": N, "response": […], "by": "human"}`. |
| `srs review ID… --rating N [--by agent]` | Record a review without a card. |
| `srs play NAME` | Study a prepared session (see below). |
| `srs reset SELECTOR` | Make matching atoms new again. |

### Caches and sessions

| Command | Does |
|---|---|
| `srs generated put ID` | Replace the cached cards of an atom with JSONL from stdin. Cards are validated. |
| `srs generated clear [SELECTOR]` | Delete cached cards. |
| `srs session put NAME` | Save a list of cards (JSONL from stdin) as a session. Cards are validated. |
| `srs session list` / `srs session rm NAME` | Manage sessions. |

### Drills

| Command | Does |
|---|---|
| `srs drills [--json]` | List available drills and where they were found. |

### Analysis

| Command | Does |
|---|---|
| `srs stats [SELECTOR] [--json]` | Atoms per state, reviews today, retention over the last 30 days (share of non-Again ratings on atoms that were not new). |
| `srs log [SELECTOR] [--json]` | Print the log events of matching atoms. |

## Sessions

A session is a prepared list of cards in `sessions/NAME.jsonl`, usually written
by an agent ("15 questions on N1 grammar using food vocabulary") or a script.

- `srs play NAME` shows the cards in file order, each once, regardless of
  whether their atoms are due.
- Reviews are recorded normally and update the atoms' schedules.
- A session can be played again; it is just a file.

## The study loop, scripted

`srs study` is a convenience. The same loop can be written with the primitives:

```sh
sel='tag:jlpt/n1,kanji/food'
while card=$(srs next "$sel" --json); do
  # show the card, collect a response, optionally:
  #   result=$(jq -n --argjson card "$card" --argjson r '["…"]' '{card: $card, response: $r}' | srs grade)
  # then record the rating:
  jq -n --argjson card "$card" --argjson rating 3 '{card: $card, rating: $rating}' | srs review --stdin
done
```
