# The frontend contract

A **frontend** is a program that shows cards to a person and records the
ratings. `srl` has none built in (see [rationale.md](rationale.md), "Why no
built-in frontend"): every frontend is a separate program, and all of them
meet `srl` at one fixed contract, described here.

The contract is `srl next --json`, the one mapping from `srl`'s data to
what a frontend shows: it turns the store into one card ready to show.
Frontends understand this and nothing else. They don't know about sources,
kinds or cloze syntax, so any frontend can show any card, and a change to
how items are kept or to a kind never changes a frontend. A kind reaches a frontend
only as its name, a label to show.

Recording the rating is not part of the contract. `srl review` takes only
a card ID and a rating, which belong to the log, not to what a frontend
shows, so a frontend records it with that ordinary command, as a script or
a person does.

How to write a frontend, and the ones in the repository's `examples/`, are
in [guides/frontends.md](guides/frontends.md).

## The loop

Every frontend runs the same loop, around the contract:

1. **Get a card:** `srl next SELECTOR --json`. It prints the
   card (see "What a frontend gets" below); exit 1 means nothing is left,
   and exit 2 is an error, with its reason on standard error, such as a
   card its kind couldn't make (see [cli.md](cli.md), "Studying"). How
   to show an error is up to the frontend.
2. **Show the prompt**, and wait until the person wants to see the answer.
3. **Show the answer:** `prompt_revealed` in place of `prompt`, or
   `prompt` again when `prompt_revealed` is `null`; then `answer` and the
   context, each if it is not `null`. The note the context comes from is
   `see`; a frontend may name it.
4. **Get a rating** from 1 to 4.
5. **Record it:** `srl review ID --rating N`, with the card's ID and the
   rating (see [cli.md](cli.md), "Studying").

The person rates every card; nothing is judged automatically.

A frontend may name the card's `kind` at any step, so that the person
knows where the card came from, such as a kind whose program draws a new
question each time, like an arithmetic drill.

A frontend may offer more, with the other commands everyone has: suspending the
card (`srl suspend id:ID`, with `--reason TEXT` if the person says why,
so that they can fix the card later), opening the note its item names
(`see`, relative to the store's folder, in a program of its choice), or
a summary at the end, counting the cards whose `schedule` was `new`.

A frontend never computes schedules. That belongs to `srl`, so every
frontend behaves the same. Since `srl next` reads every source and the
log every time, edits and reviews made elsewhere count at once.

## What a frontend gets

`srl next --json` prints one object: the card's fields (see
[cards.md](cards.md)), then where the card is in its schedule, whether it
is a leech, and its item's kind, collection and
context:

```json
{"id": "k3m9q1.2", "prompt": "周囲の反対をものともせず、==[who]==は起業した。",
 "prompt_revealed": "周囲の反対をものともせず、==彼女==は起業した。",
 "answer": "Despite everyone's opposition, she started a company.",
 "schedule": "due", "leech": false, "kind": "qa",
 "collection": "japanese/grammar.jsonl", "source": "items",
 "see": "japanese/grammar/wo-mono-tomo-sezu.md",
 "context": "# 〜をものともせず\n\nUndeterred by (adversity), braving. …\n"}
```

| Field | Meaning |
|---|---|
| `id`, `prompt`, `prompt_revealed`, `answer` | The card (see [cards.md](cards.md)). `prompt_revealed` and `answer` may be `null`. |
| `schedule` | `new` or `due`, since `srl next` only gives cards that are due or new (see [scheduling.md](scheduling.md), "Schedule" and "Choosing the next card"). |
| `leech` | Whether the card is a leech, `true` or `false` (see [scheduling.md](scheduling.md), "Leeches"): a frontend may say so, as the question likely needs rewriting. |
| `kind` | The item's kind: `qa`, or the name of a kind program in `.srl/config.toml`, as in `srl show --json` (see [cards.md](cards.md), "Kinds"). |
| `collection` | The collection the item is in, as in `srl show --json` (see [items.md](items.md), "The item object"). |
| `source` | The name of the item's source (see [sources.md](sources.md)). |
| `see` | The path of the note the item names as its context, relative to the store's folder, or `null` (see "Rendering"). |
| `context` | The note's text, or `null` when there is none: no `see`, `--no-context`, or a note that isn't UTF-8 or holds nothing but blank space (see [items.md](items.md), "Context"). |

**The frontend chooses whether to get the context**: `--no-context`
leaves it out, without reading the note, for a frontend that never shows
it or reads the file itself.

## Rendering

Card text is Markdown, and may also hold `==highlight==` (see
[cards.md](cards.md), "Text formatting"). **`srl` doesn't say what the
context's text is: the frontend decides how to render it from `see`**,
the path of the note it comes from (see [items.md](items.md), "Context"),
for example by its extension: Markdown for `.md`, Quarto for `.qmd`,
plain text for `.txt`. The text is the note as it is, whole: notes hold
no items, so nothing is removed from them.

A frontend renders the contexts it supports as it likes, and **shows any
other context as plain text**, with whatever fallbacks it wants, such as for
a file whose content doesn't match its extension. A note `srl` gives no
text for, such as an image or a PDF, may be read by the frontend itself
from the path in `see`, or only named. Rendering only Markdown, or nothing
at all, is fine.

- A note is a file as you wrote it, so it may use syntax beyond
  CommonMark that notes commonly have, such as GFM tables. A frontend may
  render it.
- How highlighted text looks is a frontend setting, never card content.
- Printing the raw text must always remain acceptable. A simple frontend can
  skip rendering entirely.
