# The command line

The CLI is the only interface to a store. People, scripts, frontends and
every other program use the same commands. There is no separate API.

Every command reads or changes the store's data: the items of its
sources, the log and the configuration. One of them, `srl next --json`, is
also the frontend contract: the mapping from that data to what a frontend
shows (see [frontends.md](frontends.md)). `srl cards --json` prints cards
in the same format, the one a frontend requires (see [cards.md](cards.md)),
without what `srl next --json` adds around them; frontends rely on
`srl next` only. No command shows cards
interactively or runs other programs for you, except the kind programs of
`[kinds]`, which commands that read items run for the items whose answers
aren't kept (see [cards.md](cards.md), "Kind programs").

## Conventions

- **`--json` for data.** Every command that reads data out of the store
  (`find`, `show`, `cards`, `collections`, `notes`, `sources`, `kinds`,
  `next`, `stats`, `log`) can print JSON instead (JSONL for lists). The
  default output is for people. Commands that change the store print the
  IDs they changed, one per line (`srl id` adds the collection); they have
  no `--json`.
- **Never interactive.** No command asks questions or waits for a person.
  Studying is up to frontends (see [frontends.md](frontends.md)), and
  writing items by hand is up to your editor, or to whatever keeps the
  items of a source.
- **Standard output carries results only.** Errors and warnings go to standard
  error, as one plain message each, saying what to do about it.
- **Exit codes:** 0 success (or something found), 1 nothing found or nothing
  left, 2 error. Stopped by a signal, `srl` first stops the programs it
  runs, then ends by that signal, with no exit code (see
  [cards.md](cards.md), "Stopping a program").
- **Errors in sources stop every command that reads items** (see
  [items.md](items.md), "Errors"), except `srl id` for items without an ID,
  and `srl check`, which reports them.
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
| `srl check` | Check the sources, the configuration and the log, and look for likely mistakes in the items; report each on standard error and exit with 2 if there are problems (see "Checking" below). |
| `srl id` | Give an ID to every item without one (see [items.md](items.md), "How srl changes items"). Prints `ID  COLLECTION` per item; exit 1 if every item already had one. |
| `srl collections [--json]` | List the collections of the store, of every source (see [sources.md](sources.md)): one collection per line, in the order of their names, as `srl find` sorts them. With `--json`, one object per collection, with its name and its source: `{"collection": "japanese/grammar.jsonl", "source": "items"}`. It doesn't read any item, so errors in them don't stop it. Exit 1 if the store has none. |
| `srl notes [--json]` | List the notes of the store: the files `[context]` allows, which items may name with `see` (see [items.md](items.md), "Context"). One path per line, relative to the store's folder, in the order of their paths in NFC. With `--json`, one object per note: `{"path": "japanese/grammar/wo.md"}`. It reads no item and no note. Exit 1 if the store has none. |
| `srl sources [--json]` | Describe the sources of the store, in the order of their names (see [sources.md](sources.md)): for each, a line with its name and description, then lines, indented by two spaces, for its driver (`filesystem`, or `program` for a source program), the `include` globs and the `exclude` globs if it has some, each name and value separated by two spaces and globs by one. With `--json`, one object per source: `{"name": "items", "driver": "filesystem", "description": "…", "creates": {"include": ["**/*.jsonl", "**/*.xml"], "exclude": []}}`. The globs, in `creates` with `--json`, are those of the collections `srl add` may create in it: for `filesystem`, its `include` and `exclude`; for a source program, the globs its `describe` gives, and no `exclude`. It doesn't read any item. |
| `srl kinds [--json]` | Describe the kinds of the store: `qa`, then the kind programs of `[kinds]` sorted by name (see [cards.md](cards.md), "Describing a kind"). For each, a line with its name and description, then a line per field, indented by two spaces, with its name, its label in parentheses if it has one, and its description: `  Q (Question)  The question. …`; names and descriptions are separated by two spaces. With `--json`, one object per kind: `{"name": "qa", "description": "…", "fields": [{"name": "Q", "label": "Question", "description": "…"}, …]}`, a field's `label` being `null` when its kind gives none. It doesn't read any item. If a kind program can't describe its kind, it fails at once with the reason, and prints nothing. |

### Items and cards

| Command | Does |
|---|---|
| `srl find [SELECTOR] [--json]` | List matching cards, in the order they are written (by collection, the order of items in the collection, then card ID): one line each with the card ID, the collection and the first line of the item's first field, separated by two spaces. With `--json`, one object per card (see below). Exit 1 if none matches. |
| `srl show ID… [--no-context] [--json]` | Print the item of each ID, with its context (see below). Unknown IDs are reported, and make the exit code 2. |
| `srl cards ID… [--json]` | Print cards by ID, as a frontend requires them: a card ID gives that card, an item ID all of its cards (see below). |
| `srl add COLLECTION --stdin` | Add new items to the collection `COLLECTION`, such as a file's path (see "Adding items" below). Prints the new item IDs. |
| `srl edit --stdin` | Replace items, in any source, with the item objects of standard input, one per line, each naming the item it replaces by its `id` (see "Editing items" below). Nothing else of their collections changes. Prints the IDs of the items. |
| `srl accept SELECTOR` | Remove `draft` from the items of matching draft cards. |
| `srl suspend SELECTOR [--reason TEXT]` | Suspend matching active cards, saying why if `--reason` is given (see below). |
| `srl unsuspend SELECTOR` | Make matching suspended cards active again. |
| `srl move FROM TO` | Give the history of the orphan `FROM` to the card `TO` (see [scheduling.md](scheduling.md), "Moving history"). Prints nothing; with `--dry-run`, the event it would write. |

`srl find --json` prints one object per card:

```json
{"id": "k3m9q1.2", "item": "k3m9q1", "collection": "japanese/grammar.jsonl",
 "source": "items", "status": "active", "schedule": "due", "leech": false,
 "due": "2026-10-24", "suspend_reason": null}
```

`status` says whether the card is studied (see [items.md](items.md),
"Status"), `schedule` where it is in its schedule, and `leech` whether it
keeps being forgotten (see [scheduling.md](scheduling.md), "Schedule" and
"Leeches"). `due` is the study day the card is due, or `null` for a new card.
`suspend_reason` is why the card is suspended, the `reason` of its last
`suspend` event (see [log.md](log.md)), or `null` if the card is not
suspended or was suspended without one. `srl show
--json` prints one item object per item, with every entry and the keys
only `srl` gives (see [items.md](items.md), "The item object").
`--no-context` leaves the context out, without reading the note (see
[items.md](items.md), "Context").

**Without `--json`**, `srl show` prints the same keys, in the same
order, as prose: one line `KEY: value` each, the same for every item
whatever its source and format. `null` is printed as nothing after the
colon, `true` and `false` as words, and `cards` as its IDs separated by
`, `. A value of several lines, such as a field or the context, is
printed as `KEY:` on its own line, then each of its lines indented by two
spaces. Items are separated by blank lines:

```
id: p4n8d2
kind: qa
see: japanese/grammar/wo-mono-tomo-sezu.md
draft: false
Q:
  嵐を{{c1::ものともせず}}、
  漁師たちは海へ出た。
A: The fishermen went out to sea, braving the storm.
collection: japanese/grammar.jsonl
source: items
cards: p4n8d2.1
context:
  # 〜をものともせず

  Undeterred by (adversity), braving. …
```

It is for people to read, not for programs to parse: `--json` is the
form to read or change items with.

### Editing items

```sh
srl show x7k2m9 --json > item.jsonl
$EDITOR item.jsonl
srl edit --stdin < item.jsonl
```

Standard input holds one item object per line (see [items.md](items.md),
"The item object"), each with the `id` of the item it replaces, which can
be in any collection, of any source. Blank lines are ignored. What `srl show
--json` prints is such input: its keys `collection`, `source`, `cards` and
`context` are ignored. Each object is the whole new item, not a
change to the old one: an entry left out takes its default, and a field
left out is
gone.

- **Every line is checked before anything is written**: if any line is not
  a valid item object, names no item, names an item another line names
  too, or makes an item that is not valid (a `see` that doesn't name a
  note, an unknown kind, fields the kind doesn't accept),
  `srl edit` reports the first such line and changes nothing. The lines
  are checked together, so each kind program is asked once about all of
  them (see [cards.md](cards.md), "How srl calls them"); a kind program
  that fails as a whole is reported as itself, not as a line.
- **Each item is written by its source** (see [items.md](items.md), "How
  srl changes items"): a source program its own way (see
  [sources.md](sources.md), "Calls"), and the `filesystem` driver in the
  format of the item's collection, keeping the rest of the file as it
  can (see [formats.md](formats.md), "How srl writes to collections"). An
  item given unchanged is left as it is.

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
k3m9q1.1
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
from: `srl suspend in:x` doesn't touch drafts. One exception: `srl
suspend --reason TEXT` suspends matching cards that are already suspended
again too, so that the new reason replaces theirs; every card it changes
gets the same reason. An empty `TEXT` is no reason. `srl accept` prints the item
IDs it changed, the others the card IDs, one per line in the order they are
written, and each exits with 1 if it changed nothing. `srl accept` changes
items (see [items.md](items.md), "How srl changes items"); the others
only append to the log.

### Adding items

```sh
srl add COLLECTION --stdin
```

The items come from standard input, one item object per line (see
[items.md](items.md), "The item object"): its header entries and its
fields. Each item has its own, so items about different collections, of
different kinds, or drafts and not, can be added at once.

```sh
srl add generated/japanese.jsonl --stdin <<'JSONL'
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "Make a sentence with 〜をものともせず about a football team.", "A": "…"}
{"see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": true, "Q": "{{c1::嵐}}をものともせず、漁師たちは海へ出た。"}
{"see": "japanese/grammar/ni-mo-kakawarazu.md", "draft": true, "Q": "What does 〜にもかかわらず express?", "A": "Despite, in spite of."}
JSONL
```

- **The lowercase keys `id`, `kind`, `see` and `draft` are header
  entries** (see [items.md](items.md), "Header entries"). Each may be left
  out: the item then gets a new ID, is `qa`, has no context, and is not a
  draft. An `id` given is kept, and must not be used by an item of the
  store or another line. A `see` names a note, never the collection the
  items are added to (see [items.md](items.md), "Context").
- **Uppercase keys are field names, and their values the fields' text**,
  in the order given. Which fields are accepted is up to the kind, so `srl
  add` works the same for every kind there is; `srl kinds` describes them.
- `COLLECTION` is the name of a collection (see [sources.md](sources.md),
  "Collections"): for the `filesystem` source, its path relative to the
  store's folder, such as `japanese/grammar.jsonl`. If no collection has
  that name, it is created in the one source that accepts it: for
  `filesystem`, a file whose path its `include` matches and its `exclude`
  doesn't, outside folders that start with `.`, in a format it knows by
  its extension (see [formats.md](formats.md), "Which files are
  collections"), created with its folders. No source accepting it, or
  more than one, is an error, which lists every source with its reason,
  or the sources that accept it (see [sources.md](sources.md),
  "Collections"). A note is no collection: `srl add` never
  adds to a note.
- A source program adds the items its own way (see
  [sources.md](sources.md), "Calls"). In a collection of the
  `filesystem` source, they are added in its format, following the rules
  of [formats.md](formats.md), "How srl writes to collections": a line
  each in JSONL, an element each before `</items>` in XML, and an element
  each at the end of the array in JSON.
- Every line is checked before anything is written: if any line is not a
  valid item (not a valid item object, an ID already used, an unknown
  kind, a `see` that doesn't name a note, fields the kind
  doesn't accept), `srl add` reports the first such line and adds nothing.
  Blank lines are ignored. The lines are checked together, so each kind
  program is asked once about all of its items (see [cards.md](cards.md),
  "How srl calls them"); a kind program that fails as a whole is reported
  as itself, not as a line.

It prints the new item IDs, one per line. With `--dry-run` it prints the
items as their source would add them, and writes nothing: in a
collection file, as they would be written in it; for a source program,
the preview it gives.

### Studying

| Command | Does |
|---|---|
| `srl next [SELECTOR] [--no-context] [--json]` | Give a frontend the next card to show (see [scheduling.md](scheduling.md)): with `--json`, the card and what a frontend requires around it (see below); without, the card for people, in the format of `srl cards`. Exit 1 if there is none; exit 2 if the card it picks can't be made. |
| `srl review --stdin` | Record a review: `{"card": "k3m9q1.2", "rating": N, "response": "…", "by": "human"}`. `response` and `by` may be left out (`by` is then `human`); no other key is allowed. Prints nothing. |
| `srl review ID… --rating N [--by agent]` | Record the same rating for each card. Prints nothing. |
| `srl reset SELECTOR` | Make matching cards new again. Only cards that are not new get a `reset` event. Prints their IDs; exit 1 if there were none. |

The rating is always given by whoever records the review (see
[scheduling.md](scheduling.md), "Ratings"); `srl` never judges an answer.
`response` is optional text: what was answered, if whoever records the
review wants to keep it, such as an answer typed into a frontend (see
[log.md](log.md)). `by` says who recorded the review, not how the rating
was decided.

`srl next --json` is the frontend contract: what it prints, one object
with the card, where it is in its schedule, whether it is a leech, and its
item's kind, collection and context, is
specified in [frontends.md](frontends.md), "What a frontend gets".
`--no-context` leaves the context out, without reading its note. Only the
JSON output has a context, so without `--json` the flag changes nothing.

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
> as when a web service a kind program needs is down. Ways to make it less annoying without ever
> working with broken data are wanted, and still undecided (see
> [overview.md](overview.md), "Open questions").

Every card given to `srl review` must be a card of the store; an item ID
is not enough, because a review is about one card. With `--dry-run`,
`srl review` prints the events it would write, one JSON line each, and
`srl reset` prints the IDs it would reset; neither writes to the log.

### Analysis

| Command | Does |
|---|---|
| `srl stats [SELECTOR] [--json]` | Cards per place in the schedule, leeches, reviews today with the new cards and Again ratings among them, retention over the last 30 days (share of non-Again ratings on cards that were not new), for the matching cards (see below). Exit 1 if none matches. |
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
new today  8
again today  3
retention  88.5% of 400 reviews in 30 days
```

- `new`, `due`, `young` and `mature` add up to `cards`. `leech` counts
  the leeches among them (see [scheduling.md](scheduling.md), "Schedule" and
  "Leeches").
- `reviews today` counts the reviews of the matching cards on today's study
  day. Among them, `new today` counts the first reviews of cards that were
  new (since their last reset), and `again today` those rated Again. They
  are the day's whole study, whoever recorded it: a frontend can show them
  as its counts, the same after a reload.
- `retention` looks at the last 30 study days, today included. It counts
  the reviews of matching cards that were not new at the time, and gives
  the share that were not Again. Without such reviews it is `-`.

With `--json` it prints one object:
`{"cards": N, "schedule": {"new": N, "due": N, "young": N, "mature": N},
"leech": N, "reviews_today": N, "new_today": N, "again_today": N,
"retention": R, "retention_reviews": N}`,
where `retention` is a number from 0 to 1, or `null`.

`srl log` prints one line per event, its fields separated by two spaces:
the time, the card, and then `review`, the rating and who recorded it;
`reset`; `suspend` and the first line of its reason, if it has one;
`unsuspend`; or `move` and the card the history came from. A `move` event
belongs to the card it moved history to.

## Checking

`srl check` reads every source, the configuration and the log, and reports:

- every error in the store's sources, such as an error in a collection
  or a source program that fails (see [items.md](items.md), "Errors");
- an invalid configuration;
- log lines that are not valid events;
- orphans, with the `srl move` command that would keep their history when
  there is an obvious one (see [scheduling.md](scheduling.md), "Moving
  history");
- likely mistakes in the items, as warnings (see below).

Each problem is reported on standard error, one per line. Orphans and
warnings are notices: the exit code is 2 if there is any other problem,
and 0 otherwise.

`srl check` asks kind programs about every item, rather than using the
answers kept in the index cache, and keeps their new answers (see
[cards.md](cards.md), "How srl calls them").

The configuration is read first, because the sources and the log can't be
read without it (it names the sources, and the study day). So an
invalid configuration is reported alone, and `srl check` stops there; run
it again once the configuration is fixed to see the other problems.

`srl check` checks only what `srl` needs. Whether other programs can open
the store's files, for example paths too long for some programs on Windows, is not
its concern.

**Warnings** are likely mistakes `srl check` can't call errors: items that
are valid but probably not what you meant. A collection's format is
structured enough that a mistake in it is an error, so the warnings are
those of the `qa` kind, the same in every source: in the `Q` of a valid
`qa` item, something that was probably meant to be a cloze deletion
(`{{C1::…}}`, or `{{c1:…}}` with one colon), and a deletion whose hint is
the text it hides. Each is reported once per item, as `srl: warning:
COLLECTION: item ID: message`, in the order of the items. `srl check` knows
only the `qa` kind: it doesn't look at the fields of items of kind
programs, and a source program reports what it finds as problems of
`read` (see [sources.md](sources.md), "Source programs").
