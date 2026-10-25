# The command line

The CLI is the only interface to a store. People, scripts, frontends and
agents all use the same commands. There is no separate API.

Every command reads or changes the store's data: its notes and their
items, the log and the configuration. One of them, `srl next --json`, is
also the frontend contract: the mapping from that data to what a frontend
shows (see [frontends.md](frontends.md)). `srl cards --json` prints cards
in the same format, the one a frontend shows (see [cards.md](cards.md)),
without what `srl next --json` adds around them; frontends rely on
`srl next` only. No command shows cards
interactively or runs other programs for you, except `srl edit ID`, which
opens a note in your editor.

## Conventions

- **`--json` everywhere.** Every command that prints data can print JSON
  instead (JSONL for lists). The default output is for people.
- **Never ask questions when standard input is not a terminal.** The one
  interactive command, `srl edit ID`, refuses in a pipe; every action has a
  non-interactive form. Studying is up to frontends (see
  [frontends.md](frontends.md)).
- **Standard output carries results only.** Errors and warnings go to standard
  error, as one plain message each, saying what to do about it.
- **Exit codes:** 0 success (or something found), 1 nothing found or nothing
  left, 2 error.
- **Errors in notes stop every command that reads items** (see
  [notes.md](notes.md), "Errors"), except `srl id` for items without an ID,
  `srl check`, which reports them, and `srl lint`.
- **`--dry-run`** on every command that writes: show what would change, change
  nothing.
- **`--dir DIR`** on every command, to choose the store (see
  [store.md](store.md)).
- **IDs** given to commands are card IDs or item IDs. An item ID stands for
  every card of the item, as in the selector `id:`.
- **Platform independence.** `srl` behaves the same on Linux, macOS and
  Windows. Where it can't, the document says so in a note marked
  **TODO (platform)** (see [overview.md](overview.md), "Open questions").
- **Only built-in commands.** Any other command name is a usage error.
  `srl` doesn't launch other programs: a frontend or tool runs under its
  own name (`srl-web`), and your shell's aliases and scripts give it any
  other (see [rationale.md](rationale.md), "Why a data layer and one
  mapping, and nothing else").

## Commands

### Store

| Command | Does |
|---|---|
| `srl init [DIR]` | Create a store in `DIR`, `--dir`, `SRL_DIR` or the current directory (see [store.md](store.md)). |
| `srl check` | Check the notes, the configuration and the log; report each problem on standard error and exit with 2 if there are any (see "Checking" below). |
| `srl lint` | Look for text that was probably meant to be an item but isn't one (see "Checking" below). |
| `srl id` | Give an ID to every item without one (see [notes.md](notes.md), "How srl writes to notes"). Prints `ID  PATH:LINE` per item; exit 1 if every item already had one. |
| `srl reindex` | Delete the index cache and build it again from the files (see [store.md](store.md), "The index cache"). Prints nothing. |

### Items and cards

| Command | Does |
|---|---|
| `srl find [SELECTOR] [--json]` | List matching cards, in the order they are written (by note path, line, then card ID): one line each with the card ID, `PATH:LINE` and the first line of the item's first field, separated by two spaces. With `--json`, one object per card (see below). Exit 1 if none matches. |
| `srl show ID… [--context MODE] [--json]` | Print the item of each ID, with its context (see below). Unknown IDs are reported, and make the exit code 2. |
| `srl cards ID… [--json]` | Print cards by ID, as a frontend shows them: a card ID gives that card, an item ID all of its cards (see below). |
| `srl add NOTE --stdin` | Append new items to `NOTE` (see "Adding items" below). Prints the new item IDs. |
| `srl edit ID` | Open the note of the item in the editor (see "Editing in the editor" below). |
| `srl edit ID --stdin` | Replace the item's lines in its note with standard input, which must be exactly one item with the same ID. The rest of the note is unchanged. Prints nothing; with `--dry-run`, only checks the input. |
| `srl drafts [SELECTOR] [--json]` | List drafts: the same as `srl find SELECTOR status:draft`. |
| `srl accept SELECTOR` | Remove `draft` from the headers of the items of matching draft cards. |
| `srl suspend SELECTOR` | Suspend matching active cards. |
| `srl unsuspend SELECTOR` | Make matching suspended cards active again. |
| `srl move FROM TO` | Give the history of the orphan `FROM` to the card `TO` (see [scheduling.md](scheduling.md), "Moving history"). Prints nothing; with `--dry-run`, the event it would write. |

`srl find --json` prints one object per card:

```json
{"id": "k3m9q1-2", "item": "k3m9q1", "path": "japanese/grammar/wo-mono-tomo-sezu.md",
 "line": 9, "status": "active", "state": "due", "due": "2026-10-24"}
```

`due` is the study day the card is due, or `null` for a new card. `srl show
--json` prints items as described in [notes.md](notes.md), "Items as JSON".
Without `--json`, `srl show` prints, for each item: a line `PATH:LINE`, the
item's lines as written, and, if there is a context, a line `---` and
the context. Items are separated by blank lines. `--context MODE` chooses
the context's mode (see [notes.md](notes.md), "Context"); it is `prose`
without it.

`srl cards` prints cards in the format of [cards.md](cards.md) with
`--json`, one per line. Without it, each card is printed for people: a line
with its ID, the prompt, a line `---`, the revealed prompt, and, if the
answer is not `null`, a line `---` and the answer. The revealed prompt is
printed even when it is the prompt again, because `prompt_revealed` is
`null` (see [cards.md](cards.md), "Format"). Cards are separated by blank
lines:

```
k3m9q1-1
==[...]==を==[...]==、彼女は起業した。
---
==周囲の反対==を==ものともせず==、彼女は起業した。
---
Despite everyone's opposition, she started a company.
```

The cards of a kind program are made when `srl cards` prints them (see
[cards.md](cards.md), "Kind programs"). If one can't be made, `srl cards`
fails at once with the reason, exit 2, and prints no card at all.

`srl accept`, `srl suspend` and `srl unsuspend` apply the selector exactly
as written, and change only matching cards that have the status they start
from: `srl suspend in:x` doesn't touch drafts. `srl accept` prints the item
IDs it changed, the others the card IDs, one per line in the order they are
written, and each exits with 1 if it changed nothing. `srl accept` changes
notes (see [notes.md](notes.md), "How srl writes to notes"); the others
only append to the log.

### Adding items

```sh
srl add NOTE --stdin
```

The items come from standard input, one per line, each a JSON object of
its header entries and its fields. Each item has its own, so items about
different notes, of different kinds, or drafts and not, can be added at
once.

```sh
srl add generated/japanese.md --stdin <<'JSONL'
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "Make a sentence with 〜をものともせず about a football team.", "A": "…"}
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "{{c1::嵐}}をものともせず、漁師たちは海へ出た。"}
{"see": "japanese/grammar/ni-mo-kakawarazu.md", "draft": true, "Q": "What does 〜にもかかわらず express?", "A": "Despite, in spite of."}
JSONL
```

- **The lowercase keys `kind`, `see` and `draft` are header entries** (see
  [notes.md](notes.md), "Header entries"): `kind` and `see` take text,
  `draft` takes `true` or `false`. Each may be left out: the item is then
  `qa`, has its own note as context, and is not a draft. There is no `id`:
  every item gets a new ID.
- **Every other key is a field name, and its value the field's text**, in
  the order given. Multi-line text is written with continuation lines.
  Which fields are accepted is up to the kind, so `srl add` works the same
  for every kind there is.
- `NOTE` is a path relative to the store's folder, and must be a note:
  matched by `include`, not by `exclude`, and not in a folder that starts
  with `.` (see [notes.md](notes.md), "Which files are notes"). If it
  doesn't exist, it is created, with its folders.
- The items are appended to the end of the note, each after a blank line,
  following the rules of [notes.md](notes.md), "How srl writes to notes".
- Every line is checked before anything is written: if any line is not a
  valid item (not a JSON object, a key that is neither a header entry nor
  a field name, a value of the wrong type, an unknown kind, a `see` that
  doesn't name a note of the store, fields the kind doesn't accept),
  `srl add` reports the first such line and adds nothing. Blank lines are
  ignored.

It prints the new item IDs, one per line. With `--dry-run` it prints the
items it would write, as they would appear in the note, and writes nothing.

### Editing in the editor

`srl edit ID` opens the item's note in the editor:

- The editor is `VISUAL` if it is set and not empty, else `EDITOR`, else
  the platform's default, as in git: `notepad` on Windows, `vi` everywhere
  else.
- The value is split at spaces into a program and its arguments
  (`code --wait`). A value that starts with `"` names the program up to the
  next `"`, so a path with spaces can have arguments too
  (`"C:\Program Files\Microsoft VS Code\bin\code.cmd" --wait`). There is no
  other quoting. The program is found on `PATH`, or given as a path. The
  note's file comes last.
- The editor works on the note itself. A note is yours, so `srl` doesn't
  check it when the editor exits; the next command that reads items reports
  any error in it.

> **TODO (design):** `srl edit` opens the note at its start. Editors take a
> line number in different ways (`+LINE`, `FILE:LINE`, `--goto`), and how
> to say which one yours uses is undecided (see
> [overview.md](overview.md), "Open questions").

> **TODO (platform):** not run on Windows yet. Notepad, the default there,
> is the one editor every Windows has, but Windows 11's Notepad may open the
> file in a tab of a Notepad window that is already open and return at
> once. That is harmless, since `srl` doesn't check the note afterwards.

### Studying

| Command | Does |
|---|---|
| `srl next [SELECTOR] [--context MODE] [--json]` | Give a frontend the next card to show (see [scheduling.md](scheduling.md)): with `--json`, the card and what a frontend shows around it (see below); without, the card for people, in the format of `srl cards`. Exit 1 if there is none; exit 2 if the card it picks can't be made. |
| `srl review --stdin` | Record a review: `{"card": "k3m9q1-2", "rating": N, "response": "…", "by": "human"}`. `response` and `by` may be left out (`by` is then `human`); no other key is allowed. Prints nothing. |
| `srl review ID… --rating N [--by agent]` | Record the same rating for each card. Prints nothing. |
| `srl reset SELECTOR` | Make matching cards new again. Only cards that are not new get a `reset` event. Prints their IDs; exit 1 if there were none. |

The rating is always given by whoever records the review (see
[scheduling.md](scheduling.md), "Ratings"); `srl` never judges an answer.
`response` is optional text: what was answered, if whoever records the
review wants to keep it, such as an answer typed to an agent (see
[log.md](log.md)). `by` says who recorded the review, not how the rating
was decided.

`srl next --json` is the frontend contract: what it prints, one object
with the card, its state, and its item's kind, note and context, is
specified in [frontends.md](frontends.md), "What a frontend gets". `--context MODE` chooses the context's mode,
`prose` without it. Only the JSON output has a context, so without
`--json` the flag changes nothing.

`srl next` picks one card and makes it. If the card can't be made, because
its kind program fails (see [cards.md](cards.md), "Kind programs") or its
note can't be read, `srl next` fails with the reason and exit 2, prints no
card, and tries no other card in its place. It writes nothing, so it fails
the same way every time until what failed is fixed, the card is suspended
(`srl suspend id:ID`), or the study day changes and another card is picked.

> **TODO (design):** being this strict can get in the way of studying,
> as when an LLM API is down. Ways to make it less annoying without ever
> working with broken data are wanted, and still undecided (see
> [overview.md](overview.md), "Open questions").

Every card given to `srl review` must be a card of the store; an item ID
is not enough, because a review is about one card. With `--dry-run`,
`srl review` prints the events it would write, one JSON line each, and
`srl reset` prints the IDs it would reset; neither writes to the log.

### Analysis

| Command | Does |
|---|---|
| `srl stats [SELECTOR] [--json]` | Cards per state, reviews today, retention over the last 30 days (share of non-Again ratings on cards that were not new), for the matching cards (see below). Exit 1 if none matches. |
| `srl log [SELECTOR] [--json]` | Print the log events of matching cards, in log order (see below); with `--json`, the events as stored. Exit 1 if there are none. |

`srl stats` applies the selector as written, so it counts drafts and
suspended cards too unless the selector says otherwise. It prints one
`NAME  VALUE` line each:

```
cards  120
new  30
due  12
young  41
mature  37
leech  2
reviews today  25
retention  88.5% of 400 reviews in 30 days
```

- `new`, `due`, `young` and `mature` add up to `cards`. `leech` overlaps
  them (see [scheduling.md](scheduling.md), "States").
- `reviews today` counts the reviews of the matching cards on today's study
  day.
- `retention` looks at the last 30 study days, today included. It counts
  the reviews of matching cards that were not new at the time, and gives
  the share that were not Again. Without such reviews it is `-`.

With `--json` it prints one object:
`{"cards": N, "states": {"new": N, "due": N, "young": N, "mature": N,
"leech": N}, "reviews_today": N, "retention": R, "retention_reviews": N}`,
where `retention` is a number from 0 to 1, or `null`.

`srl log` prints one line per event, its fields separated by two spaces:
the time, the card, and then `review`, the rating and who recorded it;
`reset`; `suspend`; `unsuspend`; or `move` and the card the history came
from. A `move` event belongs to the card it moved history to.

## Checking

`srl check` reads every note, the configuration and the log, and reports:

- every error in notes (see [notes.md](notes.md), "Errors");
- an invalid configuration;
- log lines that are not valid events;
- orphans, with the `srl move` command that would keep their history when
  there is an obvious one (see [scheduling.md](scheduling.md), "Moving
  history").

Each problem is reported on standard error, one per line. Orphans are
notices: the exit code is 2 if there is any other problem, and 0 otherwise.

The configuration is read first, because the notes and the log can't be
read without it (it says which notes to exclude, and the study day). So an
invalid configuration is reported alone, and `srl check` stops there; run
it again once the configuration is fixed to see the other problems.

`srl check` checks only what `srl` needs. Whether other programs can open
the notes, for example paths too long for some programs on Windows, is not
its concern.

`srl lint` looks for mistakes `srl check` can't call errors, because only
headers make items (see [notes.md](notes.md), "The item syntax"). It
reports, as `PATH:LINE: message` on standard output:

- lines outside items that look like fields of the `qa` kind: `Q:` or `A:`
  at the start of a line, or after spaces, a list marker (`-`, `*`, `+`,
  `1.`) or a quote marker (`>`);
- header-like text that isn't at column 0, such as an indented
  `<!-- srl -->`;
- cloze deletions (`{{c1::…}}`) outside items.

These are guesses, so `srl lint` never fails because of what it finds: it
exits with 0 if it found something, and 1 if not.
