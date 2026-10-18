# Scheduling

`srs` schedules **atoms**, not cards. Every card of an atom shares the atom's
one memory state, and every rating goes to the atom, whichever card produced
it.

## FSRS, computed from the log

The scheduler is FSRS. An atom's memory state (stability, difficulty, last
review, due day) is never stored as truth. It is computed by replaying the
atom's review events from the log, in order, from the last `reset` event
onwards.

Consequences:

- Changing `desired_retention` or the FSRS weights in `srs.toml` takes effect
  for the whole history at once.
- Replacing FSRS with another algorithm later only means replaying the same
  log differently.
- The index cache stores the computed state for speed, and can always be
  rebuilt.

## Days, not minutes

Scheduling works in whole **study days**. A study day starts at
`day_starts_at` (default 4:00 local time), so a late-night session counts as
the day it began.

- Every interval is at least one day. A card rated Again is due again the next
  study day.
- There are no learning steps within a day.
- Once an atom is rated, it is not due again the same day, so `srs next` will
  not return it again. Nothing else is needed to keep an atom from repeating
  within a session.

## Choosing the next card: `srs next SELECTOR`

1. **Due atoms first.** Active atoms matching the selector whose due day is
   today or earlier, ordered by due day (oldest first), then by ID.
2. **Then new atoms**, if today's new-atom limit is not reached. Active atoms
   matching the selector that have no reviews (since their last reset),
   ordered by file path.
3. **One card for the chosen atom:** among the cards of every applicable
   drill, the one whose `ref` was used least recently for this atom according
   to the log. Cards never used come first. Ties are broken by the fixed drill
   order `reveal`, `reverse`, `cloze`, `cards`, `confusable`, `generated`, then
   other drills alphabetically, then by `ref`. `--drill NAME` restricts the
   pool to one drill.
4. If no atom qualifies, `srs next` prints nothing and exits with status 1.

### The new-atom limit

`new_per_day` (default 10) is one global number: the count of atoms whose first
review since their last reset happened today, across all selectors. When it is
reached, `srs next` only returns due atoms until the next study day.

## Reviews outside the schedule

Reviewing an atom that is not due (for example by playing a prepared session
with `srs play`) is allowed. The review is recorded like any other, and FSRS
treats it as an early review.

## Cards about several atoms

A card can list several atoms in `atoms` (a sentence using two grammar points,
a brain dump about a topic). Its rating is written as **one review event per
atom, all with the same rating**. There is no partial credit.

## Reset

`srs reset SELECTOR` appends a `reset` event for each matching atom. From then
on, replay starts empty: the atom is new again. Earlier events stay in the log
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
