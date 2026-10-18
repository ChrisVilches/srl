# Grading

Grading decides whether an answer was right and suggests a rating. The rating
itself is always yours: a frontend shows the suggestion, you accept or change
it, and the result goes to `srl review`.

Grading lives in the core, not in frontends, so that the terminal, the web
viewer and any script judge answers in exactly the same way.

## Grading modes

| Mode | The answer is | Judged by |
|---|---|---|
| `self` | Not entered, or entered but not checked | You. No grading call; you rate directly. |
| `exact` | Typed, one string per blank | `srl`: normalized comparison with `accept` |
| `choice` | One of `choices` | `srl`: comparison with `accept` |
| `cmd` | Free text | The external grader configured in `srl.toml` |

These four are fixed. A drill picks one for each card.

## `srl grade`

A frontend sends the card back together with the answer. The core keeps no
session state, so this works for every card, including cards that cannot be
produced again.

```sh
echo '{"card": {…}, "response": ["周囲の反対", "ものともせず"]}' | srl grade
```

```json
{"correct": [true, true], "suggested_rating": 3, "feedback": null}
```

- `response` is always a list of strings: one per blank for `exact`, one for
  `choice` (the chosen text), one for `cmd`.
- `correct` has one entry per blank.
- `feedback` is Markdown or `null`.
- For `self` cards, `srl grade` returns `{"correct": null, "suggested_rating":
  null, "feedback": null}`.

## Normalization

Before comparing, `exact` and `choice` normalize both the response and every
accepted string:

1. Unicode NFKC (so full-width and half-width forms compare equal).
2. Trim whitespace at both ends.
3. Collapse runs of whitespace into one space.
4. Lowercase.

A blank is correct if its normalized response equals any normalized string in
its `accept` entry. Nothing else is tolerated: no fuzzy matching, no kana/kanji
equivalence. If you want an alternative accepted, list it.

## Suggested rating

| Result | Suggested rating |
|---|---|
| All blanks correct | 3 (Good) |
| Any blank wrong | 1 (Again) |

## Multiple choice can't be rated Easy

Recognizing the right option among a few is easier than producing it. So a
review of a `choice` card is never recorded as Easy: `srl review` stores a 4
as 3. This is the only rule that depends on how a card is graded, and it is
enforced in the core so no frontend can bypass it.

## The external grader

`cmd` cards are for answers no string comparison can judge: a sentence you
wrote, an explanation in your own words. They are graded by a command named in
`srl.toml`:

```toml
[grading]
cmd = "srl-grade-agi"
```

`srl` runs the command, writes JSON on its standard input and reads JSON from
its standard output:

```json
{"card": {…}, "response": ["鰻の値段をものともせず、毎週食べに行く。"],
 "atom": {"id": "a7f3k2", "title": "〜をものともせず", "answer": null, "body": "…"}}
```

```json
{"correct": [true], "rating": 3, "feedback": "Natural. Note the admiring tone fits here."}
```

`srl grade` returns the command's `rating` as `suggested_rating`.

The command can be anything: a script with regular expressions, a call to an
LLM, a program that asks a friend. `srl` neither knows nor cares. If no
command is configured, or the command fails, `srl grade` returns
`{"correct": null, "suggested_rating": null, "feedback": null, "fallback":
"self"}` and the frontend falls back to self-grading. A failing command is
also reported on standard error, so a broken grader is never mistaken for
one that isn't configured.

## Ratings

| Rating | Name | Meaning |
|---|---|---|
| 1 | Again | You did not know it. |
| 2 | Hard | You knew it, with real difficulty. |
| 3 | Good | You knew it. |
| 4 | Easy | You knew it without effort. |
