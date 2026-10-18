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

> **TODO (platform):** "One append can't be interleaved with another" holds
> for files opened in append mode on Linux and macOS local file systems, but
> isn't guaranteed on Windows or on network file systems. Before `srl review`
> is written, decide how to make concurrent appends safe everywhere (for
> example a lock file around each append).

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
| `t` | Time of the review, ISO 8601 with offset. |
| `atom` | The atom's ID. A card about several atoms produces one event per atom. |
| `rating` | 1–4, as stored (after the `choice` rule). |
| `card` | Which card was used: drill, ref and grading mode. `null` when reviewed without a card (`srl review ID --rating N`). |
| `response` | What you answered, if anything. |
| `by` | `human` or `agent`: who ran the review. |
| `grader` | `self`, `exact`, `choice`, or `cmd:<command>`: what judged the answer. |
| `hash` | The first 12 hex characters of the SHA-256 of the atom file at review time. |

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
