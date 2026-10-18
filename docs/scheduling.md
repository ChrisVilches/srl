# Scheduling

`srl` schedules **atoms**, not cards. Every card of an atom shares the atom's
one memory state, and every rating goes to the atom, whichever card produced
it.

> **TODO (design):** whether different directions of the same atom (such as
> `reveal` and `reverse`) should get separate memory states is undecided
> (see [overview.md](overview.md), "Open questions").

## FSRS, computed from the log

The scheduler is FSRS. An atom's memory state (stability, difficulty, last
review, due day) is never stored as truth. It is computed by replaying the
atom's review events from the log, in order, from the last `reset` event
onwards.

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
  atom is due on the review's study day plus the interval.
- A **lapse** is an Again (1) on an atom that already had a review since its
  last reset.
- A `reset` event clears everything except which cards were used, which
  only decides the card rotation (see below).

Weights in `srl.toml` are given to FSRS as they are. It takes 21 (FSRS-6),
or 19 or 17 from older versions, and fills in the rest. Any other count is an
error.

Consequences:

- Changing `desired_retention` or the FSRS weights in `srl.toml` takes effect
  for the whole history at once.
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
the study day fail and say so, rather than assume UTC.

- Every interval is at least one day. A card rated Again is due again the next
  study day.
- There are no learning steps within a day.
- Once an atom is rated, it is not due again the same day, so `srl next` will
  not return it again. Nothing else is needed to keep an atom from repeating
  within a session.

## Choosing the next card: `srl next SELECTOR`

1. **Due atoms first.** Active atoms matching the selector whose due day is
   today or earlier, ordered by due day (oldest first), then by ID.
2. **Then new atoms**, if today's new-atom limit is not reached. Active atoms
   matching the selector that have no reviews (since their last reset),
   ordered by file path.
3. **One card for the chosen atom:** among the cards of every applicable
   drill, the one whose `ref` was used least recently for this atom according
   to the log. Cards never used come first. Ties are broken by the fixed drill
   order `reveal`, `reverse`, `cloze`, `cards`, `confusable`, then
   other drills alphabetically, then by `ref`. `--drill NAME` restricts the
   pool to one drill.
4. **An atom with no card is skipped**, and the next atom is tried. This
   happens with `--drill NAME` on an atom that drill doesn't apply to, or
   when every drill of the atom fails.
5. If no atom qualifies, `srl next` prints nothing and exits with status 1.

`--drill NAME` must name a drill that exists (see `srl drills`).

### The new-atom limit

`new_per_day` (default 10) is one global number: the count of atoms whose first
review since their last reset happened today, across all selectors. When it is
reached, `srl next` only returns due atoms until the next study day.

## Reviews outside the schedule

Reviewing an atom that is not due (for example by studying a prepared session
with `srl study --session`) is allowed. The review is recorded like any other, and FSRS
treats it as an early review.

## Cards about several atoms

A card can list several atoms in `atoms` (a sentence using two grammar points,
a brain dump about a topic). Its rating is written as **one review event per
atom, all with the same rating**. There is no partial credit.

> **TODO (design):** whether a review should also give partial credit to the
> atoms an atom requires (`requires:`) is undecided; it would need a new
> scheduling rule (see [overview.md](overview.md), "Open questions", and
> [ideas.md](ideas.md), "Knowledge structure").

## Reset

`srl reset SELECTOR` appends a `reset` event for each matching atom that is
not new. A new atom has nothing to reset, so it gets no event. From then on,
replay starts empty: the atom is new again. Earlier events stay in the log
for analysis.

Editing an atom's content never resets it.

## States

Every atom has one derived state, usable in selectors as `state:NAME`:

| State | Meaning |
|---|---|
| `new` | No reviews since the last reset. |
| `due` | Has reviews and is due today or earlier. |
| `mature` | Not due, and stability is at least `mature_days` (default 21). |
| `young` | Not due, and stability is below `mature_days`. |
| `leech` | At least `leech_lapses` (default 8) lapses since the last reset. A lapse is an Again on an atom that was not new. Can also be `due`, `mature` or `young`. |

`leech` overlaps with the others; the other four are exclusive.

## What scheduling ignores

- Atom content and edits.
- Which drill or card produced a review (except that `choice` reviews cannot
  be Easy; see [grading.md](grading.md)).
- Who graded the review.

All of these are recorded in the log, so they can be analyzed later without
having changed any scheduling.
