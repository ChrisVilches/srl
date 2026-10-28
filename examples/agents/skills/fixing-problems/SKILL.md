---
name: fixing-problems
description: Diagnose and fix srl problems - errors in notes that stop every command, a card a kind program can't make that stops srl next, srl check and srl lint, items that silently aren't items, history left behind after editing cloze numbers (orphans and srl move), duplicate IDs, and locking or cache messages. Load whenever an srl command fails, or when cards are missing or behave unexpectedly.
---

# Fixing problems

## Errors stop every command

Any error in any note stops every command that reads items (`srl find`,
`srl next`, ...). Each error is printed on standard error as
`PATH:LINE: message`, and the exit code is 2. Fix them all, then run the
command again. `srl check` lists every problem in the store at once,
except after an invalid configuration: it reports that alone and stops,
since the notes and the log can't be read without it. Fix it and run
`srl check` again.

| Error | Usual cause | Fix |
|---|---|---|
| srl block never closed | a ```` ```srl ```` without its closing ```` ``` ````, or a field's code block that closed it early | Close the block; when a field holds a code block, open the srl block with `~~~srl` and close it with `~~~` (see "An item cut short, or gone" below). |
| first line of an srl block, or block that mixes forms | a blank line or text right after the fence, entries on the fence of a block of `@` items, or an `@` line in a block of one item | Start the block with a field line (one item) or an `@` line (several); put entries on the `@` lines when there are some. |
| line not part of an item in an srl block | a continuation line without its two spaces, or prose inside the block | Indent the continuation with two spaces, or move the prose out of the block. |
| header not followed by a field line | a blank line or a paragraph right after an `@` line | Put `Q:` on the line right after it. |
| line starts with one space or a tab | a continuation line indented wrong | Indent continuation lines with exactly two spaces. |
| item without an ID | a new item | Run `srl id`. |
| duplicate ID | an item was copied | Delete `id:...` from the copy, then `srl id`. |
| unknown or repeated header entry | a typo such as `darft` | Use only `id:`, `kind:`, `see:` and `draft`. |
| unknown field, missing `Q` or `A` | `B:`, or a `qa` item without `A` and without cloze | Use `Q` and `A` only; `A` is optional only with cloze. |
| bad cloze | `{{c1::` without `}}`, empty text, `c0`, `c01`, nested, or cloze in `A` | Fix the deletion. |
| `PATH:LINE: FIELD: message` on an item of a kind program, or there is no such kind | the fields don't suit the kind (its `validate` said so), or `kind:` names no kind of `[kinds]` | Fix the field as the message says (`srl kinds` lists the kind's fields), or the `kind:`. |
| `the program of kind X failed …; no item of kind X can be read until the program works` | the program itself is broken: not installed, a missing dependency, a bug; not your notes | Fix or reinstall the program, or its command in `[kinds]`; run `srl check` again. |
| `the program of kind X printed invalid output …` | a bug in the program | Report it to its author; the item is fine as written. |
| `see:` doesn't name a note | the note was moved or renamed | Update the path, relative to the store. |
| invalid configuration | a typo or out-of-range value in `.srl/config.toml` | Fix the named setting. |

`srl show ID` prints an item exactly as written, which helps spot the
problem.

## A card that can't be made

`srl next` picks one card. When its kind program fails to make it, `srl
next` fails with exit 2 and `cannot make the card ID (...)`, followed by
the program's reason, and shows no other card instead. It fails the same
way every time, because nothing changes until someone acts:

- **Fix what failed**, as the reason says: a missing API key, an API that
  is down, a program that isn't installed, a program that took longer
  than `card_timeout`. Then run `srl next` again.
- **Or set the card aside** with `srl suspend id:ID`, and later
  `srl unsuspend id:ID`. Suspending changes what the person studies, so
  ask them first.

`srl cards ID` makes the card too, so it shows whether the fix worked.
The person can also see it rendered in the Browse tab of `srl-web`.

## Text that should be an item but isn't

Only srl blocks make items, so `Q:` lines outside one are ordinary text,
and no error is reported. `srl lint` finds such likely mistakes: `Q:` or
`A:` lines outside blocks (also in lists and quotes), indented or
misspelled block fences (```` ```SRL ````, ```` ```srl{…} ````), cloze
deletions outside items, and srl blocks closed too early (see below). It prints `PATH:LINE: message` and never
fails; exit 0 means it found something.

## An item cut short, or gone

When a field holds a fenced code block and its srl block was opened with
```` ``` ````, the code's closing fence closes the srl block. The rest of
the item becomes text of the note, and the srl block's own closing fence,
left over, opens a code block that hides the next srl blocks until another
fence.

- `srl check` reports it only when the cut item is invalid, such as a `qa`
  item that lost its `A:`. An item whose `Q` has cloze deletions, or an
  item of a kind program, is just shorter, and the hidden items just gone.
- `srl lint` reports it every time: "this indented fence closes the srl
  block of line N…", at the code's closing fence, which is indented by
  the two spaces of its continuation line.

Other signs: `srl show ID` prints fewer or shorter fields than the note
seems to hold; `srl find in:NOTE` lists fewer items than the note has. The
fix: open that srl block with `~~~srl` and close it with `~~~`, or with
more backticks than any fence inside it. Then run `srl check`, `srl lint`
and `srl find` again.

## History left behind: orphans and `srl move`

A card's schedule belongs to its card ID. Some edits change the ID:

- adding the first cloze to an item: `x7k2m9` becomes `x7k2m9-1`;
- renumbering cloze deletions (`c2` becomes `c3`);
- changing an item's ID by hand.

The old card's reviews stay in the log under the old ID, an **orphan**.
`srl check` reports orphans, and prints the fix when there is an obvious
one:

```
srl: x7k2m9 has reviews but no card; x7k2m9-1 is new. To keep the history: srl move x7k2m9 x7k2m9-1
```

Run `srl move FROM TO` if the person wants the new card to keep the old
schedule (ask them first). `TO` must have no history of its own. A deleted
item also leaves an orphan; that is harmless, and `srl check` still exits
with 0 when orphans are the only thing it reports.

Editing an item's text, moving it, or renaming its note never changes its
schedule.

## Suspending and resetting

- `srl suspend SEL`: stop studying cards, without deleting them; the note
  isn't changed. `srl unsuspend SEL` undoes it. `srl find status:suspended`
  lists them.
- `srl reset SEL`: make cards new again, for a fresh start. History stays
  in the log. Check the selector with `srl find SEL` (or `--dry-run`)
  first, since an empty selector matches every card.
- Deleting an item from a note removes its cards. Its history stays in the
  log.

## Other messages

- **"is not a store"**: the command didn't find a store. `cd` into it, or
  use `--dir` or `SRL_DIR`; `srl init` creates one.
- **A note that changed while srl was working is not written**: an editor
  saved it at the same moment. Run the command again.
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
  built-in frontend, and doesn't run other programs. Study with a frontend
  run by its own name, such as `srl-web` or `srl-tui` (see the `studying`
  skill); scripts use `srl next` and `srl review`.
- **An invalid configuration naming `study`**: `.srl/config.toml` has a
  `[study]` section, which stores made by older versions have. It is no
  longer a setting: each frontend chooses the context with `--context`.
  Delete the `[study]` line and the `context` line under it, telling the
  person why.
