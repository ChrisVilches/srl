---
name: fixing-problems
description: Diagnose and fix srl problems - errors in notes that stop every command, srl check and srl lint, items that silently aren't items, history left behind after editing cloze numbers (orphans and srl move), duplicate IDs, and locking or cache messages. Load whenever an srl command fails, or when cards are missing or behave unexpectedly.
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
| line starts like a header but isn't one | `<!-- srl` without `-->` on the same line, or text after `-->` | Put the whole header on one line. |
| header not followed by a field line | a blank line or a paragraph right after the header | Put `Q:` on the line right after the header. |
| line starts with one space or a tab | a continuation line indented wrong | Indent continuation lines with exactly two spaces. |
| item without an ID | a new item | Run `srl id`. |
| duplicate ID | an item was copied | Delete `id:...` from the copy, then `srl id`. |
| unknown or repeated header entry | a typo such as `darft` | Use only `id:`, `kind:`, `see:` and `draft`. |
| unknown field, missing `Q` or `A` | `B:`, or a `qa` item without `A` and without cloze | Use `Q` and `A` only; `A` is optional only with cloze. |
| bad cloze | `{{c1::` without `}}`, empty text, `c0`, `c01`, nested, or cloze in `A` | Fix the deletion. |
| a kind program failed or there is no such kind | the fields don't suit the program, or `kind:` names no kind of `[kinds]` | Fix the fields as the program's message says, or the `kind:`. |
| `see:` doesn't name a note | the note was moved or renamed | Update the path, relative to the store. |
| invalid configuration | a typo or out-of-range value in `.srl/config.toml` | Fix the named setting. |

`srl show ID` prints an item exactly as written, which helps spot the
problem.

## Text that should be an item but isn't

Only a header makes an item, so `Q:` lines without one are ordinary text,
and no error is reported. `srl lint` finds such likely mistakes: `Q:` or
`A:` lines outside items (also in lists and quotes), indented headers, and
cloze deletions outside items. It prints `PATH:LINE: message` and never
fails; exit 0 means it found something.

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
- **Index cache messages**: `.srl/cache/` is only a cache. `srl reindex`
  rebuilds it, and deleting it loses nothing.
- **unrecognized subcommand `study`, `web` or `tui`**: `srl` has no
  built-in frontend, and doesn't run other programs. Study with a frontend
  run by its own name, such as `srl-web` or `srl-tui` (see the `studying`
  skill); scripts use `srl next` and `srl review`.
- **An invalid configuration naming `study`**: `.srl/config.toml` has a
  `[study]` section, which stores made by older versions have. It is no
  longer a setting: each frontend chooses the context with `--context`.
  Delete the `[study]` line and the `context` line under it, telling the
  person why.
