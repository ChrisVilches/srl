# Cards

A **card** is one question, ready to show. An item gives one or more cards,
and every card has the same format, so any frontend can show any card.
Cards are produced from the item whenever they are needed, and are never
stored. `srl cards` prints them, and frontends get them inside
`srl next --json`, together with what is shown around them (see
[frontends.md](frontends.md)).

The card is also the unit that gets scheduled: every card has its own
memory state (see [scheduling.md](scheduling.md)).

## Format

```json
{"id": "k3m9q1-2",
 "prompt": "周囲の反対をものともせず、==[who]==は起業した。",
 "prompt_revealed": "周囲の反対をものともせず、==彼女==は起業した。",
 "answer": "Despite everyone's opposition, she started a company."}
```

| Field | Meaning |
|---|---|
| `id` | The card ID (see "Card IDs"). |
| `prompt` | The question, as shown before the answer. |
| `prompt_revealed` | The question as shown with the answer, in place of `prompt`. It is the same as `prompt` unless the question hides part of itself, as a cloze deletion does. |
| `answer` | The answer, or `null` when there is nothing to show beyond `prompt_revealed`. |

Every field is always present. You always rate a card yourself (see
[scheduling.md](scheduling.md), "Ratings"): a frontend shows `prompt`, then
`prompt_revealed` in its place, the answer and the item's context, and
records the rating you give.

## Rules

1. **The prompt never contains the answer.** `prompt` must be safe to
   display as-is, by the simplest frontend, in a log viewer or with `jq`.
   `prompt_revealed` and `answer` are both shown only with the answer.
2. **Cards describe meaning, frontends decide appearance.** A card can say
   "this part is important"; it never says "this part is yellow".
3. **The format is extended for everyone or not at all.** If a kind needs
   something the format cannot express, the format gets a new field, and
   frontends learn it.

## Card IDs

A **card ID** is the ID of its item, alone or followed by `-` and a
**suffix** made of lowercase letters `a`–`z` and digits:

- An item that gives one card gives it the item's ID: `x7k2m9`.
- An item that gives several cards gives each a suffix: `k3m9q1-1`,
  `k3m9q1-2`. Which suffixes is up to the kind.

Item IDs never contain `-`, so the part before the first `-` is always the
item, and no two items can produce the same card ID.

**Card IDs are ordered** by item ID, then by suffix: a card without a
suffix first, then suffixes that are whole numbers, compared as numbers,
then any other suffix as text. So `k3m9q1-2` comes before `k3m9q1-10`.

A kind takes its suffixes from what you wrote, never from positions, so
that adding, removing or reordering parts of an item doesn't give one
card's history to another. When a card disappears anyway, because you
changed the kind or a cloze number, its history stays in the log under its
old ID; `srl check` reports it, and `srl move` gives it to another card
(see [scheduling.md](scheduling.md), "Moving history").

## Kinds

An item's **kind**, set by `kind:` in its header, decides which fields it
takes and how they become cards. There is one kind, `qa`, and it is the
default. Any other `kind:` is an error.

> **TODO (design):** other kinds, possibly as programs, are not specified
> yet (see [overview.md](overview.md), "Open questions", and
> [ideas.md](ideas.md), "Kinds").

### `qa`

Fields:

| Field | Required | Meaning |
|---|---|---|
| `Q` | yes | The question. May contain cloze deletions. |
| `A` | unless `Q` has cloze deletions | The answer. |

Each field may appear once, and any other field is an error.

**Without cloze deletions**, the item gives one card, with the item's ID:
the prompt and the revealed prompt are `Q`, and the answer is `A`.

```markdown
<!-- srl id:x7k2m9 -->
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.
```

```json
{"id": "x7k2m9", "prompt": "What does 〜をものともせず mean?",
 "prompt_revealed": "What does 〜をものともせず mean?",
 "answer": "Undeterred by (adversity), braving."}
```

**With cloze deletions**, the item gives one card per cloze number, as in
Anki.

### Cloze deletions

A **cloze deletion** is written in `Q` exactly as in Anki:

- `{{cN::text}}` hides `text` in the card of number `N`.
- `{{cN::text::hint}}` does the same, and shows `hint` in its place.
- `N` is a whole number from 1 up, written without leading zeros (`c1`,
  `c12`; not `c0` or `c01`).

The rules:

1. **One card per number.** Deletions with the same number are hidden
   together, in the same card. The card's suffix is the number: deletions
   `c1` and `c2` in item `k3m9q1` give cards `k3m9q1-1` and `k3m9q1-2`.
2. **The prompt** is `Q` with every deletion of the card's number replaced
   by `==[...]==`, or by `==[hint]==` if it has a hint, and every other
   deletion replaced by its text.
3. **The revealed prompt** is `Q` with every deletion of the card's number
   replaced by `==text==`, and every other deletion by its text.
4. **The answer** is `A`, or `null` if the item has no `A`.
5. **Only `{{c` followed by a number and `::` starts a deletion.** Any other
   `{{`, such as in code (`format!("{{}}")`), is ordinary text.
6. **These are errors:** a deletion without its closing `}}`, a deletion
   with empty text, a deletion inside another, a number with leading zeros
   or `c0`, and deletions in `A`.

```markdown
<!-- srl id:k3m9q1 -->
Q: {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。
A: Despite everyone's opposition, she started a company.
```

gives two cards:

```json
{"id": "k3m9q1-1",
 "prompt": "==[...]==を==[...]==、彼女は起業した。",
 "prompt_revealed": "==周囲の反対==を==ものともせず==、彼女は起業した。",
 "answer": "Despite everyone's opposition, she started a company."}
{"id": "k3m9q1-2",
 "prompt": "周囲の反対をものともせず、==[who]==は起業した。",
 "prompt_revealed": "周囲の反対をものともせず、==彼女==は起業した。",
 "answer": "Despite everyone's opposition, she started a company."}
```

Without the `A:` line, both cards would have `"answer": null`: the
revealed prompt is all there is to show.

The cards of one item are **siblings**: each is scheduled on its own, and
`srl next` shows at most one of them per day (see
[scheduling.md](scheduling.md), "Siblings").

## Text formatting

The text fields `prompt`, `prompt_revealed` and `answer` are **CommonMark
Markdown plus one extension: `==highlight==`.**

- `==…==` marks the important part: the blank in a cloze prompt, the
  revealed text in its revealed prompt.
- Each frontend decides how to render it: bold yellow in a terminal, `<mark>`
  with CSS in a browser, or the raw `==…==` in a plain frontend.
- Printing the raw text must always be an acceptable rendering. A frontend may
  render less than Markdown offers, but never needs more.

The text of `Q` and `A` is copied into cards as written, so a `==…==` you
write in a field is a highlight in the card too.

> **TODO (design):** cards can't carry media (audio, images) or furigana
> yet; how to add them without breaking "raw text is always acceptable" is
> undecided (see [overview.md](overview.md), "Open questions").
