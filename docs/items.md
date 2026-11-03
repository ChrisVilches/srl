# Items

An **item** is a question, with its header entries and fields. Items come
from the store's **sources** (see [sources.md](sources.md)), each in one of
its **collections**, named everywhere by an **address**. Everything here
holds for every item, whatever its source: how a source keeps its items,
and how it writes them, is its own business. The two built-in drivers
keep them in **notes**, files of the store (see [notes.md](notes.md)); a
source program keeps them however it likes, such as in a database or
behind a web service (see [sources.md](sources.md), "Source programs").

An item has:

- **header entries**: its ID, its kind, the collection shown as its
  context, and whether it is a draft (see "Header entries");
- **fields**: named texts, such as `Q` and `A`, that its kind turns into
  cards (see [cards.md](cards.md), "Kinds").

Each source writes them its own way: on the fence of an srl block in a
Markdown note, as a line of JSON in a JSONL note (see
[notes.md](notes.md)), or as a source program keeps them. Everything after
reading the items (kinds, cards, scheduling, the log, selectors) is the
same for every source.

## Header entries

| Entry | Meaning | Default |
|---|---|---|
| `id` | The item's ID (see "Identity"). | none: an item without an ID is an error until `srl id` gives it one |
| `kind` | How the fields become cards (see [cards.md](cards.md), "Kinds"). | `qa` |
| `see` | The address of the collection shown as context instead of the one the item is in (see "Context"). | the item's own collection |
| `draft` | The item is a draft (see "Status"). | not a draft |

How they are written depends on the source: `id:x7k2m9` on a fence or an
`@` line in a Markdown note (see [notes.md](notes.md), "The srl block
syntax"), `"id": "x7k2m9"` in an item object (see "The item object"). In
every source, each entry may be given once, and a misspelled one is an
error.

## Fields

A field has a name and a text. Its name is uppercase letters `A`–`Z`,
digits and `_`, starting with a letter: `Q`, `A`, `TEXT`, `FORMS`. Its
text may hold anything, line endings included. No name is given twice in
an item, whatever its kind, and which fields an item takes is up to its
kind (see [cards.md](cards.md), "Kinds"). An item has at least one field.

## Identity

- **An item ID** is made of words of lowercase letters `a`–`z` and
  digits, joined by single `-`, 1 to 32 characters: `x7k2m9`, or
  `jp-grammar-desu` when you name it yourself. `-` is never first, last
  or doubled, and `.` is never part of an ID: it separates a card's
  suffix. `srl id` and `srl add` make random ones of 6 characters
  (`SRL_SEED` makes them repeatable; see [cli.md](cli.md),
  "Conventions"); you may also give one yourself, in the item or to `srl
  add`.
- **IDs are unique in the store**, across every source. Two items with the
  same ID are an error, which happens when an item is copied: delete the
  ID from the copy and run `srl id`.
- **The ID is the identity.** Edit the item, move it within its
  collection, to another collection or to another source, rename its
  collection: the item keeps its cards and their history.
- An item gives one or more cards, and each card has its own ID, made from
  the item's: `x7k2m9`, or `p4n8d2.1` and `p4n8d2.2` (see
  [cards.md](cards.md), "Card IDs"). Because an item ID never contains `.`,
  a card ID always names its item.
- **An item's line is for people only.** `ADDRESS:LINE` in listings and
  messages, and `line` in JSON, say where the item starts, as near as its
  source can say, or nothing when it can't. `srl` never finds an item by
  its line: commands that change an item find it by its ID in its
  collection as it is (see "How srl changes items").
- **Editing an item never resets its cards.** `srl reset` is the way to
  start over. When an edit makes a card disappear, its history is kept in
  the log, and `srl move` can give it to another card (see
  [scheduling.md](scheduling.md), "Moving history").

## Status

Every card has one status, usable in selectors as `status:NAME`:

| Status | Studied? | Comes from |
|---|---|---|
| `draft` | no | The item has `draft`. |
| `suspended` | no | The card was suspended with `srl suspend`, and not unsuspended since (see [log.md](log.md)). |
| `active` | yes | Neither. |

A card that is both a draft and suspended is `draft`.

Drafts exist so that items written by a program, such as a generator or
an import, never enter your study queue without your approval. `srl accept
SELECTOR` removes `draft` from the matching items. Suspending is about
studying, not about the item, so it is recorded in the log and never
changes the item. A suspension may say why (`srl suspend --reason`), such
as what to fix in the item before studying it again; `srl find --json`
gives it as `suspend_reason`.

## Context

An item's **context** is the collection named by `see` if the item has
one, and otherwise the collection the item is in, **as its source shows
it**. Frontends show it after the answer of each of the item's cards,
taking it from `srl next --json`; `srl show` prints it too.

- `see` is the address of a collection of the store, of any source: for
  a note of a source without a prefix, its path relative to the store's
  folder. A `see` that names anything else is an error. What a collection
  shows as context is up to its own source, whatever source the item
  comes from (see [sources.md](sources.md), "What a source does").
- `see` is meant for items that ask about another collection, such as
  items a script imported or generated into a collection of their own.

The context is given in one of three **modes**. How much of a collection
to show is a choice of whoever shows it, so the mode is not a setting of
the store: each frontend chooses it with `--context MODE` on `srl next`,
and `srl show` takes the same flag. Without it, the mode is `prose`.

| Mode | Shows |
|---|---|
| `prose` | The collection without its items: what is written around them, for people. |
| `full` | The collection with its items. |
| `none` | Nothing. |

What each mode shows of a collection is up to its source: for the notes
of the built-in drivers, see [notes.md](notes.md), "Context"; a source
program gives its own (see [sources.md](sources.md), "Calls"). A source
may have no context to give for a collection, as a JSONL note has none in
mode `prose`.

**A context with nothing but blank space left is no context**, exactly as
in mode `none`: frontends show nothing, and `context` is `null` in JSON. So
a collection that holds only items has no context in `prose` mode.

**A context comes with the address of its collection**, given as
`context_address` in JSON: the collection named by `see`, or else the
item's own collection. It is `null` when `context` is `null`. `srl` doesn't
say how to read the text: whoever shows it decides that from the address, as
it likes, with any fallbacks it wants (see [frontends.md](frontends.md),
"Rendering").

## How srl changes items

Only four commands ever change an item:

| Command | Changes |
|---|---|
| `srl id` | Gives an ID to the items that have none. |
| `srl accept SELECTOR` | Removes `draft` from the matching items. |
| `srl add ADDRESS` | Adds new items to the collection at `ADDRESS`, or creates it. |
| `srl edit --stdin` | Replaces the items it is given with new ones, for scripts and other programs. |

Every other command only reads items. The rules, for every source:

1. **Every change is checked before anything is written.** `srl` checks
   the items it is about to write (item objects, IDs, kinds, `see`), and
   each source checks its own changes, before any source writes. If any
   change is refused, or any source has an error, the command reports
   every error and changes nothing. For `srl id`, an item without an ID
   is not an error, since that is what it fixes.
2. **An item that changed while `srl` was working is not written.** Each
   item to change is found in its collection as it is when its source
   writes it: by its ID, or, for `srl id`, by its place among its
   collection's items. If it is not there any more, or not as `srl` read
   it, the command stops and says so; changes written before stay
   written, and running it again finishes the work.
3. **Every change is reported.** `srl id` prints one line per item it gave
   an ID: the ID and `ADDRESS:LINE`, separated by two spaces. `srl accept`,
   `srl add` and `srl edit` print the IDs of the items they changed or
   added (see [cli.md](cli.md)). With `--dry-run` they write nothing, and
   print the same (`srl add`: the items as their source would add them).

How a source writes its changes is its own: the built-in drivers change
only the lines of the items changed, and write atomically (see
[notes.md](notes.md), "How srl writes to notes"); a source program writes
its own way, and `srl` reads it again to check the result (see
[sources.md](sources.md), "What srl checks"). `srl add`, `srl edit` and
their formats are described in [cli.md](cli.md).

## Errors

Every command that reads items reads every source, and **any error in any
source stops it**: it reports every error it found, as `ADDRESS:LINE: message`
(`ADDRESS: message` where there is no line) on standard error, one per
line, in the order of the addresses and their lines, and exits with
status 2. The errors are those of each source's format (for the notes of
the built-in drivers, see [notes.md](notes.md), "Errors"), and, for every
source:

- an unknown, repeated or empty header entry;
- an item without an ID (`srl id` fixes it), an invalid ID, or an ID that
  another item already has;
- a field name written twice in one item, whatever its kind;
- an unknown kind, or fields that the kind doesn't accept (see
  [cards.md](cards.md));
- a kind program that fails as a whole, reported once, without a line
  (see [cards.md](cards.md), "How srl calls them");
- a source program that fails as a whole or prints invalid output,
  reported once, as a problem of the source (see [sources.md](sources.md),
  "What srl checks");
- an item in a collection its source doesn't list;
- a `see` that doesn't name a collection of the store;
- a collection that breaks a rule of addresses (see
  [sources.md](sources.md), "Addresses"), reported as `ADDRESS: message`.

`srl id` is the exception for items without an ID: that is the error it
fixes. `srl check` reports the same errors and more (see [cli.md](cli.md)).

> **TODO (design):** stopping at every error can get in the way. Ways to
> make it less annoying are wanted, among them turning some errors into
> warnings and skipping the broken items; none is decided (see
> [overview.md](overview.md), "Open questions").

## The item object

The **item object** is an item as one JSON object: its header entries and
its fields side by side. It is the one JSON form of an item everywhere:

- a line of a JSONL note (see [notes.md](notes.md), "The JSONL syntax");
- a line of `srl add --stdin` and of `srl edit --stdin` (see
  [cli.md](cli.md));
- what `srl show --json` prints, with every entry and the keys only `srl`
  gives;
- an item a source program reads or is given to write (see
  [sources.md](sources.md), "Calls").

So what `srl show --json` prints can be changed and given back to `srl
edit --stdin` as it is.

```json
{"id": "p4n8d2", "kind": "qa", "see": null, "draft": false,
 "Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。",
 "address": "japanese/grammar/wo-mono-tomo-sezu.md", "source": "notes",
 "file": "japanese/grammar/wo-mono-tomo-sezu.md", "line": 11,
 "cards": ["p4n8d2.1"],
 "context": "# 〜をものともせず\n\nUndeterred by (adversity), braving. …\n",
 "context_address": "japanese/grammar/wo-mono-tomo-sezu.md"}
```

Its keys:

| Key | Value |
|---|---|
| `id` | The item's ID, as text. |
| `kind` | The kind's name, as text. `"qa"` is the same as leaving it out. |
| `see` | The address of the collection shown as context, as text. `null` is the same as leaving it out. |
| `draft` | `true` or `false`. `false` is the same as leaving it out. |
| a field name (see "Fields") | The field's text. |
| `address`, `source`, `file`, `line`, `cards`, `context`, `context_address` | Given by `srl` only (see below), and ignored wherever an item object is read. |

- Header entries are lowercase and field names uppercase, so they never
  clash. Any other key is an error, so a typo such as `drfat` fails
  instead of being ignored. So is a key given twice, which JSON allows.
- The fields are in the order of their keys. An item object has at least
  one field.
- Text values can't be empty, except a field's. Values of the wrong type
  are errors: an ID, a kind or a `see` written as a number, a draft
  written as `"yes"`.

`srl show --json` prints every entry, defaults included, its fields, then
the keys only `srl` gives:

- `address` is the address of the collection the item is in, `source`
  the name of its source, `file` the file the collection is kept in
  (`null` when it is no file of its own, as for a table of a database),
  and `line` the line where the item starts, counting from 1, as its
  source gives it (for the notes of the built-in drivers, see
  [notes.md](notes.md), "An item's line"), or `null` for an item of a
  source that gives no line (see [sources.md](sources.md), "Source
  programs").
- `cards` lists the IDs of the item's cards, in order.
- `context` is the context in the mode chosen, or `null` when there is
  none (mode `none`, a collection with nothing to show in that mode, or
  nothing but blank space left).
- `context_address` is the address of the collection the context comes
  from, or `null` when `context` is (see "Context").
