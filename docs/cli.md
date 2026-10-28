# The command line

The CLI is the only interface to a store. People, scripts, frontends and
agents all use the same commands. There is no separate API.

Every command reads or changes the store's data: its notes and their
items, the log and the configuration. One of them, `srl next --json`, is
also the frontend contract: the mapping from that data to what a frontend
shows (see [frontends.md](frontends.md)). `srl cards --json` prints cards
in the same format, the one a frontend requires (see [cards.md](cards.md)),
without what `srl next --json` adds around them; frontends rely on
`srl next` only. No command shows cards
interactively or runs other programs for you, except the kind programs of
`[kinds]`, which commands that read items run for the items whose answers
aren't kept (see [cards.md](cards.md), "Kind programs").

## Conventions

- **`--json` everywhere.** Every command that prints data can print JSON
  instead (JSONL for lists). The default output is for people.
- **Never interactive.** No command asks questions or waits for a person.
  Studying is up to frontends (see [frontends.md](frontends.md)), and
  editing notes is up to your editor.
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
- **The time and the random numbers can be fixed**, for tests and
  scripts. `SRL_NOW`, a time such as `2026-01-05T09:00:00Z`, is the
  current time for every command: the time that reviews are recorded
  with, and the time today's study day is worked out from. The time zone
  is still the system's, or `TZ`. `SRL_SEED`, a whole number from 0 to
  18446744073709551615, replaces the random numbers that new IDs are made
  from and that `srl next` uses to pick a card, so the same store gives
  the same results. An empty variable counts as unset. A value that can't
  be read is an error (exit code 2).
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
| `srl lint` | Look for likely mistakes: text probably meant to be an item, and items probably not written as meant (see "Checking" below). |
| `srl id` | Give an ID to every item without one (see [notes.md](notes.md), "How srl writes to notes"). Prints `ID  PATH:LINE` per item; exit 1 if every item already had one. |
| `srl notes [--json]` | List the notes of the store (see [notes.md](notes.md), "Which files are notes"): one path per line, relative to the store's folder, sorted as `srl find` sorts notes. With `--json`, one object per note: `{"path": "japanese/grammar/wo.md"}`. It doesn't read the notes, so errors in them don't stop it. Exit 1 if the store has none. |
| `srl kinds [--json]` | Describe the kinds of the store: `qa`, then the kind programs of `[kinds]` sorted by name (see [cards.md](cards.md), "Describing a kind"). For each, a line with its name and description, then a line per field, indented by two spaces, with its name and description; names and descriptions are separated by two spaces. With `--json`, one object per kind: `{"name": "qa", "description": "…", "fields": [{"name": "Q", "description": "…"}, …]}`. It doesn't read the notes. If a kind program can't describe its kind, it fails at once with the reason, and prints nothing. |

### Items and cards

| Command | Does |
|---|---|
| `srl find [SELECTOR] [--json]` | List matching cards, in the order they are written (by note path, line, then card ID): one line each with the card ID, `PATH:LINE` and the first line of the item's first field, separated by two spaces. With `--json`, one object per card (see below). Exit 1 if none matches. |
| `srl show ID… [--context MODE] [--json]` | Print the item of each ID, with its context (see below). Unknown IDs are reported, and make the exit code 2. |
| `srl cards ID… [--json]` | Print cards by ID, as a frontend requires them: a card ID gives that card, an item ID all of its cards (see below). |
| `srl add NOTE --stdin` | Append new items to `NOTE` (see "Adding items" below). Prints the new item IDs. |
| `srl edit --stdin` | Replace items in their notes with the item objects of standard input, one per line, each naming the item it replaces by its `id` (see "Editing items" below). The rest of each note is unchanged. Prints the IDs of the items. |
| `srl accept SELECTOR` | Remove `draft` from the items of matching draft cards. |
| `srl suspend SELECTOR` | Suspend matching active cards. |
| `srl unsuspend SELECTOR` | Make matching suspended cards active again. |
| `srl move FROM TO` | Give the history of the orphan `FROM` to the card `TO` (see [scheduling.md](scheduling.md), "Moving history"). Prints nothing; with `--dry-run`, the event it would write. |

`srl find --json` prints one object per card:

```json
{"id": "k3m9q1-2", "item": "k3m9q1", "path": "japanese/grammar/wo-mono-tomo-sezu.md",
 "line": 10, "status": "active", "state": "due", "due": "2026-10-24"}
```

`due` is the study day the card is due, or `null` for a new card. `srl show
--json` prints one item object per item, with every entry and the keys
only `srl` gives (see [notes.md](notes.md), "The item object"). Without
`--json`, `srl show` prints, for each item: a line `PATH:LINE`, the item as
written in its syntax, and, if there is a context, a line `---` and the
context. Items are separated by blank lines. `--context MODE` chooses the
context's mode (see [notes.md](notes.md), "Context"); it is `prose`
without it.

**An item as written** is, in a JSONL note, its line. In srl blocks, it is
the item as an srl block of its own: an opening fence made of the same
backticks or tildes as its block's, `srl`, and the item's entries as
written (after a space, when it has some); the item's field lines as
written; and a closing fence like the opening one.

### Editing items

```sh
srl show x7k2m9 --json > item.jsonl
$EDITOR item.jsonl
srl edit --stdin < item.jsonl
```

Standard input holds one item object per line (see [notes.md](notes.md),
"The item object"), each with the `id` of the item it replaces, which can
be in any note and either syntax. Blank lines are ignored. What `srl show
--json` prints is such input: its keys `path`, `line`, `cards` and
`context` are ignored. Each object is the whole new item, not a change to
the old one: an entry left out takes its default, and a field left out is
gone.

- **Every line is checked before anything is written**: if any line is not
  a valid item object, names no item, names an item another line names
  too, or makes an item that is not valid (a `see` that doesn't name a
  note of the store, an unknown kind, fields the kind doesn't accept),
  `srl edit` reports the first such line and changes nothing. The lines
  are checked together, so each kind program is asked once about all of
  them (see [cards.md](cards.md), "How srl calls them"); a kind program
  that fails as a whole is reported as itself, not as a line.
- **Each item is written in the syntax of its note** (see
  [notes.md](notes.md), "How srl writes to notes"):
  - In a JSONL note, its line is written again.
  - In srl blocks, an item that has its block to itself is replaced, block
    and all, by a block of its own, as `srl add` writes it. An item that
    starts with an `@` line keeps its place in its block: its lines are
    replaced by an `@` line with the new entries (`@` alone when there
    are none) and the new field lines. A field line that would close the
    block is an error; open and close the block with a longer fence.

It prints the IDs of the items, one per line, in the order given, and
exits with 1 if standard input held none. With `--dry-run` it prints the
same and writes nothing.

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

The cards of a kind program are made when `srl cards` prints them, all
the cards of a kind program in one call (see [cards.md](cards.md), "How
srl calls them"). If one can't be made, `srl cards` fails at once with
the reason, exit 2, and prints no card at all.

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

The items come from standard input, one item object per line (see
[notes.md](notes.md), "The item object"): its header entries and its
fields. Each item has its own, so items about different notes, of
different kinds, or drafts and not, can be added at once.

```sh
srl add generated/japanese.md --stdin <<'JSONL'
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "Make a sentence with 〜をものともせず about a football team.", "A": "…"}
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "{{c1::嵐}}をものともせず、漁師たちは海へ出た。"}
{"see": "japanese/grammar/ni-mo-kakawarazu.md", "draft": true, "Q": "What does 〜にもかかわらず express?", "A": "Despite, in spite of."}
JSONL
```

- **The lowercase keys `id`, `kind`, `see` and `draft` are header
  entries** (see [notes.md](notes.md), "Header entries"). Each may be left
  out: the item then gets a new ID, is `qa`, has its own note as context
  (none in a JSONL note), and is not a draft. An `id` given is kept, and
  must not be used by an item of the store or another line.
- **Uppercase keys are field names, and their values the fields' text**,
  in the order given. Which fields are accepted is up to the kind, so `srl
  add` works the same for every kind there is; `srl kinds` describes them.
- `NOTE` is a path relative to the store's folder, and must be a note:
  matched by `include`, not by `exclude`, and not in a folder that starts
  with `.` (see [notes.md](notes.md), "Which files are notes"). If it
  doesn't exist, it is created, with its folders.
- The items are appended to the end of the note, in the note's syntax,
  following the rules of [notes.md](notes.md), "How srl writes to notes":
  - In srl blocks, each item is in a block of its own with its entries on
    the opening fence, after a blank line. Multi-line text is written with
    continuation lines. The fence is three backticks, or one more than the
    longest run of backticks that a line of the item's text starts with,
    so that a code block in a field doesn't close it.
  - In a JSONL note, each item is a line, written as `srl` writes an item
    object (see [notes.md](notes.md), "How srl writes to notes").
- Every line is checked before anything is written: if any line is not a
  valid item (not a valid item object, an ID already used, an unknown
  kind, a `see` that doesn't name a note of the store, fields the kind
  doesn't accept), `srl add` reports the first such line and adds nothing.
  Blank lines are ignored. The lines are checked together, so each kind
  program is asked once about all of its items (see [cards.md](cards.md),
  "How srl calls them"); a kind program that fails as a whole is reported
  as itself, not as a line.

It prints the new item IDs, one per line. With `--dry-run` it prints the
items it would write, as they would appear in the note, and writes nothing.

### Studying

| Command | Does |
|---|---|
| `srl next [SELECTOR] [--context MODE] [--json]` | Give a frontend the next card to show (see [scheduling.md](scheduling.md)): with `--json`, the card and what a frontend requires around it (see below); without, the card for people, in the format of `srl cards`. Exit 1 if there is none; exit 2 if the card it picks can't be made. |
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
the same way every time it picks that card, until what failed is fixed, the
card is suspended (`srl suspend id:ID`), or the study day changes and
another card is picked. With `max_skip` above 0 (see
[scheduling.md](scheduling.md), "Choosing the next card"), another call
may pick another card, but the broken one stays among the first and keeps
coming back.

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

`srl check` asks kind programs about every item, rather than using the
answers kept in the index cache, and keeps their new answers (see
[cards.md](cards.md), "How srl calls them").

The configuration is read first, because the notes and the log can't be
read without it (it says which notes to exclude, and the study day). So an
invalid configuration is reported alone, and `srl check` stops there; run
it again once the configuration is fixed to see the other problems.

`srl check` checks only what `srl` needs. Whether other programs can open
the notes, for example paths too long for some programs on Windows, is not
its concern.

`srl lint` looks for mistakes `srl check` can't call errors, because only
srl blocks make items (see [notes.md](notes.md), "The srl block syntax"), and
because an item can be valid and still not be what you meant. It reports,
as `PATH:LINE: message` on standard output, in the order of the notes and
their lines:

- outside srl blocks, lines that look like part of one. Each may come at
  the start of a line, or after spaces, a list marker (`-`, `*`, `+`,
  `1.`) or a quote marker (`>`):
  - an srl block's opening fence that isn't at column 0, such as an
    indented ```` ```srl ````;
  - a misspelled opening fence: a fence of three or more backticks or
    tildes followed, after any spaces, by `srl` or `slr` in any case, then
    by nothing, a space or a character other than a letter, a digit, `-`
    or `_`, such as ```` ```SRL ```` or ```` ```srl{id:x7k2m9} ````;
  - a field of the `qa` kind: `Q:` or `A:`, with or without the space
    after the colon, `q:`, or `Q` with a full-width colon or letter
    (`Q：`, `Ｑ:`);
  - a cloze deletion (`{{c1::…}}`);
- in the `Q` of a `qa` item, something that was probably meant to be a
  cloze deletion (`{{C1::…}}`, or `{{c1:…}}` with one colon), and a
  deletion whose hint is the text it hides;
- an srl block whose closing fence is indented (by one to three spaces): it
  is almost always the end of a code block in a field, which closed the
  srl block too early (see [notes.md](notes.md), "The srl block syntax", rule
  12), whatever the item's kind;
- a fenced code block that is never closed, when srl blocks come after it.

Outside srl blocks, a paragraph (a run of lines without a blank line) is
reported once, at its first suspicious line, since the lines after it are
usually the same mistake. The paragraph right after a block closed by an
indented fence is the rest of its item, so it is not reported. Other
lines inside an srl block are never reported: mistakes there are errors,
which `srl check` reports.

A JSONL note holds nothing but items and blank lines, and a line that is
neither is an error, so only the `Q` of its `qa` items is looked at, as
above. Each finding is reported at the item's line.

Only the item syntaxes and the `qa` kind are known to `srl lint`: it
doesn't look at the fields of items of kind programs, nor for their fields
outside items.

These are guesses, so `srl lint` never fails because of what it finds: it
exits with 0 if it found something, and 1 if not.
