# Ideas

Mechanics that are not part of the specification yet. Each one can be built
on top of the current design, mostly as agent workflows, drills or frontends,
without changing the core. They are recorded here so they are not lost.

## Agent workflows

- **Brain-dump recall.** "Write everything you know about Rust ownership." An
  LLM compares the answer with the atoms of that topic and records a review for
  each atom you recalled or missed. One prompt reviews many atoms.
- **Socratic repair of leeches.** After repeated lapses, a short dialogue looks
  for where your understanding breaks, then proposes a rewrite of the atom or a
  new draft atom for the misconception.
- **Error classification.** On a wrong answer, an LLM labels the error and a
  fixed rule decides what to do: a slip needs nothing, confusion with another
  atom adds a `confusable:` link, a missing prerequisite points to the atom that
  should be studied first.
- **Synthesis questions.** One question that needs two or three due atoms from
  different topics, recorded as a card about several atoms.
- **Contextual questions.** Questions set in your own projects and interests,
  using a personal context file.
- **Language practice at "i+1".** Sentences using only vocabulary you already
  know (`srl find tag:vocab state:mature`) plus one new item.
- **Teach-back.** You explain an atom to a simulated confused student, or write
  the question yourself, and an LLM points out what was missing.
- **Source drift.** Compare an atom with the current version of its `source`
  and propose an edit when the original changed.

## Knowledge structure

- **Knowledge graph extraction.** An agent extracts prerequisites from notes
  and records them as a `requires:` facet.
- **Partial credit for prerequisites.** A correct answer about an atom also
  counts, partially, for the atoms it requires. Needs a scheduling rule, so it
  would change the core.

## Studying

- **Confidence calibration.** Rate your confidence before answering, and track
  how well confidence predicts correctness. A frontend feature.
- **Adaptive difficulty.** Prefer harder cards (production over recognition)
  for atoms with high stability. Would change how `srl next` picks a card.
- **Ambient review.** One question at a time in the shell prompt, the tmux
  status line or a git hook.
- **Code exercises graded by tests.** A drill writes an exercise; the grader
  runs it in a sandbox.

## Analysis

- **Compare paradigms on yourself.** Every review records its drill, so you can
  measure whether atoms reviewed one way are retained better than atoms
  reviewed another way.
- **How well the LLM grader agrees with you**, by comparing its suggested
  ratings with the ratings you recorded.
