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
- It can be wrong again each time; a checked question stays correct.
- You cannot measure whether you are improving.

So the minimum database is **questions with stable IDs and a log of
reviews.** Everything else must justify itself.

## Why questions live inside notes

Writing a question should take seconds, next to the knowledge it asks
about. A separate file per question, with its own metadata, made adding
knowledge slow and split it from the notes it came from. Inside a note, a
question is three lines, and the note around it explains the answer for
free.

Trade-off: a note is a file you also edit by hand, so `srl` must parse
text written for people, and must be careful when it writes to it.

## Why text files, not SQLite, as the source of truth

Files can be edited in any editor, diffed by git line by line, searched with
`grep`, and read by agents directly. A database is better at queries and
integrity, but those are covered by a derived index and `srl check`. The
reverse is not true: editing, diffing and plain-text access cannot be added to
a database without export and import machinery.

Trade-off: validation happens on read rather than on write.

## Why headers are HTML comments

A header must say "this is an item" and carry its ID without spoiling the
note. HTML comments are part of CommonMark, so every renderer hides them,
and in an editor they read as what they are: a short remark above the
question. No syntax specific to one app is needed.

Trade-off: `<!-- srl -->` is longer to type than a symbol would be. An
editor snippet makes it one keystroke.

## Why only headers make items

If any `Q:` line made an item, ordinary text would be read as questions:
an interview transcript, an FAQ, a quote. With one rule, "an item starts
with a header", `srl` never reads into your prose, and everything it reads
it can check strictly.

Trade-off: a question typed without a header is silently not studied.
`srl lint` looks for these, as a convenience.

## Why one error stops everything

A note with a mistake in an item is a question you think you are studying
and aren't. Reporting it every time, until it is fixed, is the only way to
be sure it is noticed. Skipping it with a warning would be easy to miss
among other output.

Trade-off: a typo in one note blocks studying every note until it is
fixed. A non-strict mode may come later.

## Why `srl` writes only header lines

Your notes are yours. The commands that change them add an ID or remove
`draft`, on the header line, and nothing else; new items are only ever
appended. So a change by `srl` can't damage text you wrote, and is easy to
see in a diff.

## Why IDs, not positions

An item's position in a note changes whenever you add a question above it.
Using it as identity would give one question's history to another without
a word. An ID in the header moves with the item.

Trade-off: every header carries an ID, and copying an item copies its ID,
which is an error until you remove it.

## Why schedule cards, with siblings

Two cloze deletions in one sentence are two things you can know or not
know, so they are scheduled separately, as in Anki. Their IDs are made from
the item's ID and the cloze number you wrote, so they stay stable when the
item changes. The one thing siblings share is the sibling rule, because
seeing one would give the other away.

Trade-off: when you change a cloze number or an item's kind, a card's
history is orphaned until you move it with `srl move`. `srl` never guesses.

## Why you always rate yourself

Typing answers is slow, and a typed answer that was right in substance often
fails a string comparison: another spelling, a synonym, kana instead of
kanji. Seeing the answer and rating yourself is faster and more honest, as
in Anki. So `srl` never judges answers: there are no answer checks, no
suggested ratings, and every rating is given.

Judging a free-form answer is still possible, by an agent outside `srl`:
you answer it, it decides the rating, and records it with `--by agent`. The
core stays the same either way.

Trade-off: nothing stops you from rating yourself too kindly.

## Why the kinds are built in

Most questions are a question and an answer, sometimes with cloze
deletions. Programs that turned structured data into questions could do
little without an LLM, and with one they wrote questions that were better
written as items. So what a question looks like is in its text: an item
with cloze deletions behaves as cloze, with nothing to configure.

Trade-off: a new kind of question needs a change to `srl`, until kinds as
plugins are designed.

## Why editing content never affects the schedule

You should be able to fix and improve items without the app objecting. Any
automatic policy (reset on large changes, warnings) would need a rule for
what counts as a large change, and would sometimes be wrong. Resetting is an
explicit command instead. The content hash in each review keeps analysis
possible later.

## Why a fixed card format

Frontends understand cards and nothing else, so they never change when the
note syntax or a kind does. Any frontend can show any card.

Trade-off: a kind of question that deserves a special interaction (drag to
reorder, drawing) is shown plainly unless a specialized frontend is
written.

## Why the prompt never contains the answer

Writing an answer once and marking it is convenient, and cloze deletions
do exactly that. But a card is produced by code, so repeating the answer in
it costs nothing, and keeping it out of the prompt means any frontend, log
viewer or `jq` can show a card without giving the answer away.

## Why a built-in frontend

The study loop can be written as a script around `srl next`, `srl show` and
`srl review`, so `srl study` adds nothing a frontend couldn't do. It is built
in anyway, for two reasons:

- **A script can't be the same on every platform.** A shell loop needs `sh`
  and `jq`, and doesn't run on Windows without extra tools; a PowerShell one
  runs nowhere else. `srl study` is part of the binary, so studying works
  the same on Linux, macOS and Windows.
- **A person needs nothing but a terminal and an editor.** Without a
  built-in frontend, `srl` would have no way to study until you wrote or
  installed one.

Trade-off: `srl` keeps one interactive command to maintain. It lives outside
the core, and calls the same core functions as `srl next` and `srl review`,
so it can't schedule differently from any other frontend.

## Why no LLM in the core

The store should be like a notes folder: complete on its own, and open to any
agent. Keeping LLMs outside means `srl` works offline, costs nothing to run,
behaves the same every time, and is not tied to a model, a provider or an
agent. An agent contributes the way a person does: by writing notes and
running commands.

## Why agent-written questions are ordinary items

An agent that writes questions writes items, in a note of their own, with
`see:` pointing to the note they ask about. There is no separate cache or
session for generated questions: that would be a concept that exists only
for agents, and would hide content outside your notes. As items, you can
read, edit, accept or delete them like any other.

Trade-off: generated notes accumulate in the store, like any notes.

## Why not tied to Obsidian

Notes are plain Markdown, written in any editor and read in any viewer.
Everything `srl` adds uses syntax common to Markdown flavours, so notes
look right in every app, and `srl` assumes none.

## Why flat rules

Every rule in `srl` can be stated in one sentence, and each one picks the
simplest option even when a smarter one exists: whole days instead of
learning steps, one global new-card limit, one card per item per day. The
result is behaviour you can predict without reading code. Because the log
records everything, a smarter rule can always be introduced later and
applied to the whole history.
