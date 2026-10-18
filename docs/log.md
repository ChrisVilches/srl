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
- An event goes into the file of the month of its own time `t` (its local
  date, with its offset).

> **TODO (platform): concurrent appends are only safe on local Linux and
> macOS file systems.**
>
> `srl` writes each event with one `write` call to a file opened in append
> mode. On local Linux and macOS file systems the kernel makes such a write
> atomic with respect to other appends, so two frontends recording reviews at
> the same moment can't mix their lines.
>
> **Windows does not guarantee this, and neither do network file systems
> (NFS, SMB) on any platform.** There, two `srl review` runs at the same time
> could interleave their bytes into one corrupt line. Every later command
> would report that line and skip it, so both reviews would be lost from the
> schedule. A single person studying in one terminal never triggers this;
> two frontends, or a frontend and an agent, writing at the same moment can.
>
> Not solved yet. The likely fix is a lock file (for example `log/.lock`,
> created with `create_new` and removed after the append) around every
> append, on every platform, so that the rule is the same everywhere. The
> code that appends (`append` in `crates/srl-core/src/log.rs`) carries the
> same TODO.

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
 "card": {"drill": "cloze", "ref": "cloze:0:1", "grading": "exact"},
 "response": ["ものともせず"],
 "by": "human",
 "grader": "exact",
 "hash": "3fa91c0d22be"}
```

| Field | Meaning |
|---|---|
| `type` | `review` |
| `t` | Time of the review, ISO 8601 to the second, with the local offset. |
| `atom` | The atom's ID. A card about several atoms produces one event per atom. |
| `rating` | 1–4, as stored (after the `choice` rule). |
| `card` | Which card was used: drill, ref and grading mode. `null` when reviewed without a card (`srl review ID --rating N`). |
| `response` | What you answered, as given to `srl review --stdin`; `null` if nothing was given, and always `null` without a card. |
| `by` | `human` or `agent`: who ran the review. |
| `grader` | What judged the answer: the card's grading mode (`self`, `exact`, `choice`), `cmd:<command>` for a `cmd` card with `[grading] cmd` set, and `self` for a `cmd` card without it or for a review without a card. |
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
