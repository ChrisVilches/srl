# Scheduling

`srl` schedules **cards**. Every card has its own memory state, and every
rating goes to the card it was given for. The cards of one item are
scheduled separately; the only rule they can share is the sibling rule (see
below).

## Ratings

| Rating | Name | Meaning |
|---|---|---|
| 1 | Again | You did not know it. |
| 2 | Hard | You knew it, with real difficulty. |
| 3 | Good | You knew it. |
| 4 | Easy | You knew it without effort. |

**The rating is always given, never computed.** `srl` doesn't judge answers:
you see the answer and rate yourself, or an agent you asked to judge your
answer gives the rating. Either way, `srl review` records the rating as
given.

## FSRS, computed from the log

The scheduler is FSRS. A card's memory state (stability, difficulty, last
review, due day) is never stored as truth. It is computed by replaying the
log, in order.

### Replay

The implementation is the `fsrs` crate of the FSRS authors (FSRS-6). Every
command that needs memory states replays the whole log, one rule per step:

- Events are replayed in **log order**: files by name (so by month), lines
  in order. Their times are not sorted.
- Each review's **study day** comes from its own time `t`, with the offset
  written in it (see "Days, not minutes").
- **Days elapsed** is the review's study day minus the previous review's,
  and 0 for the first review. A review dated before the previous one, which
  can happen only out of log order, counts as 0 days.
- FSRS gives the new stability and difficulty for the rating, and an
  interval. The interval is **rounded to whole days, and at least 1**. The
  card is due on the review's study day plus the interval.
- A **lapse** is an Again (1) on a card that already had a review since its
  last reset.
- A `reset` event makes the card new again: no memory state, no lapses.
  Its status, and the time of its last review for the sibling rule, are
  kept.
- A `move` event gives everything the card `from` has to the card `to`, and
  leaves `from` with nothing (see "Moving history").
- `suspend` and `unsuspend` events only change the card's status (see
  [notes.md](notes.md), "Status").

Events about cards that no longer exist are replayed like any other; their
memory is kept, and used again if the card comes back or is moved.

Weights in `.srl/config.toml` are given to FSRS as they are. It takes 21
(FSRS-6), or 19 or 17 from older versions, and fills in the rest. Any other
count is an error.

Consequences:

- Changing `desired_retention` or the FSRS weights takes effect for the
  whole history at once.
- Replacing FSRS with another algorithm later only means replaying the same
  log differently.
- The index cache keeps the replayed memories for speed, and can always be
  rebuilt.

## Days, not minutes

Scheduling works in whole **study days**. A study day starts at
`day_starts_at` (default 4:00 local time), so a late-night session counts as
the day it began: the study day of a moment is its local date, or the day
before if the hour is earlier than `day_starts_at`. Local time is the
system's time zone. If `srl` can't find the time zone, commands that need
the study day fail and say so, rather than assume UTC. The current time
is the system's clock, or `SRL_NOW` when it is set (see [cli.md](cli.md),
"Conventions").

- Every interval is at least one day. A card rated Again is due again the next
  study day.
- There are no learning steps within a day.
- Once a card is rated, it is not due again the same day, so `srl next` will
  not return it again. Nothing else is needed to keep a card from repeating
  within a session.

## Choosing the next card: `srl next SELECTOR`

`srl next` puts the active cards matching the selector in this order, and
keeps those that the sibling rule allows:

1. **Due cards**, ordered by due day (oldest first), then by card ID.
2. **Then new cards**, if today's new-card limit is not reached: cards with
   no reviews since their last reset, in the order they are written: by
   note path, the order of items in the note, then card ID.

> **It picks one of the first `max_skip` + 1 of those, uniformly at
> random.**

`max_skip` (default 0) is set in the configuration (see
[store.md](store.md), "Configuration"). At 0, `srl next` always prints the
first card. At 3, it skips 0, 1, 2 or 3 cards, each as likely; when fewer
than 4 cards are left, it picks uniformly among those left. The cards it
skips are not set aside: `srl next` writes nothing, so they are still at
the front of the order the next time, and each call picks afresh (with
`SRL_SEED` set, every call picks the same way; see [cli.md](cli.md),
"Conventions"). Asking
again without reviewing, such as by reloading a frontend, can give a
different card.

If no card qualifies, `srl next` prints nothing and exits with status 1.
It picks one card only: if that card can't be made, `srl next` fails,
and doesn't go on to another (see [cli.md](cli.md), "Studying").

### Siblings

An item's kind puts its cards in **groups**, and the cards of one group
are **siblings** (see [cards.md](cards.md), "Kinds"). Siblings are cards
where seeing one would often give away another: what a cloze card hides
is in the prompt of its sibling, so a `qa` item's cards are one group. A
kind whose cards don't give each other away puts each in a group of its
own. For siblings:

> **Each study day, a group offers at most one card. If none of its cards
> was reviewed today, it offers the eligible card reviewed longest ago,
> where a card never reviewed counts as longest ago, and ties go to the
> lowest card ID** (see [cards.md](cards.md), "Card IDs", for their order).

A card is **eligible** if it is active, matches the selector, and is due,
or is new while today's new-card limit is not reached. "Reviewed" means its
last review in the log, whether before or after a reset.

This rule never starves a card. Take an eligible card B of a group with k
cards. Each day the group is studied, it shows one card, which then becomes
its most recently reviewed. A sibling shown before B is more recent than B
from then on, so it can't be chosen ahead of B again, and B stays eligible
until it is shown (a due card stays due until it is reviewed). So at most
k − 1 siblings go before B: **every eligible card is shown within k days of
study**, however short its siblings' intervals are. A card alone in its
group is never held back.

Consequences:

- New siblings are introduced one per day, in the order of their IDs:
  `.1`, then `.2`, then `.3`.
- A new card can go before a due sibling, which then waits a day; the new
  card counts towards `new_per_day`.
- The rule only decides what `srl next` offers. `srl review` can record any
  card at any time, and that review counts as "reviewed today" for its
  siblings.

### The new-card limit

`new_per_day` (default 10) is one global number: the count of cards whose
first review since their last reset happened today, across all selectors.
When it is reached, `srl next` only returns due cards until the next study
day.

## Reviews outside the schedule

Reviewing a card that is not due (for example `srl review x7k2m9 --rating 3`
after you studied it some other way) is allowed. The review is recorded
like any other, and FSRS treats it as an early review.

## Reset

`srl reset SELECTOR` appends a `reset` event for each matching card that is
not new. A new card has nothing to reset, so it gets no event. From then on,
the card is new again. Earlier events stay in the log for analysis.

Editing an item never resets its cards.

## Moving history

A card ID can stop existing while its history is still wanted: you changed
an item's kind, renumbered a cloze, added the first cloze to an item
(`x7k2m9` becomes `x7k2m9.1`), or changed an ID by hand. Its events stay in
the log under the old ID. `srl` never guesses where they belong:

- `srl check` reports each such **orphan**: an ID that has reviews in the
  log, not moved away, but no card in the store. Deleted items leave orphans too, so they are
  notices, not errors. When an orphan is the only one of an item that
  still exists, and that item has exactly one card with no events,
  `srl check` prints the command that would move the history there:

  ```
  x7k2m9 has reviews but no card; x7k2m9.1 is new. To keep the history: srl move x7k2m9 x7k2m9.1
  ```

  Otherwise it says only that the history stays in the log, so it never
  suggests two moves to one card.

- `srl move FROM TO` appends a `move` event. `FROM` must be an orphan, and
  `TO` a card of the store with no events at all, so history is never
  merged or overwritten. From that event on, `TO` has `FROM`'s memory
  state, lapses, last review and status, and `FROM` has nothing.

> **TODO (design):** `srl move` is hard to use by hand. To run it, a person
> has to know what an orphan is, which new card took the old one's place,
> and that a wrong move can't be undone. A helper that does this more
> safely is wanted, in `srl` or in a frontend or wrapper; nothing about it
> is decided (see [overview.md](overview.md), "Open questions").

## States

Every card has one derived state, usable in selectors as `state:NAME`:

| State | Meaning |
|---|---|
| `new` | No reviews since the last reset. |
| `due` | Has reviews and is due today or earlier. |
| `mature` | Not due, and stability is at least `mature_days` (default 21). |
| `young` | Not due, and stability is below `mature_days`. |
| `leech` | At least `leech_lapses` (default 8) lapses since the last reset. Can also be `due`, `mature` or `young`. |

`leech` overlaps with the others; the other four are exclusive.

## What scheduling ignores

- Item content and edits.
- Who recorded the review, and its `response`.

Both are recorded in the log, so they can be analyzed later without having
changed any scheduling.
