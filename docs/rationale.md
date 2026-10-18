# Rationale

Why the main decisions were made. Each section states the decision, then the
reasons and the trade-off accepted.

## Why a database at all, instead of just prompting an LLM

Asking an LLM "quiz me on N1 grammar" works, and gives you a tutor. It cannot
give you spacing and repetition:

- It does not remember what you reviewed last week, so it cannot know what is
  due.
- Every session invents new questions on new topics, so the same knowledge is
  never tracked over time.
- It drifts towards well-known material and skips the long tail.
- It can be wrong again each time; a checked atom stays correct.
- You cannot measure whether you are improving.

So the minimum database is **atoms with stable IDs and a log of reviews.**
Everything else must justify itself.

## Why text files, not SQLite, as the source of truth

Files can be edited in any editor, diffed by git line by line, searched with
`grep`, and read by agents directly. A database is better at queries and
integrity, but those are covered by a derived index and `srl check`. The
reverse is not true: editing, diffing and plain-text access cannot be added to
a database without export and import machinery.

Trade-off: thousands of small files, and validation happens on read rather
than on write.

## Why schedule atoms, not cards

A card is one way of asking. Scheduling the knowledge instead of the wording
means questions can vary from review to review, which stops you from
memorizing the card instead of the fact, and lets new kinds of questions be
added without losing history.

Trade-off: cards of the same atom differ in difficulty, and FSRS sees that as
noise. The `choice` rule (no Easy) removes the largest source of it.

## Why front-matter is data and the body is prose

A body with special sections (`## Cards`, `## Examples`) would need a second,
fragile parser and would make the body something you must keep valid. With
one rule, programs read only YAML, which `srl check` can validate strictly,
and the body is completely free.

Trade-off: examples and hand-written cards are written in YAML rather than as
Markdown lists.

## Why editing content never affects the schedule

You should be able to fix and improve atoms without the app objecting. Any
automatic policy (reset on large changes, warnings) would need a rule for
what counts as a large change, and would sometimes be wrong. Resetting is an
explicit command instead. The content hash in each review keeps analysis
possible later.

## Why drills and a fixed card format

Facets and drills let new kinds of questions be added as small programs. The
fixed card format means frontends never need to change when that happens: with
N drills and M frontends, there are N + M pieces to write instead of N × M.
Grading also lives in the core, so the terminal and the web viewer judge
answers in the same way.

Trade-off: frontends present everything through a generic format, so a kind of
question that deserves a special interaction (drag to reorder, drawing) is
shown more plainly unless a specialized frontend is written.

## Why the prompt never contains the answer

Writing an answer once and marking it is convenient, and the `cloze` facet
does exactly that. But a card is produced by code, so repeating the answer in
it costs nothing, and keeping it out of the prompt means any frontend, log
viewer or `jq` can show a card without giving the answer away.

## Why no LLM in the core

The store should be like a notes folder: complete on its own, and open to any
agent. Keeping LLMs outside means `srl` works offline, costs nothing to run,
behaves the same every time, and is not tied to a model, a provider or an
agent. LLMs still contribute through three clearly bounded routes: agents
running the CLI, drills, and the external grader.

## Why agent-made cards go into facets

An agent that wants to add questions writes them into the atom's facets
(`cards:`, `cloze:`, `confusable:`), or provides a drill. There is no separate
card cache for agents. A cache would be a concept that exists only for agents,
would break the rules that drills run only for their facet and that cards are
not stored, and would hide content outside the atom file. Ready-made questions
already have a home in the `cards:` facet, where you can see, edit and version
them.

Trade-off: atom files grow when an agent adds many questions.

## Why not tied to Obsidian

A vault is one source of knowledge among many (articles, PDFs, LLM
generation, imports). `srl` has its own store, and atoms are copies, not
links. An atom is a checked, condensed version of something, and should not
change underneath its review history when the original note changes.

## Why flat rules

Every rule in `srl` can be stated in one sentence, and each one picks the
simplest option even when a smarter one exists: whole days instead of
learning steps, the same rating for every atom of a card instead of partial
credit, one global new-atom limit. The result is behaviour you can predict
without reading code. Because the log records everything, a smarter rule can
always be introduced later and applied to the whole history.
