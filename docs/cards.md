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
  adds the call, `describe`, `validate`, `groups` or `make` (see below),
  as the last argument. It is a list,
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

#### Calls

A kind program answers four calls, and **each call does one thing**:

| Call | Does | Rule |
|---|---|---|
| `describe` | Says what the kind is for and which fields it takes (see "Describing a kind" below). | **Only informs.** Called only by `srl kinds`. |
| `validate` | Says whether each item is valid and, if not, why. | **The kind's only check, and deterministic**: an item is invalid only if `validate` says so, so it must reject every item the program can't use; the same item always gets the same answer. Called for the items of the kind whose answer isn't kept (see "How srl calls them"). |
| `groups` | Gives the card suffixes of each valid item, in groups of siblings. | **Deterministic, and never checks**: the same fields always give the same cards. Called only for the items `validate` accepted whose answer isn't kept. |
| `make` | Makes each card's content. | **May be slow, and may differ every time.** Called only for cards about to be shown or printed. |

Every call is a **batch**, run the same way:

1. `srl` starts the program with the call as its last argument, and
   writes its inputs to standard input, one JSON object per line, then
   closes it. Even a single input is one line, and `srl` never runs a
   call without any.
2. The program reads up to the end of standard input, and prints
   **exactly one JSON object per input line, on one line, in the same
   order**. It may read everything first or answer as it goes. `srl`
   matches each answer to its input by its place. Each answer depends on
   its own input line alone: a call may hold any of a kind's items, in
   any number.
3. It exits with status 0. **A status other than 0 is a failure of the
   whole call**, and what it printed on standard error is the reason. An
   invalid item is an answer, not a failure.

Lines end with `\n` or `\r\n`. Nothing else may go to standard output;
standard error is free for messages.

| Call | Each input line | Each output line |
|---|---|---|
| `describe` | `{}`, which the program may ignore | `{"description": "…", "fields": [{"name": "VERB", "description": "…"}]}` (see "Describing a kind") |
| `validate` | the item: `{"id": "v81k3z", "fields": {"VERB": "動く", "FORMS": "te past negative"}}` | `{"errors": []}` for a valid item; otherwise its errors, each with a `message` and the `field` it is about, or `null` (or no `field`) when it is about the whole item: `{"errors": [{"field": "FORMS", "message": "\`tense\` is not a form; the forms are te, past, negative"}]}` |
| `groups` | the item, as for `validate` | its cards, in groups: `{"groups": [["te", "past"], ["negative"]]}` |
| `make` | the item, plus `"suffix"` (`null` for a card with the item's ID), `"context"` (the item's context in mode `prose`, as in `srl show --json`) and `"day"` (the study day, such as `"2026-10-27"`) | the card's fields of "Format" above, without `id`: `{"prompt": "…", "prompt_revealed": "…", "answer": "…"}`, where only `prompt` must be present, and `prompt_revealed` and `answer` may be left out, which is the same as `null`; or, when this card can't be made, `{"error": "…"}` |

`fields` is as in `srl show --json` (see [notes.md](notes.md), "Items as
JSON"). No key is written twice in an item, whatever its kind (see
[notes.md](notes.md), "Errors"), so the object loses nothing. An error's
`message` and `field` follow the rules of messages in [cli.md](cli.md):
a plain sentence that says what to do; `field` is a field name, made as
in [notes.md](notes.md), "The item syntax".

**`groups` never gets the context or the day**, so that card IDs depend
on the item's fields alone (see [rationale.md](rationale.md), "Why
`groups` sees only the fields"). `make` gets both: the context is always
in mode `prose`, whatever mode a frontend asks for, so a card doesn't
depend on the frontend that shows it.

**Groups decide siblings.** The cards of one group are siblings (see
[scheduling.md](scheduling.md), "Siblings"). A kind whose cards give
each other away puts them in one group, as `qa` does; a kind whose cards
are independent gives each card its own group. Groups have no names:
they are only how the cards are written, so a kind can change them
without touching any card's history. The output of `groups` holds:

- at least one group, and no group without cards;
- each suffix once, made as in "Card IDs" above;
- `null` in place of a suffix only for a single card with the item's ID:
  `{"groups": [[null]]}`.

Example: a conjugation kind.

```markdown
<!-- srl id:v81k3z kind:conj -->
VERB: 動く
FORMS: te past negative
```

`validate` accepts the item, and `groups` gives cards `v81k3z-te`,
`v81k3z-past` and `v81k3z-negative`, the first two in one group: 動いて
and 動いた share their stem, so seeing one gives the other away. `make`
with suffix `te` gives the て-form question for 動く. With `FORMS: te
tense`, `validate` gives an error about `FORMS`, and `groups` is never
asked about the item.

#### How srl calls them

- **Reading items**, on every command that reads them: one `validate`
  call per kind program, for all of its items in the store whose answer
  isn't kept (or, for `srl add` and `srl edit ID --stdin`, all the items
  given), then one `groups` call for the items it accepted whose answer
  isn't kept. So a command starts at most two processes per kind
  program, however many items it has, and none when every answer is
  kept.
- **Kept answers.** The answers of `validate` and `groups` calls are kept
  in the index cache (see [store.md](store.md), "The index cache"),
  each under the kind's command as written in `[kinds]`, the call, and
  the item's input line. An item whose input line hasn't changed since
  it was answered is not asked about again. Only the answers of a call
  that succeeded are kept, including answers that say an item is
  invalid or that don't follow the rules; a call that fails as a whole
  keeps nothing, and its items are asked about again next time.
  `srl check` asks about every item, and keeps the new answers.
- **After changing a kind program**, without changing its command in
  `[kinds]`, run `srl reindex`, which forgets every kept answer, or
  `srl check`. `srl` can't see that a program changed: the answers it
  kept come from the old program until then.
- **Making cards**: `srl next` makes its one card with a `make` call of
  one line; `srl cards` makes the cards it prints with one `make` call per
  kind program. A `make` call with N lines is stopped after N times
  `kind_programs.card_timeout` seconds, and fails.
- **Describing**: `srl kinds` makes one `describe` call per kind program.

When something goes wrong, the place it happens says whose fault it is:

- **An error `validate` gives is an error in the note**, like a `qa` item
  without `Q`, reported at the item's line, as `PATH:LINE: FIELD:
  message`, or `PATH:LINE: message` when it is about no field.
- **An answer about one item that doesn't follow the rules above**, from
  `validate` or `groups`, is an error in the note at that item, saying
  that the program printed invalid output.
- **A call that fails as a whole** (a status other than 0, a program
  that can't be found or started, or a number of lines that isn't the
  number of input lines) **is a problem of the program**, not of its
  items: it is reported once, and no item of the kind is read until it is
  fixed. Like an error in a note, it stops every command that reads
  items (see [notes.md](notes.md), "Errors").
- **A card that can't be made fails the command**, whether `make` gave
  `{"error": …}` for it, failed as a whole, took too long, or printed
  invalid output for it. `srl next` fails with the reason and prints no
  card; it doesn't go on to another card. `srl cards` fails at once,
  with the reason and no output for any ID (see [cli.md](cli.md)). Until
  what failed is fixed (an LLM API that is down, a missing key),
  `srl next` keeps failing on that card; `srl suspend` sets it aside
  (see [rationale.md](rationale.md), "Why a card that can't be made
  stops studying").
- **A failing `describe` call fails `srl kinds`**, at once, with the
  reason, and so does output that doesn't follow "Describing a kind"
  below. No other command calls it.

`srl cards` makes every card it prints, so a kind whose content differs
every time gives a new card each time: what `srl cards` prints is one
card the kind could give, not the one `srl next` will show.

> **TODO (design):** starting a process for every call is **the only way
> to talk to a kind program**. Other ways could be offered, for the
> program's author to choose one in `[kinds]`, each with the same four
> calls and the same answers; for example an HTTP server that a daemon
> of the store keeps running for a while, so that a program starts once
> instead of once per call. Which ways, and how a store says which one a
> kind uses, is undecided (see [ideas.md](ideas.md), "Kind programs").
> Since answers are kept, only a program that is slow to start (a large
> dictionary, a local model) would gain much, mostly in `make`; this
> waits until kept answers turn out not to be enough.

### Describing a kind

Every kind describes itself, so that a person, an editor or an agent
can write an item of a kind without reading its program (see
[rationale.md](rationale.md), "Why a kind describes itself, and the
description only informs"). `srl kinds` prints the descriptions (see
[cli.md](cli.md)); a kind program gives its own with `describe`, and
`qa`'s is built in.

```json
{"description": "Conjugation questions about a verb, one card per form.",
 "fields": [{"name": "VERB", "description": "The verb, in its dictionary form."},
            {"name": "FORMS", "description": "The forms to ask, separated by spaces: te, past, negative."}]}
```

- `description` says what the kind is for, in a sentence.
- `fields` lists the fields the kind takes, in the order an item usually
  writes them, each with its `name` and a `description`. Whatever an item
  needs to know about a field goes in its description, such as when it
  may be left out: "The answer. Optional when Q has cloze deletions."
- The output holds a description that isn't blank, at least one field,
  each field once with a name made as in [notes.md](notes.md), "The item
  syntax", and descriptions that aren't blank. Any other key is invalid.

**A description only informs.** It doesn't say which fields are required
or how they depend on each other, and `srl` never checks an item against
it: the kind checks its items, `qa` in `srl` and a program in
`validate`. So an editor shows the fields of
`describe`, and learns whether an item is valid from `srl add --dry-run`.
Rules such as "A may be left out when Q has cloze deletions" are written
once, in the kind, and a description that falls behind them misleads
people but never accepts or refuses an item.

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
