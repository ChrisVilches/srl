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
| `srl check [--fix]` | Validate atoms, drills and cards; report each problem on standard error and exit with 2 if there are any. `--fix` assigns missing IDs and prints them (see [atoms.md](atoms.md), "Validation"). |
| `srl reindex` | Rebuild the index cache. |

### Atoms

| Command | Does |
|---|---|
| `srl new [--draft]` | Write a new atom in `$EDITOR`. |
| `srl add [--draft] --title TITLE [--answer ANSWER] [--tag TAG]… [--source SOURCE]` | Add an atom from flags. |
| `srl add [--draft] --stdin` | Add atoms from JSONL (see [atoms.md](atoms.md)). Prints the new IDs. |
| `srl edit ID` | Edit an atom in the editor (see "Editing in the editor" below). Prints nothing. |
| `srl edit ID --stdin` | Replace an atom's file with standard input, exactly as given. It must be a valid atom with the same `id`. The file keeps its path, even if the tags or title change. Prints nothing. `--dry-run` only checks the text. |
| `srl show ID… [--json]` | Print atoms: each file as it is, or JSON (see [atoms.md](atoms.md)). Unknown IDs are reported, and make the exit code 2. |
| `srl find [SELECTOR] [--json]` | List matching atoms, ordered by path: one line each with ID, title and tags, separated by two spaces; or the atom JSON of [atoms.md](atoms.md). Exit 1 if none matches. |
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

### Editing in the editor

`srl edit ID`, `srl new` and `e` in `srl study` open the editor:

- The editor is `VISUAL` if it is set and not empty, else `EDITOR`. If
  neither is set, the command fails and says to set `EDITOR`.
- The value is split at spaces into a program and its arguments
  (`code --wait`), with no quoting. The program is found on `PATH`, or given
  as a path. The file to edit comes last.
- The editor works on a temporary copy of the atom file. When it exits with
  status 0, the copy is checked like `srl edit ID --stdin` (a valid atom,
  the same `id`) and replaces the atom file. An unchanged copy changes
  nothing. An editor that exits with another status changes nothing.
- A copy that is not a valid atom changes nothing either, and is kept: the
  error says where, so the edit isn't lost.
- `--dry-run` only works with `--stdin`.

> **TODO (platform):** `EDITOR` and `VISUAL` are usually set on Unix but
> rarely on Windows, where `srl edit ID` then fails and says to set `EDITOR`.
> Decide whether to fall back to a platform default there (such as
> `notepad`). Values are split at spaces on every platform, so an editor
> whose path contains a space can't be given arguments.

### Studying

| Command | Does |
|---|---|
| `srl study [SELECTOR] [--drill NAME]` | The built-in terminal frontend (see [frontends.md](frontends.md)). Adds `status:active`, like `srl next`. |
| `srl study --session NAME` | Study a prepared session with the same frontend (see below). |
| `srl next [SELECTOR] [--drill NAME] [--json]` | Print the next card (see [scheduling.md](scheduling.md)), in the format of `srl cards`. Exit 1 if there is none. Failing drills are reported but don't change the exit code, so a study loop keeps going. |
| `srl cards ID [--json]` | Print every card of an atom, from every applicable drill (see below). Failing drills and invalid cards are reported, and make the exit code 2. |
| `srl grade` | Grade an answer: card and response as JSON on stdin; prints the result as JSON (see [grading.md](grading.md)). Exits 0 even when it falls back to self-grading. |
| `srl review --stdin` | Record a review: `{"card": …, "rating": N, "response": […], "by": "human"}`. `response` and `by` may be left out (`by` is then `human`); no other key is allowed. Prints nothing. |
| `srl review ID… --rating N [--by agent]` | Record a review without a card. Every ID must be an atom of the store. Prints nothing. |
| `srl reset SELECTOR` | Make matching atoms new again. Only atoms that are not new get a `reset` event. Prints their IDs; exit 1 if there were none. |

The card given to `srl grade` and `srl review --stdin` is checked like any
card (see [cards.md](cards.md), rule 4), and every atom it names must exist.
With `--dry-run`, `srl review` prints the events it would write, one JSON
line each, and `srl reset` prints the IDs it would reset; neither writes to
the log.

`srl cards ID` orders the cards by drill, in the fixed drill order of
[scheduling.md](scheduling.md) (`reveal`, `reverse`, `cloze`, `cards`,
`confusable`, then the others by name), and keeps each drill's own order.
With `--json` it prints one card per line in the format of
[cards.md](cards.md). Without it, each card is printed for people: a line
with its `ref` and grading mode separated by two spaces, the prompt, the
choices as a numbered list (`choice` cards only), a line `---`, and the
answer. Cards are separated by blank lines:

```
reveal  self
鮪
---
まぐろ — tuna

cloze:0:1  exact
嵐を==[1]==、漁師たちは海へ出た。
---
嵐を==ものともせず==、漁師たちは海へ出た。
```

### Sessions

| Command | Does |
|---|---|
| `srl session put NAME` | Save a list of cards (JSONL from stdin) as a session. Cards are validated. |
| `srl session list` / `srl session rm NAME` | Manage sessions. |

### Drills

| Command | Does |
|---|---|
| `srl drills [--json]` | List available drills, where they were found, and the first line of their help (see [drills.md](drills.md), "Finding drills"). |

### Analysis

| Command | Does |
|---|---|
| `srl stats [SELECTOR] [--json]` | Atoms per state, reviews today, retention over the last 30 days (share of non-Again ratings on atoms that were not new), for the matching atoms (see below). Exit 1 if none matches. |
| `srl log [SELECTOR] [--json]` | Print the log events of matching atoms, in log order: one line each with the time, the atom, and `reset`, or `review`, the rating, the card's ref (`-` without a card) and who reviewed, separated by two spaces; or with `--json`, the events as stored. Exit 1 if there are none. |

`srl stats` applies the selector as written, so it counts drafts and
suspended atoms too unless the selector says otherwise. It prints one
`NAME  VALUE` line each:

```
atoms  120
new  30
due  12
young  41
mature  37
leech  2
reviews today  25
retention  88.5% of 400 reviews in 30 days
```

- `new`, `due`, `young` and `mature` add up to `atoms`. `leech` overlaps
  them (see [scheduling.md](scheduling.md), "States").
- `reviews today` counts the reviews of the matching atoms on today's study
  day.
- `retention` looks at the last 30 study days, today included. It counts
  the reviews of matching atoms that were not new at the time, and gives
  the share that were not Again. Without such reviews it is `-`.

With `--json` it prints one object:
`{"atoms": N, "states": {"new": N, "due": N, "young": N, "mature": N,
"leech": N}, "reviews_today": N, "retention": R, "retention_reviews": N}`,
where `retention` is a number from 0 to 1, or `null`.

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
