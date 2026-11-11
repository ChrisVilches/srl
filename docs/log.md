# The review log

The log is the record of everything that happened to the cards' memory.
Together with the sources' items, it is the store's source of truth: every memory
state and every suspension is computed from it.

## Files

- `.srl/log/YYYY-MM.jsonl`, one file per month, one event per line.
- **Append-only.** Events are never changed or deleted. Only `srl review`,
  `srl reset`, `srl suspend` and `srl unsuspend` write to the
  log; never edit it by hand.
- Each event is written as a single line in one append, so several frontends
  can record reviews at the same time without corrupting the file.
- **Every append holds the log's lock.** These commands take an exclusive
  lock on the empty file `.srl/log/.lock` (created on first use) for the
  whole append, through the operating system's file locking, the same on
  every platform. Another `srl` that wants to append waits for it, for at
  most 5 seconds, and then fails without recording anything. The operating
  system releases the lock when a process ends, so a crash never leaves the
  log locked. Reading the log doesn't take the lock.
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
without an offset, an invalid card ID) is skipped and reported on standard
error, with its file and line number. Every command carries on with the
other events.

## Events

Every event has a `type` and a time `t`: ISO 8601 to the second, with the
local offset. Every field is always written, with `null` where it doesn't
apply. Card IDs in events don't have to be cards of the store: a card whose
item was deleted keeps its events (see [scheduling.md](scheduling.md),
"Orphans").

### `review`

```json
{"type": "review",
 "t": "2026-10-26T09:12:31+09:00",
 "card": "k3m9q1.2",
 "rating": 3,
 "hash": "3fa91c0d22be"}
```

| Field | Meaning |
|---|---|
| `card` | The card's ID. |
| `rating` | 1–4, as given (see [scheduling.md](scheduling.md), "Ratings"). |
| `hash` | The first 12 hex characters of the SHA-256 of the item's question at review time: its kind and fields as an item object, written as `srl` writes one into a JSONL collection (see [formats.md](formats.md), "How srl writes to collections"), without its other entries, and with its fields in the order of their names: `{"A":"…","Q":"…"}`, or `{"kind":"conj","VERB":"…"}` for an item of a kind program. So it changes when the question changes, and not when the item is moved, rewritten in another format, given an ID, accepted, given another `see`, or has its fields written in another order. Reviews recorded before this rule keep the hash they were given: before fields were put in the order of their names, that of the fields in the order they were written, and before that, that of the item as `srl show` printed it. |

`hash` is never used by `srl`. It exists so that you can later ask questions
like "did my retention change after I rewrote this item?".

### `reset`

```json
{"type": "reset", "t": "2026-10-26T10:00:00+09:00", "card": "k3m9q1.2"}
```

The card is new again after this event (see [scheduling.md](scheduling.md),
"Reset").

### `suspend` and `unsuspend`

```json
{"type": "suspend", "t": "2026-10-26T10:10:00+09:00", "card": "x7k2m9", "reason": "Too hard for now; split it in two."}
{"type": "unsuspend", "t": "2026-11-02T08:00:00+09:00", "card": "x7k2m9"}
```

| Field | Meaning |
|---|---|
| `card` | The card's ID. |
| `reason` | `suspend` only: why the card was suspended, as text, if it was suspended with one (`srl suspend --reason`), so that it can be fixed later. `null` if none was given. Suspensions recorded before this rule have no `reason`, and are read as with `null`. |

A card is suspended when its last `suspend` or `unsuspend` event is a
`suspend` (see [items.md](items.md), "Status"). Its reason is the
`reason` of that last `suspend`: a new `suspend` replaces it, even with
`null`, and an `unsuspend` clears it. A `reset` keeps it, as it keeps the
suspension.

## Reading the log

The log is JSONL, so ordinary tools work:

```sh
srl log in:grammar --json                             # events of matching cards
jq -c 'select(.rating == 1)' .srl/log/*.jsonl          # every Again
duckdb -c "select card, avg(rating) from '.srl/log/*.jsonl'
           where type = 'review' group by 1"           # average rating per card
```
