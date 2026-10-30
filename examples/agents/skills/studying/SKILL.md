---
name: studying
description: Run study sessions with srl, through a frontend such as srl-web or srl-tui, or in the chat with srl next and srl review, choose ratings, and understand which card comes next and why (study days, new-card limit, siblings). Load when the person wants to study, asks how to rate, asks why a card did or didn't appear, or wants you to quiz them.
---

# Studying

## With a frontend

`srl` has no built-in study command. A **frontend** shows the cards and
records the ratings, and the person runs it themselves: it is interactive,
so you can't run it for them. The `srl` repository's `examples/` has two:

| Frontend | Where | Needs |
|---|---|---|
| `srl-web [SELECTOR]` | the browser, with Markdown rendered | Node.js; installed with npm from `examples/web-frontend/` |
| `srl-tui [SELECTOR]` | the terminal, with Markdown rendered | uv; installed with `uv tool install` from `examples/python-frontend/` |

```sh
srl-web                     # every active card
srl-web in:grammar          # only notes in a folder named grammar
```

For each card, space or Enter shows the answer, then the context (the
note around the item, or the one its `see` names; none for an item of a
JSONL note without `see`, and a JSONL note named by `see` shown as a code
block); `1`–`4` records the
rating; `s` suspends the card without rating it. In `srl-tui`, the card
and the context scroll in panels of their own: `j`/`k` or the arrows
scroll the one in focus (Ctrl-D/Ctrl-U by half of it, `g`/`G` to its top
and bottom), Tab moves the focus, and `q` quits. Each keeps a count of the
cards reviewed, the new ones and the Again ratings. Nothing is lost by quitting: every rating is recorded
as it is given.

`srl-web` also has a Browse tab, which lists the cards of a selector and
shows any of them as studying would, recording nothing: for seeing how a
card looks, such as one of a kind program, without waiting for it to be
due. Its Add tab writes items one at a time into a note the person picks,
for a person who prefers a form to editing notes (see the writing-items
skill for what an item holds).

Both are meant for Linux, macOS and Windows, but are tried on Linux only;
their READMEs say what may differ elsewhere. In `srl-tui` on Windows, the
arrow keys may not scroll; `j`/`k` do the same.

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

With `max_skip` above 0 in `.srl/config.toml` (default 0), it picks
uniformly among the first `max_skip` + 1 of those, so the order is a
little shuffled; skipped cards come back on later calls.

Things that explain "why didn't my card show up?":

- **Days, not minutes.** Every interval is at least one day; there are no
  repeats within a session. A card rated Again is due the next study day.
- **The study day starts at 4:00** local time (`day_starts_at`), so a
  late-night session counts as the day before.
- **Siblings.** The cloze cards of one `qa` item are siblings: they are shown
  at most one per day, so one doesn't give away another, and new ones are
  introduced one per day. A kind program decides which of its cards are
  siblings; cards that don't give each other away, such as two words of one
  sentence, can all be shown the same day.
- **Drafts and suspended cards are never studied.** `srl find
  status:draft` lists drafts; `srl find status:suspended` lists suspended
  cards.
- **Errors stop everything.** A broken note makes `srl next` fail with the
  file and line; see the `fixing-problems` skill.
- `srl next` exits with 1 and prints nothing when nothing is left today.
- **A card that can't be made stops `srl next`.** If the card it picks
  comes from a kind program that fails (an LLM API that is down, a
  missing key), `srl next` exits with 2 and the reason, every time, and
  shows no other card instead. Fix what failed, or set the card aside with
  `srl suspend id:ID`; see the `fixing-problems` skill.

To study more new cards today, the person can raise `new_per_day` in
`.srl/config.toml`.

## In the chat, with you

In a session in the chat you act as the frontend: you show what
`srl next --json` gives, and record the same reviews as a frontend, marked
as yours. Only run one when the person asks for it. Each round:

1. `srl next [SELECTOR] --json` prints the card, with its state, kind and
   context: `{"id": "k3m9q1.2", "prompt": "...", "prompt_revealed": "...",
   "answer": "...", "state": "due", "kind": "qa", "path": "...",
   "see": null, "context": "...", "context_format": "markdown"}`. Mention the kind when it isn't `qa`:
   it says which program made the card. Exit
   code 1 means nothing is left: say so and stop. Exit code 2 is an
   error: tell the person what it says, and stop. `--context MODE` chooses
   how much of the note comes along: `prose` (the default) without its
   items, `full` with them, `none` nothing. An item of a JSONL note
   without `see` has no context (`null`), and a JSONL note named by `see`
   comes whole, as it is, in `prose` and `full` alike.
2. Show the prompt, and wait for the person's answer or for them to ask for
   the answer.
3. Show the answer: `prompt_revealed` (the prompt with a cloze's blanks
   filled in), or `prompt` again when it is `null`; then `answer`, unless
   it is `null` (there is none: show nothing for it); then the context,
   unless it is `null`. Say which note it comes from: `see`, or `path` when
   `see` is `null`. `context_format` says what the context is: `markdown`
   is the note's Markdown; `jsonl` is a JSONL note, data rather than prose,
   so show it as a code block or say what in it matters. Treat any other
   format as plain text.
4. Get the rating. Either the person gives it, or, if they asked you to
   judge, you compare their answer with the card's and decide, telling them
   the rating and why.
5. Record it:

   ```sh
   srl review k3m9q1.2 --rating 3 --by agent
   ```

   To keep what they answered, use JSON instead:

   ```sh
   srl review --stdin <<'JSON'
   {"card": "k3m9q1.2", "rating": 3, "response": "彼女", "by": "agent"}
   JSON
   ```

Then go back to step 1. `==text==` in a card marks the important part:
render it as bold, or leave it as written. A review needs a card ID such as
`k3m9q1.2`, not an item ID such as `k3m9q1`.

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
  jq -r '.prompt_revealed // .prompt, .answer // empty, "---", .context // empty' <<<"$card"; read -r -p 'rating 1-4: ' r
  srl review "$(jq -r .id <<<"$card")" --rating "$r"
done
```
