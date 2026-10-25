# The frontend contract

A **frontend** is a program that shows cards to a person and records the
ratings. `srl` has none built in (see [rationale.md](rationale.md), "Why no
built-in frontend"): every frontend is a separate program, and all of them
meet `srl` at one fixed contract, described here.

The contract is the mapping from `srl`'s data to what a frontend shows,
and back: `srl next --json` turns the store into one card ready to show,
and `srl review` turns a rating into a review event. Frontends understand
this and nothing else. They don't know about notes, kinds or cloze syntax,
so any frontend can show any card, and a change to the notes or to a kind
never changes a frontend.

How to write a frontend, and the ones in the repository's `examples/`, are
in [guides/frontends.md](guides/frontends.md).

## The loop

Every frontend runs the same loop:

1. **Get a card:** `srl next SELECTOR --json --context MODE`. It prints the
   card (see "What a frontend gets" below); exit 1 means nothing is left,
   and exit 2 is an error, with its reason on standard error, such as a
   card its kind couldn't make (see [cli.md](cli.md), "Studying"). How
   to show an error is up to the frontend.
2. **Show the prompt**, and wait until the person wants to see the answer.
3. **Show the answer:** `prompt_revealed` in place of `prompt`, or
   `prompt` again when `prompt_revealed` is `null`; then `answer` and the
   context, each if it is not `null`. The note the context
   comes from is `see`, or `path` when `see` is `null`; a frontend may name
   it.
4. **Get a rating** from 1 to 4.
5. **Record it:** the card ID and the rating to `srl review --stdin` (see
   [cli.md](cli.md), "Studying").

The person rates every card; nothing is judged automatically. A frontend may
also collect an answer before step 3 and pass it as `response`, which is
only kept in the log.

A frontend may offer more, with the commands everyone has: suspending the
card (`srl suspend id:ID`), opening its note (`srl edit ID`, or the file at
`path` in an editor of its own), or a summary at the end, counting the
cards whose `state` was `new`.

A frontend never computes schedules. That belongs to `srl`, so every
frontend behaves the same. Since `srl next` reads the notes and the log
every time, edits and reviews made elsewhere count at once.

## What a frontend gets

`srl next --json` prints one object: the card's fields (see
[cards.md](cards.md)), then the card's state and its item's note and
context:

```json
{"id": "k3m9q1-2", "prompt": "周囲の反対をものともせず、==[who]==は起業した。",
 "prompt_revealed": "周囲の反対をものともせず、==彼女==は起業した。",
 "answer": "Despite everyone's opposition, she started a company.",
 "state": "due", "path": "japanese/grammar/wo-mono-tomo-sezu.md", "see": null,
 "context": "# 〜をものともせず\n\nUndeterred by (adversity), braving. …\n"}
```

| Field | Meaning |
|---|---|
| `id`, `prompt`, `prompt_revealed`, `answer` | The card (see [cards.md](cards.md)). `prompt_revealed` and `answer` may be `null`. |
| `state` | `new` or `due`, since `srl next` only gives cards that are due or new (see [scheduling.md](scheduling.md), "Choosing the next card"). |
| `path` | The note the item is in, as in `srl show --json` (see [notes.md](notes.md), "Items as JSON"). |
| `see` | The note its `see:` names, or `null`. |
| `context` | The context, in the mode the frontend chose, or `null` when there is none (see [notes.md](notes.md), "Context"). |

**The frontend chooses the context's mode** with `--context`: `prose` (the
default), `full` or `none`. How much of the note to show is a choice about
presentation, so it belongs to the frontend, not to the store.

## Rendering

Card text and context are Markdown; card text may also hold `==highlight==`
(see [cards.md](cards.md), "Text formatting").

- The context is the note as written, so it may use syntax beyond
  CommonMark that notes commonly have, such as GFM tables. A frontend may
  render it.
- How highlighted text looks is a frontend setting, never card content.
- Printing the raw text must always remain acceptable. A simple frontend can
  skip rendering entirely.
