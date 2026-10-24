# Ideas for things built on srl

This is a guide, not part of the specification (see
[overview.md](../overview.md), "Specification, guides and examples"). It
records ideas for programs and workflows that would use `srl` as it is,
through its commands and formats, so they are not lost, not as promises.
Ideas that would change `srl` itself are in [ideas.md](../ideas.md).

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
- **Code exercises checked by tests.** An agent writes an exercise, runs your
  solution in a sandbox and records the rating.

## Frontends

- **Confidence calibration.** Rate your confidence before answering, and track
  how well confidence predicts correctness.
- **Ambient review.** One question at a time in the shell prompt, the tmux
  status line or a git hook.

## Importing

- **Import from Anki.** The cloze syntax is Anki's, so a note type with a
  front, a back and cloze deletions maps to `qa` items directly. An
  importer writes them with `srl add`.

## Analysis

- **How well an agent's judgment agrees with yours**, by having it rate
  answers you also rated yourself.
