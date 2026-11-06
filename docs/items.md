# Items

An **item** is a question, with its header entries and fields. Items come
from the store's **sources** (see [sources.md](sources.md)), each in one of
its **collections**, named everywhere by an **address**. Everything here
holds for every item, whatever its source: how a source keeps its items,
and how it writes them, is its own business. The built-in `filesystem`
driver keeps them in structured files of the store, JSONL, JSON or XML
(see [formats.md](formats.md)); a source program keeps them however it
likes, such as in a database or behind a web service (see
[sources.md](sources.md), "Source programs").

An item has:

- **header entries**: its ID, its kind, the note shown as its context,
  and whether it is a draft (see "Header entries");
- **fields**: named texts, such as `Q` and `A`, that its kind turns into
  cards (see [cards.md](cards.md), "Kinds").

Each source writes them its own way: as a JSON object in a JSONL or JSON
collection, as attributes and elements in an XML collection (see
[formats.md](formats.md)), or as a source program keeps them. Everything
after reading the items (kinds, cards, scheduling, the log, selectors) is
the same for every source.

## Header entries

| Entry | Meaning | Default |
|---|---|---|
| `id` | The item's ID (see "Identity"). | none: an item without an ID is an error until `srl id` gives it one |
| `kind` | How the fields become cards (see [cards.md](cards.md), "Kinds"). | `qa` |
| `see` | The path of the note shown as the item's context (see "Context"). | no context |
| `draft` | The item is a draft (see "Status"). | not a draft |

How they are written depends on the source: `"id": "x7k2m9"` in an item
object (see "The item object"), `id="x7k2m9"` on an `<item>` of an XML
collection (see [formats.md](formats.md), "XML"). In every source, each
entry may be given once, and a misspelled one is an error.

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
- **An item is named by its ID and its collection's address**, never by
  where it is in its collection: listings and messages give the address,
  and commands that change an item find it by its ID in its collection as
  it is (see "How srl changes items"). An item without an ID yet is named
  in messages by its position among its collection's items, counting
  from 1: `item 3`.
- **Editing an item never resets its cards.** `srl reset` is the way to
  start over. When an edit makes a card disappear, its history is kept in
  the log, and `srl move` can give it to another card (see
  [scheduling.md](scheduling.md), "Moving history").

## Status

Every card has one status, given as `status` in the JSON of `srl find`,
and usable in selectors as `is:NAME` (see
[selectors.md](selectors.md), "Status, schedule and leeches"):

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

An item's **context** is the **note** its `see` names: a file of study
material, such as your Markdown notes on what the item asks about.
Frontends show it after the answer of each of the item's cards, taking it
from `srl next --json`; `srl show` prints it too. **An item without `see`
has no context**: a collection is data, and is never shown as context.

**Notes are the files of the store that `[context]` allows** (see
[store.md](store.md), "Configuration"): by default every Markdown and
text file (`**/*.md` and `**/*.txt`), outside folders whose name starts
with `.`. `srl notes` lists them. A note may be a file of any format, and
holds no items: `srl` never reads items from it, and shows it whole.

- `see` is a note's path, relative to the store's folder, with `/`
  between folders: `japanese/grammar/wo-mono-tomo-sezu.md`. **A `see`
  that names anything else is an error**: a path `[context]` doesn't
  allow, such as a collection of items, or a file that doesn't exist.
- The context is the note's text as it is, without a UTF-8 byte order
  mark, read when it is shown. **A note that isn't UTF-8, or holds
  nothing but blank space, has no text to give**: `context` is `null` in
  JSON, and a frontend that wants to show it, such as an image or a PDF,
  reads the file named by `see` itself.
- Whoever shows the context chooses whether to: `srl next` and `srl show`
  take `--no-context`, which leaves it out without reading the note.

`srl` doesn't say how to read the text: whoever shows it decides that
from the path in `see`, as it likes, with any fallbacks it wants (see
[frontends.md](frontends.md), "Rendering").

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
   an ID: the ID and the address, separated by two spaces. `srl accept`,
   `srl add` and `srl edit` print the IDs of the items they changed or
   added (see [cli.md](cli.md)). With `--dry-run` they write nothing, and
   print the same (`srl add`: the items as their source would add them).

How a source writes its changes is its own: the `filesystem` driver keeps
each file as close to how it was as its format allows, and writes
atomically (see [formats.md](formats.md), "How srl writes to
collections"); a source program writes
its own way, and `srl` reads it again to check the result (see
[sources.md](sources.md), "What srl checks"). `srl add`, `srl edit` and
their formats are described in [cli.md](cli.md).

## Errors

Every command that reads items reads every source, and **any error in any
source stops it**: it reports every error it found on standard error, one
per line, in the order of the addresses and the items in them, and exits
with status 2. An error of an item is `ADDRESS: item ID: message`, or
`ADDRESS: item N: message` for an item read without a valid ID, by its
position; an error of a collection is `ADDRESS: message`, where the
message says where in the collection it is, when its source can, such as
`ADDRESS: line 3: message`. The errors are those of each source's format
(for the `filesystem` driver, see [formats.md](formats.md), "Errors"),
and, for every source:

- an unknown, repeated or empty header entry;
- an item without an ID (`srl id` fixes it), an invalid ID, or an ID that
  another item already has;
- a field name written twice in one item, whatever its kind;
- an unknown kind, or fields that the kind doesn't accept (see
  [cards.md](cards.md));
- a kind program that fails as a whole, reported once, without an
  address (see [cards.md](cards.md), "How srl calls them");
- a source program that fails as a whole or prints invalid output,
  reported once, as a problem of the source (see [sources.md](sources.md),
  "What srl checks");
- an item in a collection its source doesn't list;
- a `see` that doesn't name a note (see "Context");
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

- a line of a JSONL collection and an element of a JSON collection (see
  [formats.md](formats.md));
- a line of `srl add --stdin` and of `srl edit --stdin` (see
  [cli.md](cli.md));
- what `srl show --json` prints, with every entry and the keys only `srl`
  gives;
- an item a source program reads or is given to write (see
  [sources.md](sources.md), "Calls").

So what `srl show --json` prints can be changed and given back to `srl
edit --stdin` as it is.

```json
{"id": "p4n8d2", "kind": "qa",
 "see": "japanese/grammar/wo-mono-tomo-sezu.md", "draft": false,
 "Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。",
 "address": "japanese/grammar.jsonl", "source": "items",
 "cards": ["p4n8d2.1"],
 "context": "# 〜をものともせず\n\nUndeterred by (adversity), braving. …\n"}
```

Its keys:

| Key | Value |
|---|---|
| `id` | The item's ID, as text. |
| `kind` | The kind's name, as text. `"qa"` is the same as leaving it out. |
| `see` | The path of the note shown as context, as text. `null` is the same as leaving it out. |
| `draft` | `true` or `false`. `false` is the same as leaving it out. |
| a field name (see "Fields") | The field's text. |
| `address`, `source`, `cards`, `context` | Given by `srl` only (see below), and ignored wherever an item object is read. |

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

- `address` is the address of the collection the item is in, and
  `source` the name of its source.
- `cards` lists the IDs of the item's cards, in order.
- `context` is the text of the note `see` names, or `null` when there is
  none: no `see`, `--no-context`, or a note that isn't UTF-8 or holds
  nothing but blank space (see "Context").
