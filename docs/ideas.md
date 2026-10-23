# Ideas

Mechanics that are not part of the specification yet. They are recorded here
so they are not lost, not as promises. Most can be built on top of the
current design, as agent workflows or frontends, without changing the core;
the ones that would change the core say so. An idea becomes a design
decision to take, listed in [overview.md](overview.md), "Open questions",
once it is planned; kinds as plugins is one.

## Kinds

### Kinds as plugins

A header with `kind:NAME` would run a program `srl-kind-NAME`. Would
change the core: finding programs, running them, and reporting their
failures.

**Card IDs and card content come from two separate calls.** Every command
that reads items needs their card IDs: selectors, `srl find`, `srl check`,
scheduling, the sibling rule. Only a few need a card's content: `srl next`
for the one card it returns, `srl cards`, and `srl study`. So a kind
program answers two questions:

| Call | Input | Output | Rule |
|---|---|---|---|
| `srl-kind-NAME ids` | the item's fields, as JSON | the card suffixes, one per line, or one empty line for a single card with the item's ID | **Deterministic and fast**: the same fields always give the same IDs. Called for every item of the kind, on every command that reads items, so its result would need a cache: notes themselves are not cached (see [store.md](store.md), "The index cache"). |
| `srl-kind-NAME card` | the item's fields, the card's suffix, its context and the study day, as JSON | one card's `prompt` and `answer`, as JSON | **May differ every time.** Called only when a card is about to be shown. |

Example: a conjugation kind.

```markdown
<!-- srl id:v81k3z kind:conj -->
VERB: 動く
FORMS: te past negative
```

`ids` prints `te`, `past` and `negative`, giving cards `v81k3z-te`,
`v81k3z-past` and `v81k3z-negative`. `card` with suffix `te` prints the
て-form question for 動く.

> **TODO (design):** the wish is a kind program with **one call**, not two.
> It is not known yet whether that is possible: card IDs are needed far
> more often than card content, and must be stable, while content may
> differ every time. Getting there would need a refactor and
> restructuring of how `srl` asks a kind for cards, for example around
> when IDs are computed and cached. To be worked on when plugin support is
> implemented; until then, the two calls above are the design that is
> known to work.

Other rules a plugin design would need:

- **Fields are checked by `ids`.** A program that exits with an error on
  `ids` makes the item an error in its note, like a `qa` item with a
  missing `Q`.
- **A failing `card` call doesn't stop everything.** An LLM API can be
  down. `srl next` reports the failure and goes on to the next card, as
  if the failed card were not eligible today.
- **A time limit** for both calls, set in `.srl/config.toml`.
- **`srl add` needs no change**: it takes field names from JSON, and
  `ids` checks them.

### Proof of concept: questions written by an LLM

A kind whose card is written anew by an LLM every time it is shown:

```markdown
<!-- srl id:q8w2e5 kind:llm-generated -->
PROMPT: Ask me to use 〜をものともせず in a new sentence about everyday
  work. The answer is a model sentence and its translation.
```

- `srl-kind-llm-generated ids` checks that the item has exactly one field,
  `PROMPT`, and prints one empty line: one card, with the item's ID
  `q8w2e5`. No call to the LLM.
- `srl-kind-llm-generated card` sends `PROMPT`, and the item's context
  (the note about 〜をものともせず), to an LLM API, and prints the question
  and the answer it wrote. A new card each time, under the same ID.

Each part of the design already allows this:

- **Scheduling** sees one card, `q8w2e5`, with one memory state: how well
  you can use 〜をものともせず, asked a different way each time. Different
  content under one ID is the kind's choice; `srl` never compares content.
- **Prompt and answer match**, because they come out of the same `card`
  call, and `srl next --json` gives both at once. A frontend holds the
  card it got; it never asks for it again.
- **Reviews record only the card ID** (and `hash`, of the item's text), so
  a card that changes doesn't affect the log. What was actually asked is
  not in the log. A frontend that wants it can pass it in `response`, or
  the program can keep its own record.
- **Cost:** one LLM call per card shown, and none for every other command,
  because `ids` doesn't call the LLM. The program may cache answers by
  item ID and study day, so the card stays the same for a day if you
  prefer that.

## Agent workflows

- **Brain-dump recall.** "Write everything you know about Rust ownership." An
  LLM compares the answer with the items of that topic and records a review
  for each card you recalled or missed. One prompt reviews many cards.
- **Socratic repair of leeches.** After repeated lapses, a short dialogue looks
  for where your understanding breaks, then proposes a rewrite of the item or a
  new draft item for the misconception.
- **Error classification.** On a wrong answer, an LLM labels the error and a
  fixed rule decides what to do: a slip needs nothing, confusion with another
  item adds an item that contrasts the two, a missing prerequisite points to
  the item that should be studied first. Needs the answer you gave, kept as
  the review's `response`.
- **Synthesis questions.** One question that needs two or three due items from
  different topics, written as a draft item of its own.
- **Contextual questions.** Questions set in your own projects and interests,
  using a personal context file.
- **Language practice at "i+1".** Sentences using only vocabulary you already
  know (`srl find in:vocab state:mature`) plus one new item.
- **Teach-back.** You explain a note to a simulated confused student, or write
  the question yourself, and an LLM points out what was missing.
- **Stale questions.** Compare an item with the note it asks about (its
  context) and propose an edit when the note changed.

## Knowledge structure

- **Prerequisites.** An agent extracts prerequisites from notes and records
  them, for example as a header entry naming other items.
- **Partial credit for prerequisites.** A correct answer about a card also
  counts, partially, for the cards it requires. Needs a scheduling rule, so
  it would change the core.

## Studying

- **Confidence calibration.** Rate your confidence before answering, and track
  how well confidence predicts correctness. A frontend feature.
- **Ambient review.** One question at a time in the shell prompt, the tmux
  status line or a git hook.
- **Code exercises checked by tests.** An agent writes an exercise, runs your
  solution in a sandbox and records the rating.

## Stores

- **Export and import between stores.** Move or copy notes from one store to
  another together with the review history of their cards, so that their
  memory state comes along. It needs rules for ID clashes and for merging
  logs. Copying a note by hand moves only the content.
- **Import from Anki.** The cloze syntax is Anki's, so a note type with a
  front, a back and cloze deletions maps to `qa` items directly.

## Analysis

- **How well an agent's judgment agrees with yours**, by having it rate
  answers you also rated yourself.
