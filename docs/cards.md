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
| `prompt_revealed` | The question as shown with the answer, in place of `prompt`, or `null` when it is `prompt` itself. It is only needed when the question hides part of itself, as a cloze deletion does. |
| `answer` | The answer, or `null` when there is none: nothing is shown beyond the revealed prompt. |

You always rate a card yourself (see [scheduling.md](scheduling.md),
"Ratings"): a frontend shows `prompt`, then the revealed prompt in its
place, the answer and the item's context, and records the rating you give.

**A field that may be `null` may also not exist, and then it shows its
fallback, or nothing if it has none.** `prompt_revealed` falls back to
`prompt`; `answer` has no fallback, so a card without one shows nothing
after the revealed prompt. Everyone who handles a card follows this:

- **A kind** may leave either field out, or give it as `null`; the two
  mean the same.
- **`srl`** always prints every field, giving `null` for one that doesn't
  exist.
- **A frontend** shows `prompt` again when `prompt_revealed` is `null`,
  and shows nothing for an `answer` that is `null`: no empty box, no
  divider.

So a kind needs to know only `prompt`. A kind whose question hides nothing
never needs `prompt_revealed`, and one that forgets it still gives a usable
card: the question just stays as it was. A kind may even give no answer at
all, for studying where you only see the question and rate how well you
feel you could answer it.

## Rules

1. **The prompt never contains the answer.** `prompt` must be safe to
   display as-is, by the simplest frontend, in a log viewer or with `jq`.
   `prompt_revealed` and `answer` are shown only once the person asks to
   see the answer.
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
takes and how they become cards, and which of its cards are siblings
(see [scheduling.md](scheduling.md), "Siblings"). `qa` is built in and
is the default. Every other kind is a program named in `.srl/config.toml`
(see "Kind programs" below). A `kind:` that is neither is an error.
The selector term `kind:` picks the cards of items of a kind (see
[selectors.md](selectors.md)).

### `qa`

Fields:

| Field | Required | Meaning |
|---|---|---|
| `Q` | yes | The question. May contain cloze deletions. |
| `A` | unless `Q` has cloze deletions | The answer. |

Each field may appear once, and any other field is an error.

**Without cloze deletions**, the item gives one card, with the item's ID:
the prompt is `Q`, `prompt_revealed` is `null`, and the answer is `A`.

```markdown
<!-- srl id:x7k2m9 -->
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.
```

```json
{"id": "x7k2m9", "prompt": "What does 〜をものともせず mean?",
 "prompt_revealed": null,
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

The cards of a `qa` item are all **siblings**, one group: seeing one would
give away another. Each is scheduled on its own, and `srl next` shows at
most one of them per day (see [scheduling.md](scheduling.md),
"Siblings").

### Kind programs

A kind can be a program, named in `.srl/config.toml` (see
[store.md](store.md), "Configuration"):

```toml
[kinds]
ja-words = ["python3", "kinds/ja_words.py"]
```

An item with `kind:ja-words` gets its cards from that program.
[examples/kind-ja-words/](../examples/kind-ja-words/) is one: one card
per `{word}` of a Japanese sentence.

- **The value is a command**: the program, then its arguments. `srl`
  adds `ids` or `card` (see below) as the last argument. It is a list,
  not a path, so that a script runs the same on every platform: Windows
  can't run a script by its `#!` line, and takes
  `["py", "kinds/ja_words.py"]`.
- **A program path with a folder is relative to the store's folder**,
  unless it is absolute, and the program runs in the store's folder, so
  relative paths among its arguments work too. A program named without a
  folder, such as `python3`, is found as the platform finds programs
  (on `PATH`).
- **`qa` can't be configured.**
- **Running a program is your choice.** `srl` runs whatever command
  `[kinds]` names, with your rights, whenever a command reads an item of
  that kind, and neither checks nor limits it, as with plugins in most
  tools. A store you copy from someone else runs their programs: read its
  `[kinds]` first.

**Card IDs and card content come from two separate calls.** Every command
that reads items needs their card IDs: selectors, `srl find`, `srl
check`, scheduling, the sibling rule. Only `srl next`, for the card it
shows, and `srl cards` need a card's content. So a kind program answers
two calls, by design. IDs must be stable, since a card's history is kept
under its ID, and are asked for all the time; content is asked for one
card at a time, and may be slow to make (a program that asks an LLM to
write the question) or differ every time. Kept apart, a command that only
needs IDs never waits for content (see [rationale.md](rationale.md), "Why
a kind program answers two calls"). Each call reads one JSON object on
standard input and prints one on standard output. A call that exits with a status other than 0 failed, and
what it printed on standard error is the reason.

| Call | Input | Output | Rule |
|---|---|---|---|
| `ids` | the item: `{"id": "v81k3z", "fields": {"VERB": "動く", "FORMS": "te past negative"}}` | its cards, in groups: `{"groups": [{"cards": ["te", "past"]}, {"cards": ["negative"]}]}` | **Deterministic and fast**: the same fields always give the same cards. Called for every item of the kind, on every command that reads items. It has no time limit. |
| `card` | the same object, plus `"suffix"` (`null` for a card with the item's ID), `"context"` (the item's context in mode `prose`, as in `srl show --json`) and `"day"` (the study day, such as `"2026-10-27"`) | `{"prompt": "…", "prompt_revealed": "…", "answer": "…"}`: the card's fields of "Format" above, without `id`. Only `prompt` must be present; `prompt_revealed` and `answer` may be left out, which is the same as `null` | **May differ every time.** Called only for a card about to be shown, and stopped after `kind_programs.card_timeout` seconds. |

`fields` is as in `srl show --json` (see [notes.md](notes.md), "Items as
JSON"). No key is written twice in an item, whatever its kind (see
[notes.md](notes.md), "Errors"), so the object loses nothing.

**Groups decide siblings.** The cards of one group are siblings (see
[scheduling.md](scheduling.md), "Siblings"). A kind whose cards give
each other away puts them in one group, as `qa` does; a kind whose cards
are independent gives each card its own group. Groups have no names:
they are only how the cards are written, so a kind can change them
without touching any card's history. The output of `ids` holds:

- at least one group, and no group without cards;
- each suffix once, made as in "Card IDs" above;
- `null` in place of a suffix only for a single card with the item's ID:
  `{"groups": [{"cards": [null]}]}`.

Example: a conjugation kind.

```markdown
<!-- srl id:v81k3z kind:conj -->
VERB: 動く
FORMS: te past negative
```

`ids` gives cards `v81k3z-te`, `v81k3z-past` and `v81k3z-negative`, the
first two in one group: 動いて and 動いた share their stem, so seeing one
gives the other away. `card` with suffix `te` prints the て-form question
for 動く.

When a call fails:

- **A failing `ids` call is an error in the note**, like a `qa` item
  without `Q`: the program checks the fields. So is output that doesn't
  follow the rules above, and a program that can't be found or started.
- **A failing `card` call fails the command.** `srl next` fails with the
  reason and prints no card; it doesn't go on to another card. `srl cards`
  fails at once, with the reason and no output for any ID (see
  [cli.md](cli.md)). A `card` call that takes longer than
  `kind_programs.card_timeout` is stopped, and fails. Until what failed is
  fixed (an LLM API that is down, a missing key), `srl next` keeps
  failing on that card; `srl suspend` sets it aside (see
  [rationale.md](rationale.md), "Why a card that can't be made stops
  studying").

> **TODO (design):** `ids` has two jobs, and its name says only one.
> Besides giving the item's card IDs, it is the only place a kind program
> checks an item's fields: srl knows nothing of a program's fields, so an
> item the program can't use is an error only if `ids` fails on it (this
> is what makes `srl check` and `srl add --dry-run` find a missing or
> misspelled field). Nothing in the call's name, nor in its row of the
> table above, says that it must check, so a program that only computes
> IDs, and accepts any fields, is easy to write by mistake: its broken
> items then pass every check and fail only in `card`, when studying. One
> call doing two unrelated things, one of them implicit, is a code smell.
> Whether to rename the call, split checking into a call of its own, or
> only say it louder here is undecided.

`srl cards` calls `card` for every card it prints, so a kind whose
content differs every time gives a new card each time: what `srl cards`
prints is one card the kind could give, not the one `srl next` will show.

> **TODO (design):** `ids` results are **not cached**: every command
> that reads items starts the program once for every item of the kind,
> so a store with many such items is slow. A cache would make `srl`
> faster without changing what any command does. What it would be keyed
> on, and what would make an entry stale (the item's text, the kind's
> command, the program changing), is still to be analyzed. Notes
> themselves are not cached either (see [store.md](store.md), "The index
> cache").

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
