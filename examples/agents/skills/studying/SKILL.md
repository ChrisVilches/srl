---
name: studying
description: Run study sessions with srl, through a frontend such as srl web or srl tui, or in the chat with srl next and srl review, choose ratings, and understand which card comes next and why (study days, new-card limit, siblings). Load when the person wants to study, asks how to rate, asks why a card did or didn't appear, or wants you to quiz them.
---

# Studying

## With a frontend

`srl` has no built-in study command. A **frontend** shows the cards and
records the ratings, and the person runs it themselves: it is interactive,
so you can't run it for them. The `srl` repository's `examples/` has two:

| Frontend | Where | Needs |
|---|---|---|
| `srl web [SELECTOR]` | the browser, with Markdown rendered | Node.js; installed with npm from `examples/web-frontend/` |
| `srl tui [SELECTOR]` | the terminal, with Markdown rendered | uv; installed with `uv tool install` from `examples/python-frontend/` |

```sh
srl web                     # every active card
srl web in:grammar          # only notes in a folder named grammar
```

For each card, space shows the answer, then the context (the note around
the item, or the one its `see:` names); `1`–`4` records the rating; `s`
suspends the card without rating it. Each keeps a count of the cards
reviewed, the new ones and the Again ratings. Nothing is lost by quitting:
every rating is recorded as it is given.

Both are examples, made to be copied and changed. A person who wants
something else can have you write a frontend of their own: see "A study
loop in a script" below.

## Ratings

| Rating | Name | When |
|---|---|---|
| 1 | Again | Didn't know it, or got it wrong. |
| 2 | Hard | Got it, with real effort or hesitation. |
| 3 | Good | Got it. The usual rating. |
| 4 | Easy | Got it instantly, with no effort. |

Advise honesty over optimism: Again on a forgotten card brings it back
tomorrow, which is the point. Good is the normal answer, not a compromise.

## Which card comes next

`srl next [SELECTOR]` (and so every frontend) picks, among active cards:

1. **Due cards** first, the most overdue first.
2. **Then new cards**, in the order they are written in the notes, until
   `new_per_day` (default 10) new cards were started today. The limit is
   for the whole store, not per selector.

Things that explain "why didn't my card show up?":

- **Days, not minutes.** Every interval is at least one day; there are no
  repeats within a session. A card rated Again is due the next study day.
- **The study day starts at 4:00** local time (`day_starts_at`), so a
  late-night session counts as the day before.
- **Siblings.** The cards of one item (its cloze numbers) are shown at most
  one per day, so one doesn't give away another. New cloze cards of an item
  are introduced one per day.
- **Drafts and suspended cards are never studied.** `srl drafts` lists
  drafts; `srl find status:suspended` lists suspended cards.
- **Errors stop everything.** A broken note makes `srl next` fail with the
  file and line; see the `fixing-problems` skill.
- `srl next` exits with 1 and prints nothing when nothing is left today.

To study more new cards today, the person can raise `new_per_day` in
`.srl/config.toml`.

## In the chat, with you

A session in the chat records the same reviews as a frontend, marked as
yours. Only run one when the person asks for it. Each round:

1. `srl next [SELECTOR] --json` prints the card, with its state and
   context: `{"id": "k3m9q1-2", "prompt": "...", "answer": "...",
   "state": "due", "path": "...", "see": null, "context": "..."}`. Exit
   code 1 means nothing is left: say so and stop.
2. Show the prompt, and wait for the person's answer or for them to ask for
   the answer.
3. Show the answer, then the context (`null` when there is none). Say
   which note it comes from: `see`, or `path` when `see` is `null`.
4. Get the rating. Either the person gives it, or, if they asked you to
   judge, you compare their answer with the card's and decide, telling them
   the rating and why.
5. Record it:

   ```sh
   srl review k3m9q1-2 --rating 3 --by agent
   ```

   To keep what they answered, use JSON instead:

   ```sh
   srl review --stdin <<'JSON'
   {"card": "k3m9q1-2", "rating": 3, "response": "彼女", "by": "agent"}
   JSON
   ```

Then go back to step 1. `==text==` in a card marks the important part:
render it as bold, or leave it as written. A review needs a card ID such as
`k3m9q1-2`, not an item ID such as `k3m9q1`.

Useful extras while tutoring: when the person gets something wrong, ask a
follow-up question to find the misunderstanding, and offer to write a draft
item about it (see the `writing-items` skill).

A quiz that should not touch the schedule needs no `srl review` at all:
read cards with `srl cards ID` or `srl find`, and ask.

## Other study actions

| Command | Does |
|---|---|
| `srl review ID --rating N` | Rate a card outside a session, even if not due (treated as an early review). |
| `srl suspend SEL` / `srl unsuspend SEL` | Stop / resume studying cards; the note doesn't change. |
| `srl reset SEL` | Make cards new again. Their history stays in the log. |

`--dry-run` shows what `srl review` or `srl reset` would write.

## A study loop in a script

The loop every frontend runs, written with the commands, for a person who
wants their own frontend (bash with `jq`):

```sh
while card=$(srl next in:grammar --json); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r '.answer, "---", .context // empty' <<<"$card"; read -r -p 'rating 1-4: ' r
  srl review "$(jq -r .id <<<"$card")" --rating "$r"
done
```
