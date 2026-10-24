# Ideas

Changes to `srl` that are not part of the specification yet. They are
recorded here so they are not lost, not as promises. Each would change
`srl`'s data, commands or contracts; ideas for things built on top of
`srl` as it is are in [guides/ideas.md](guides/ideas.md). An idea becomes a
design decision to take, listed in [overview.md](overview.md), "Open
questions", once it is planned; kinds as plugins is one.

## Kinds

### Kinds as plugins

A header with `kind:NAME` would run a program `srl-kind-NAME`. Would
change the core: finding programs, running them, and reporting their
failures.

**Card IDs and card content come from two separate calls.** Every command
that reads items needs their card IDs: selectors, `srl find`, `srl check`,
scheduling, the sibling rule. Only a few need a card's content: `srl next`
for the one card it returns, and `srl cards`. So a kind program answers
two questions:

| Call | Input | Output | Rule |
|---|---|---|---|
| `srl-kind-NAME ids` | the item's fields, as JSON | the card suffixes, one per line, or one empty line for a single card with the item's ID | **Deterministic and fast**: the same fields always give the same IDs. Called for every item of the kind, on every command that reads items, so its result would need a cache: notes themselves are not cached (see [store.md](store.md), "The index cache"). |
| `srl-kind-NAME card` | the item's fields, the card's suffix, its context and the study day, as JSON | one card's `prompt`, `prompt_revealed` and `answer`, as JSON | **May differ every time.** Called only when a card is about to be shown. |

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

> **TODO (design):** the wish is for `prompt_revealed` to be **optional
> in a kind program's output**: when `card` leaves it out, `srl` would
> fill it in from `prompt`, so that the many kinds whose question hides
> nothing, such as the LLM kind below, needn't repeat it. Nothing is
> decided. It makes writing a plugin easier, but it goes against the
> wish for flat rules: a field that is sometimes written by the kind and
> sometimes made up by `srl` is one more case to know about. As long as
> it is required, a kind that forgets it is noticed at once, because the
> question disappears when the answer is shown; once it is optional,
> forgetting it is never an error, even for a kind that hides part of its
> question and meant to reveal it. Frontends would be unaffected either
> way, since `srl next --json` would always give the field. To be
> weighed when plugin support is designed.

> **TODO (design):** the output of `ids` leaves three cases open, all
> because an empty line means a card:
>
> - **No output at all.** Is it an error in the note, or an item with no
>   cards?
> - **An empty line mixed with suffixes**, such as an empty line and
>   `te`. It would give cards `v81k3z` and `v81k3z-te` from one item.
>   [cards.md](cards.md), "Card IDs", implies this is not allowed (an item
>   gives either one card with its ID or suffixed cards), but the rule for
>   `ids` doesn't say so.
> - **A trailing newline.** `te\n` and `te\n\n` differ only in
>   whitespace, but if an empty line is a card, a program that prints one
>   newline too many gets an extra card with the item's ID.
>
> One way out is JSON output, where a single card is explicit, such as
> `[{"suffix": null}]`: an empty array can be an error, a `null` suffix
> can't be mixed with others, and whitespace means nothing. To be decided
> when plugin support is designed.

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
  it wrote as both `prompt` and `prompt_revealed`, and the answer it wrote.
  A new card each time, under the same ID.

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

## Knowledge structure

- **Prerequisites.** Items could name the items they require, for example
  in a header entry. A new header entry changes the item syntax. Finding
  prerequisites in notes, by hand or by an agent, is then work on top.
- **Partial credit for prerequisites.** A correct answer about a card also
  counts, partially, for the cards it requires. Needs a scheduling rule.

## Stores

- **Export and import between stores.** Move or copy notes from one store to
  another together with the review history of their cards, so that their
  memory state comes along. It needs rules for ID clashes and for merging
  logs. Copying a note by hand moves only the content.
