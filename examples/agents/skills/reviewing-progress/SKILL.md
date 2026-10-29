---
name: reviewing-progress
description: Report how the person's studies are going with srl stats and srl log, find weak spots and leeches, explain the card states, and analyze the review log with jq or duckdb. Load when the person asks about progress, retention, workload, difficult cards, or what to improve.
---

# Reviewing progress

## `srl stats [SELECTOR]`

```
cards  120
new  30
due  12
young  41
mature  37
leech  2
reviews today  25
retention  88.5% of 400 reviews in 30 days
```

- `new`, `due`, `young` and `mature` add up to `cards`; `leech` overlaps
  them.
- `retention` is the share of reviews in the last 30 study days that were
  not Again, counting only cards that were not new at the time. Compare it
  with `desired_retention` (default 90%): well below means the cards are
  too hard or too vague; well above is fine but costs more reviews than
  needed.
- It applies the selector as written, so drafts and suspended cards count
  unless the selector says `status:active`.
- `--json` gives `{"cards", "states": {...}, "reviews_today", "retention",
  "retention_reviews"}`, with `retention` from 0 to 1 or `null`.

Per-topic comparisons are a selector each: `srl stats in:grammar`,
`srl stats in:vocab`.

## States

| State | Meaning |
|---|---|
| `new` | Not reviewed since the last reset. |
| `due` | Reviewed before, and due today or earlier. |
| `young` | Not due; stability under 21 days (`mature_days`). |
| `mature` | Not due; stability of 21 days or more. Known well. |
| `leech` | Forgotten (rated Again after a first review) 8 times or more (`leech_lapses`). |

## Leeches and weak spots

```sh
srl find state:leech          # the cards that keep being forgotten
srl show ID                   # read one, with its context
srl log id:ID                 # its whole history
```

A leech is almost always a badly written question, not a lack of effort:
too much in one card, an ambiguous prompt, or two cards that look alike.
Propose a rewrite (split it, add a distinguishing detail, turn it into
cloze), as a new draft item or, if the person asks, with
`srl edit --stdin` (see the `writing-items` skill). Then they may
`srl suspend` the old card or `srl reset` it.

## `srl log [SELECTOR]`

One line per event of the matching cards, in the order they were recorded:

```
2026-10-26T09:12:31+09:00  k3m9q1.2  review  3  human
2026-10-26T10:00:00+09:00  k3m9q1.2  reset
```

Events are `review` (with the rating and who recorded it, `human` or
`agent`), `reset`, `suspend`, `unsuspend` and `move`. `--json` prints them
as stored. Reviews may carry a `response`, what was answered in a chat
session, which helps understand mistakes.

## The log files directly

The log is `.srl/log/YYYY-MM.jsonl`, one JSON event per line, so ordinary
tools can analyze it (read only; never write to it):

```sh
jq -c 'select(.rating == 1)' .srl/log/*.jsonl                 # every Again
jq -r 'select(.type == "review") | .t[:10]' .srl/log/*.jsonl | sort | uniq -c   # reviews per day
duckdb -c "select by, avg(rating) from '.srl/log/*.jsonl'
           where type = 'review' group by 1"                  # average rating by who recorded it
```

A review event:

```json
{"type": "review", "t": "2026-10-26T09:12:31+09:00", "card": "k3m9q1.2",
 "rating": 3, "response": null, "by": "human", "hash": "3fa91c0d22be"}
```

`hash` identifies the item's text at review time, so reviews before and
after a rewrite of the item can be told apart.

## Workload

The daily workload is due cards plus new ones. If reviews pile up, the
levers are, in order: fewer new cards a day (`new_per_day`), suspending
cards that aren't worth it, and a lower `desired_retention`. Each new card
brings reviews for weeks, so the workload of today's new cards shows up
later.
