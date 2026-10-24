# Ideas

Changes to `srl` that are not part of the specification yet. They are
recorded here so they are not lost, not as promises. Each would change
`srl`'s data, commands or contracts; ideas for things built on top of
`srl` as it is are in [guides/ideas.md](guides/ideas.md). An idea becomes a
design decision to take, listed in [overview.md](overview.md), "Open
questions", once it is planned; kinds as plugins is one.

## Kinds

### Kinds as plugins

A kind could be a program, named in `.srl/config.toml`:

```toml
[kinds]
ja-words = ["python3", "kinds/ja_words.py"]
```

A header with `kind:ja-words` would then run that program. Would change
the core: reading `[kinds]`, running programs, and reporting their
failures.

- **The value is a command**: the program, then its arguments. `srl`
  adds `ids` or `card` (see below) as the last argument. It is a list, not
  a path, so that a script runs the same on every platform: Windows can't
  run a script by its `#!` line, and would take
  `["py", "kinds/ja_words.py"]`.
- **A relative program path is relative to the store's folder**, like
  note paths, and the program runs in the store's folder, so relative
  paths among its arguments work too. A program named without a folder,
  such as `python3`, is found as the platform finds programs.
- **`qa` can't be configured**, and a `kind:NAME` that is neither `qa`
  nor in `[kinds]` is an error in the note, as an unknown kind is today.

**Card IDs and card content come from two separate calls.** Every command
that reads items needs their card IDs: selectors, `srl find`, `srl check`,
scheduling, the sibling rule. Only a few need a card's content: `srl next`
for the one card it returns, and `srl cards`. So a kind program answers
two questions. Each call reads one JSON object on standard input and
prints one on standard output. A call that exits with a status other than
0 failed, and what it printed on standard error is the reason.

| Call | Input | Output | Rule |
|---|---|---|---|
| `ids` | the item: `{"id": "v81k3z", "fields": {"VERB": "動く", "FORMS": "te past negative"}}` | its cards, in groups: `{"groups": [{"cards": ["te", "past"]}, {"cards": ["negative"]}]}` | **Deterministic and fast**: the same fields always give the same IDs. Called for every item of the kind, on every command that reads items (see the TODO on caching below). |
| `card` | the same object, plus `"suffix"`, `"context"` (as in `srl show --json`) and `"day"` (the study day, such as `"2026-10-27"`) | `{"prompt": "…", "prompt_revealed": "…", "answer": "…"}`: the card's fields of [cards.md](cards.md), "Format", without `id` | **May differ every time.** Called only when a card is about to be shown. |

`fields` is as in `srl show --json`. `srl` checks first that no key is
written twice in the item, for every kind, so the object loses nothing.

**Groups decide siblings.** The cards of one group are siblings, and the
sibling rule ([scheduling.md](scheduling.md), "Siblings") applies to each
group instead of each item: each study day, a group offers at most one
card. A kind whose cards give each other away puts them in one group, as
`qa` would with its cloze cards; a kind whose cards are independent gives
each card its own group. Groups have no names: they are only how the
cards are written, so a kind can change them without touching any card's
history. The output of `ids` holds:

- at least one group, and no group without cards;
- each suffix once, made as in [cards.md](cards.md), "Card IDs";
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

Another kind with several cards, none of them siblings, is in
[examples/kind-ja-words/](../examples/kind-ja-words/): one card per
`{word}` of a sentence.

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

Other rules a plugin design would need:

- **Fields are checked by `ids`.** A program that exits with an error on
  `ids` makes the item an error in its note, like a `qa` item with a
  missing `Q`.
- **A failing `card` call doesn't stop everything.** An LLM API can be
  down. `srl next` reports the failure and goes on to the next card, as
  if the failed card were not eligible today.
- **`srl cards` calls `card` for every card it prints**, so a kind whose
  content differs every time, such as the LLM kind below, gives a new
  card each time: what `srl cards` prints is one card the kind could
  give, not the one `srl next` will show. A failing `card` call makes
  `srl cards` fail at once, with the program's reason and no output for
  the other IDs.
- **A time limit** for both calls, set in `.srl/config.toml`.
- **`srl add` needs no change**: it takes field names from JSON, and
  `ids` checks them.
- **Running a program is your choice.** `[kinds]` runs whatever command
  you write there, with your rights, whenever a command reads an item of
  that kind. It is in your own store's configuration, so `srl` neither
  checks nor limits the programs, as with plugins in most tools. A store
  you copy from someone else runs their programs: read its `[kinds]`
  first.

> **TODO (design):** `ids` results are **not cached**: every command
> that reads items calls `ids` once for every item of a program kind, so
> a store with many such items is slow. A cache would make `srl` faster
> without changing what any command does. What it would be keyed on, and
> what would make an entry stale (the item's text, the kind's command,
> the program changing), is still to be analyzed. Notes themselves are not
> cached either (see [store.md](store.md), "The index cache").

### Proof of concept: questions written by an LLM

A kind whose card is written anew by an LLM every time it is shown:

```markdown
<!-- srl id:q8w2e5 kind:llm-generated -->
PROMPT: Ask me to use 〜をものともせず in a new sentence about everyday
  work. The answer is a model sentence and its translation.
```

- `ids` checks that the item has exactly one field, `PROMPT`, and prints
  `{"groups": [{"cards": [null]}]}`: one card, with the item's ID
  `q8w2e5`. No call to the LLM.
- `card` sends `PROMPT`, and the item's context
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
