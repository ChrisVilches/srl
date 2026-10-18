# Ideas

Mechanics that are not part of the specification yet. They are recorded here
so they are not lost, not as promises. Most can be built on top of the
current design, as agent workflows or frontends, without changing the core;
the ones that would change the core say so. An idea becomes a design
decision to take, listed in [overview.md](overview.md), "Open questions",
once it is planned; kinds as plugins is one.

## Kinds

- **Kinds as plugins.** A header with `kind:NAME` would run a program
  `srl-kind-NAME`: it reads the item's fields as JSON and writes cards, each
  with a suffix for its ID (see [cards.md](cards.md), "Card IDs"). For
  example, a conjugation kind:

  ```markdown
  <!-- srl id:v81k3z kind:conj -->
  VERB: 動く
  FORMS: te past negative
  ```

  giving cards `v81k3z-te`, `v81k3z-past` and `v81k3z-negative`. Would
  change the core: finding programs, running them, and reporting their
  failures.

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
