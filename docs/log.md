# The review log

The log is the record of everything that happened to atoms' memory. Together
with the atom files, it is the store's source of truth: every memory state is
computed from it.

## Files

- `log/YYYY-MM.jsonl`, one file per month, one event per line.
- **Append-only.** Events are never changed or deleted. Only `srl review` and
  `srl reset` write to the log; never edit it by hand.
- Each event is written as a single line in one append, so several frontends
  can record reviews at the same time without corrupting the file.
- **Every append holds the log's lock.** `srl review` and `srl reset` take
  an exclusive lock on the empty file `log/.lock` (created on first use)
  for the whole append, through the operating system's file locking, the
  same on every platform. Another `srl` that wants to append waits for it,
  for at most 5 seconds, and then fails without recording anything. The
  operating system releases the lock when a process ends, so a crash never
  leaves the log locked. Reading the log doesn't take the lock.
- An event goes into the file of the month of its own time `t` (its local
  date, with its offset).

A command that reads the log while an event is being written may, on some
platforms, see only part of that event's line. It reports that line as
skipped, like any invalid line; the next command reads the whole line.

> **TODO (platform): locks on network file systems.** On NFS and SMB the
> lock depends on the server: NFS needs its lock service, and some servers
> don't support locks at all. Where locking fails, `srl review` fails with
> that error instead of writing unlocked. Where a server accepts locks but
> doesn't share them between machines, two machines recording reviews at the
> same moment could still mix their lines into one corrupt line. Keep the
> store on a local disk to be safe. The code that appends (`lock` in
> `crates/srl-core/src/log.rs`) carries the same TODO.

Reading the log, a line that is not a valid event (not JSON, a field
missing or of the wrong type, an unknown field, a rating outside 1–4, a time
without an offset) is skipped and reported on standard error, with its file
and line number. Every command carries on with the other events.

## Events

### `review`

```json
{"type": "review",
 "t": "2026-10-26T09:12:31+09:00",
 "atom": "a7f3k2",
 "rating": 3,
 "card": {"drill": "cloze", "ref": "cloze:0:1"},
 "response": null,
 "by": "human",
 "hash": "3fa91c0d22be"}
```

| Field | Meaning |
|---|---|
| `type` | `review` |
| `t` | Time of the review, ISO 8601 to the second, with the local offset. |
| `atom` | The atom's ID. A card about several atoms produces one event per atom. |
| `rating` | 1–4, as given (see [scheduling.md](scheduling.md), "Ratings"). |
| `card` | Which card was used: drill and ref. `null` when reviewed without a card (`srl review ID --rating N`). |
| `response` | What was answered, as text, if the review was recorded with one (`srl review --stdin`), for example an answer typed to an agent. `null` if none was given, which is usual: rating yourself needs no typed answer. Always `null` without a card. `srl` never reads it. |
| `by` | `human` or `agent`: who recorded the review. It says nothing about how the rating was decided, or whether there is a `response`: an agent may record a rating you gave it, or one it judged from your answer. |
| `hash` | The first 12 hex characters of the SHA-256 of the atom file at review time. |

Every field is always written, with `null` where it doesn't apply.

`hash` is never used by `srl`. It exists so that you can later ask questions
like "did my retention change after I rewrote this atom?".

### `reset`

```json
{"type": "reset", "t": "2026-10-26T10:00:00+09:00", "atom": "a7f3k2"}
```

Replay of the atom's memory state starts empty after this event.

## Reading the log

The log is JSONL, so ordinary tools work:

```sh
srl log tag:jlpt/n1 --json                       # events for matching atoms
jq -c 'select(.rating == 1)' log/*.jsonl          # every Again
duckdb -c "select card.drill, avg(rating) from 'log/*.jsonl'
           where type = 'review' group by 1"      # average rating per drill
```
