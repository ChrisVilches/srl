---
name: fixing-problems
description: Diagnose and fix srl problems - errors in items or sources that stop every command, a card a kind program can't make that stops srl next, srl check and its warnings, history left behind after editing cloze numbers (orphans and srl move), duplicate IDs, locking or cache messages, and revising suspended cards by the reasons they were suspended with. Load whenever an srl command fails, when cards are missing or behave unexpectedly, or when the person wants to fix cards they suspended.
---

# Fixing problems

## Errors stop every command

Any error in any source stops every command that reads items (`srl find`,
`srl next`, ...). Each error is printed on standard error, and the exit
code is 2: `COLLECTION: item ID: message` for an item (`item N`, by its
position, when it has no valid ID), or `COLLECTION: message` for a
collection, where the message says where in it, such as `line 3:` in a
JSONL or XML file. The collection is named by its path, for a file of
the store. Fix them all, then run the command again. `srl check` lists
every problem in the store at once, except after an invalid
configuration: it reports that alone and stops, since the sources and the
log can't be read without it. Fix it and run `srl check` again.

| Error | Usual cause | Fix |
|---|---|---|
| item without an ID | a new item written by hand | Run `srl id`. |
| duplicate ID | an item was copied | Delete the `id` from the copy, then `srl id`. |
| a line of a JSONL collection that is not a valid item object | text that isn't JSON, an object over several lines, an unknown or misspelled key, a key written twice, a value of the wrong type (`"draft": "yes"`, a number for text) | Make the line one JSON object with the keys `id`, `kind`, `see`, `draft` and uppercase field names (see the `writing-items` skill). |
| `line N: …` in an XML collection | XML that isn't well-formed, an attribute other than `id`, `kind`, `see`, `draft`, a lowercase element in an `<item>`, `<` or `&` in a field outside CDATA | Fix the line as the message says; write a field with `<` or `&` as `<![CDATA[…]]>`. |
| `it is not a JSON array` or `item N: …` in a JSON collection | the file isn't one array of item objects | Fix the element at that position. |
| `srl reads collections in the formats …` | a source's `include` matches a file that isn't `.jsonl`, `.json` or `.xml`, such as a note | Narrow `include`, or `exclude` the file. |
| unknown field, missing `Q` or `A` | `B`, or a `qa` item without `A` and without cloze | Use `Q` and `A` only; `A` is optional only with cloze. |
| bad cloze | `{{c1::` without `}}`, empty text, `c0`, `c01`, nested, or cloze in `A` | Fix the deletion. |
| `COLLECTION: item ID: FIELD: message` on an item of a kind program, or there is no such kind | the fields don't suit the kind (its `validate` said so), or `kind` names no kind of `[kinds]` | Fix the field as the message says (`srl kinds` lists the kind's fields), or the `kind`. |
| a reason such as `cannot find the program …` or `the program of kind X failed …`, ending `; no item of kind X can be read until the program works` | the program itself is broken: not installed, a missing dependency, a bug; not your items | Fix or reinstall the program, or its command in `[kinds]`; run `srl check` again. |
| `the program of kind X printed invalid output …` | a bug in the program | Report it to its author; the item is fine as written. |
| `` `see` … is not study material `` or `` `see` … names no file of the store `` | the note was moved, renamed or deleted, or `see` names a collection or a file `[context]` doesn't include | Name a note `srl notes` lists, or widen `[context]` in `.srl/config.toml`. |
| invalid configuration | a typo or out-of-range value in `.srl/config.toml` | Fix the named setting. |
| `COLLECTION: the sources X and Y both give it …` | two sources' globs match the same file, or a source program names a collection as a file is named | Leave it out of one of them with `exclude`, or have the program name its collections under a folder of its own. |
| `the source X failed: …` | a source program is broken: not installed, a missing dependency, a bug, a wrong setting of its own in `[sources.X]`, or a service it needs is down; not your items | Fix what the reason says, or the program's command and settings in `[sources.X]`; run `srl check` again. |
| `cannot add to COLLECTION: no source accepts it (SOURCE: reason; …)` or `… more than one source accepts it (SOURCE, SOURCE)`, from `srl add` | no source can create that collection (each says why: its `include` doesn't match, its `exclude` does, the extension names no format, or a program's globs don't match), or more than one would | Pick a collection a source can create (`srl sources` says where), or an existing one (`srl collections`). For several, narrow their globs so that only one accepts it. |

`srl show ID` prints an item's entries and fields as `srl` read them, one
per line, which helps spot the problem; its collection, which `srl show`
names, has it as written.

## A card that can't be made

`srl next` picks one card. When its kind program fails to make it, `srl
next` fails with exit 2 and `cannot make the card ID (...)`, followed by
the program's reason, and shows no other card instead. It fails the same
way every time, because nothing changes until someone acts:

- **Fix what failed**, as the reason says: a missing API key, an API that
  is down, a program that isn't installed, a program that took longer
  than `card_timeout`. Then run `srl next` again.
- **Or set the card aside** with `srl suspend id:ID --reason "…"`, saying
  what failed, and later `srl unsuspend id:ID`. Suspending changes what
  the person studies, so ask them first.

`srl cards ID` makes the card too, so it shows whether the fix worked.
The person can also see it rendered in the Browse tab of `srl-web`.

## Likely mistakes: warnings of `srl check`

A collection's format is strict, so a mistake in it is an error. Some
items are valid but probably not what was meant; `srl check` warns of
them on standard error, as `srl: warning: COLLECTION: item ID: message`,
without failing: in the `Q` of a `qa` item, something that looks like a
cloze deletion but isn't (`{{C1::…}}`, `{{c1:…}}`), shown as written, and
a deletion whose hint is the text it hides. Fix them as the message says.

Items written in the person's Markdown notes are not items at all: notes
are study material, and `srl` reads no items from them. Move such
questions into a collection with `srl add`, naming the note with `see`.

## History left behind: orphans and `srl move`

A card's schedule belongs to its card ID. Some edits change the ID:

- adding the first cloze to an item: `x7k2m9` becomes `x7k2m9.1`;
- renumbering cloze deletions (`c2` becomes `c3`);
- changing an item's ID by hand.

The old card's reviews stay in the log under the old ID, an **orphan**.
`srl check` reports orphans, and prints the fix when there is an obvious
one:

```
srl: x7k2m9 has reviews but no card; x7k2m9.1 is new. To keep the history: srl move x7k2m9 x7k2m9.1
```

Run `srl move FROM TO` if the person wants the new card to keep the old
schedule (ask them first). `TO` must have no history of its own. A deleted
item also leaves an orphan; that is harmless, and `srl check` still exits
with 0 when orphans and warnings are the only thing it reports.

Editing an item's text, moving it, or renaming its collection never changes its
schedule.

## Suspending and resetting

- `srl suspend SEL`: stop studying cards, without deleting them; the item
  isn't changed. `srl unsuspend SEL` undoes it. `srl find is:suspended`
  lists them.
- `srl suspend SEL --reason TEXT` also keeps why: what the person found
  wrong with the card, so that it can be fixed later. `srl find --json`
  gives it as `suspend_reason` (`null` when none was given), and `srl log`
  shows it. Suspending cards that are already suspended with a new
  `--reason` replaces their reason; without `--reason` they are left
  alone. When you suspend a card the person complained about, put their
  complaint in `--reason`, in their words.

## Revising suspended cards

People suspend cards they dislike or that are too hard for now, saying
why, and come back later to fix them. When asked to:

1. `srl find is:suspended --json` lists them; each line's
   `suspend_reason` says what to fix. Cards without a reason: ask the
   person, or look at their reviews with `srl log id:ID`.
2. `srl show ITEM --json` gives the item (`item` in the find line), with
   its context. Fix what the reason says: a clearer question or answer,
   more context (a `see` naming a better note), or split the item into two
   with `srl add` and remove the old one (see the `writing-items` skill).
   Show the person the new item before writing it with `srl edit --stdin`.
3. Ask before `srl unsuspend id:ID`: it puts the card back into their
   studying. A rewritten item keeps its card's schedule; `srl reset id:ID`
   starts it over, if the person wants that. A split item's new cards are
   new; its old cards are gone, so there is nothing to unsuspend.
- `srl reset SEL`: make cards new again, for a fresh start. History stays
  in the log. Check the selector with `srl find SEL` (or `--dry-run`)
  first, since an empty selector matches every card.
- Deleting an item from its collection removes its cards. Its history stays in the
  log.

## Other messages

- **"is not a store"**: the command didn't find a store. `cd` into it, or
  use `--dir` or `SRL_DIR`; `srl init` creates one.
- **A collection that changed while srl was working is not written**: an
  editor saved it at the same moment, or an item to change was removed
  from it, or it has an error now. Run the command again, after `srl
  check` if the file may be broken. An item that only moved is still
  found, by its ID.
- **A collection can't be renamed into place**: `srl` writes a collection
  to `.srl/tmp/` and renames it over the file, which fails when the file
  is on another file system than `.srl/`, such as a mounted folder. The
  file is left as it was. Keep collections on the same file system as the
  store. Files left in `.srl/tmp/` by a killed `srl` can be deleted when
  no `srl` is running.
- **XML can't hold a carriage return**: an item whose text has one can't
  be written to an XML collection; keep it in a JSONL collection.
- **The log's lock**: another `srl` was recording a review for more than 5
  seconds. Try again; on a network share, move the store to a local disk.
- **Log lines skipped**: a line of `.srl/log/*.jsonl` isn't a valid event,
  usually from a hand edit or a sync conflict. The message names the file
  and line. Every other command carries on without it. The log is the
  person's review history: show them the line, and change nothing in it
  yourself.
- **Index cache messages**: `.srl/cache/` is only a cache. Deleting it
  loses nothing; the next command builds it again.
- **A kind program was changed, but items of its kind act as before**:
  srl keeps the answers of `validate` and `groups` per item, and can't see
  that the program changed. Run `srl check` so it asks again about every
  item.
- **unrecognized subcommand `study`, `web` or `tui`**: `srl` has no
  built-in frontend, and doesn't run other programs. Study with a
  frontend run by its own name, such as `srl-web` or `srl-tui` (see the
  `studying` skill); scripts use `srl next` and `srl review`.
