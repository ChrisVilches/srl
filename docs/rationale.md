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

## Why a data layer and one mapping, and nothing else

`srl` is a CLI over its data (notes with items, the log, the
configuration) plus one fixed mapping from that data to frontends:
`srl next --json` (see [overview.md](overview.md), "The idea").
Everything around it, such as showing cards, launching programs, choosing
how things look, or calling an LLM, is left to programs built on top.

What only `srl` can do well is keep the data correct: parse notes
strictly, never damage them, replay the log the same way every time, and
apply one schedule for every frontend. What people want to vary is
everything else. Keeping the two apart means a frontend, a script or an
agent can be replaced without touching `srl`, and `srl` can change inside
without breaking them, as long as the contracts hold.

Only `srl next --json` is a mapping to frontends, because only it gives
what a frontend shows: a card, made for showing, with its context.
Recording a rating, with `srl review`, takes a card ID and a rating, which
are the log's data, so it is an ordinary command that frontends use like
anyone else. The less `srl` knows about frontends, the fewer of its
commands a frontend can constrain.

Trade-off: a feature that would be convenient inside `srl`, such as
running your frontend as `srl web`, is left to you: a shell alias or a
script does it. For the same reason `srl` has no commands of your own,
whether found on `PATH` (`srl-NAME`) or listed in the configuration:
your shell already runs programs, and `srl` would only be launching
them.

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
fixed.

## Why a card that can't be made stops studying

`srl next` picks one card. If its kind program can't make it, `srl next`
fails, instead of trying the next card. Going on would hide the failure
behind a card that works: the broken card would quietly never be
studied, and a frontend would have nothing to tell apart from success.
`srl next` only reads, so the broken card stays first, or among the first
when `max_skip` lets it skip a few, until something changes. Getting past
it is a separate command you choose: fix what failed, or `srl suspend` the
card.

Trade-off: one card that can't be made, for example while an LLM API is
down, blocks studying until it is fixed or suspended, or, with
`max_skip`, interrupts it each time it is picked. Tools to make that
less annoying are still to be designed (see [overview.md](overview.md),
"Open questions").

## Why `srl` changes notes so little

Your notes are yours. `srl id` and `srl accept` change header lines only,
adding an ID or removing `draft`; `srl add` only appends new items; and
`srl edit ID --stdin` replaces the lines of the one item it was given. No
command rewrites a note, so a change by `srl` can't damage text you wrote,
and is easy to see in a diff.

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
seeing one would give the other away. Cards that don't give each other
away, such as two words of one sentence, are in different groups, and
not held back at all.

Trade-off: when you change a cloze number or an item's kind, a card's
history is orphaned until you move it with `srl move`. `srl` never guesses.

## Why `srl next` may skip a few cards at random

Always showing cards in the same order makes studying predictable: you
learn what comes next, not only the answers. Picking uniformly among the
first few cards breaks the pattern while keeping to the schedule, which
only works in whole days anyway, so reordering the cards of one day costs
nothing. Skipped cards are not set aside, since `srl next` writes nothing,
so none can starve: the oldest stays among the first until it is shown.
The sibling rule decides which cards are candidates before any skipping,
so skipping can never show two siblings in a day.

Trade-off: `srl next` no longer gives the same answer twice when
`max_skip` is above 0; the default, 0, keeps it fixed.

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

## Why `qa` is built in, and other kinds are programs

Most questions are a question and an answer, sometimes with cloze
deletions. So `qa` is built in, and what a question looks like is in its
text: an item with cloze deletions behaves as cloze, with nothing to
configure.

Other kinds turn structured data into questions: the forms of a verb, the
words of a sentence. They are programs named in `.srl/config.toml`, so
that a new kind needs no change to `srl`, and is written in whatever
language suits it. A kind only makes cards: it checks its items, gives
their card IDs, in groups of siblings, and their content when asked, and
describes its fields. Everything else, scheduling
included, is the same for every kind, so a program can't change how `srl`
behaves, only what it shows.

Trade-off: a program is something to install and keep working, it runs
with your rights, and every command that reads its items runs it.

## Why a kind program answers four calls, each doing one thing

A kind program has four jobs, and each is its own call: `describe` says
what the kind takes, `validate` checks an item, `groups` gives a valid
item's card IDs, and `make` makes a card's content. One name per job
keeps every job visible: a program can't do one of them by accident, as
a side effect of another, or leave it out without its author noticing.

Card IDs and card content come from different calls because they have
opposite needs. IDs are needed by nearly every command (selectors,
`srl find`, `srl check`, scheduling) and must be stable, since a card's
history is kept under its ID. Content is needed only when a card is
shown, and may take long to make, as when a program asks an LLM to write
the question, or differ every time. Kept apart, a command that only needs
IDs never waits for content, and a program that generates its cards does
the slow work only for the cards about to be shown.

Checking is a call of its own, apart from `groups`, because it is the
kind's only check: an item is an error only if the kind says so. If
checking happened only inside the call that gives card IDs, a program
that just computes IDs and accepts any fields would be easy to write,
and its broken items would pass every check and fail only when studied.
A call named `validate` can't be forgotten that way.

Trade-off: a program has four calls to write, and `validate` and
`groups` must agree on which items they can handle.

## Why `srl` calls `validate` before `groups`

`srl` asks `groups` only about the items `validate` accepted. So the two
calls can't contradict each other in a way that matters: `groups` never
has to check, and when it fails on an item `validate` accepted, the
fault is the program's, not the item's. Each failure then points at whoever
can fix it: an error `validate` gives is your note's, a call that fails as
a whole is the program's, and a card `make` can't make is a problem when
studying, such as an API that is down. If one call did both, a program
that crashed would look exactly like a mistake in your note.

This is still "parse, don't validate" (see [rust.md](rust.md)): the
check and the structure don't drift apart, because `srl`, not the
program, decides that nothing reaches `groups` unchecked.

## Why calls are batches

Every command that reads items needs the card IDs of every item. With
one process per item, a store with many items of a kind would start that
many programs on every command. Each call is a batch instead, so a command
starts at most two processes per kind program (`validate` and
`groups`), however many items it has. A single input is a batch of one
line, so there is one way to write a program, not two.

The program reads to the end of its input, then answers, or answers as
it goes: both work, because `srl` closes the input before it waits. So a
program can't hang `srl` by holding back its output, and one that sees a
whole batch can do its work once for all of it, such as running a
dictionary once over every sentence. Answers are matched to inputs by
their place, the simplest rule; a program that prints the wrong number of
lines fails as a whole.

Trade-off: a program reads and writes lines in a loop instead of one
object, and a call starts a process, however few lines it has.

## Why answers of kind programs are kept under the kind and its command

Starting a process costs far more than anything `srl` does with a
store's files: an interpreter alone takes tens of milliseconds, and a
command that reads items calls `validate` and `groups` for every kind
program. Both calls are deterministic, so their answers are kept in the
index cache and only new or changed items are asked about. A command on
an unchanged store then starts no kind program at all.

An answer is kept under the kind's name, its command as written, the
call and the item's input line, and nothing else. The command is what
says which program answered: changing it in `[kinds]`, for example to
choose another version, asks again on its own. The name says which kind
an answer belongs to, so that one kind never uses answers given to
another, at the cost of asking again when a kind is renamed. Two other ways to notice a changed program in place were
rejected:

- **Hashing the files the command names**, such as `kinds/ja_words.py`.
  It misses whatever that file imports or reads, so it would be a
  guarantee that holds only sometimes.
- **Asking the program for its version** on every command. It costs a
  process start per kind program, which is most of what keeping answers
  saves, and its author must still remember to change the version.
  Asking only now and then would make results depend on the clock.

So `srl` doesn't try: after changing a kind program, you run
`srl check`, which asks again about every item, or delete the cache,
which forgets every kept answer. There is no command for deleting it:
the cache is never a source of truth, so deleting `.srl/cache/` by hand
loses nothing.

Trade-off: forgetting to run `srl check` after changing a program
leaves its old answers in use, until an item changes or the cache is
deleted. Answers about earlier versions of items are kept until then
too, unused.

## Why `groups` sees only the fields

`groups` gets an item's ID and fields, and nothing else: not its note,
not its context, not the day. Card IDs have to stay the same for as long
as the item's fields do, because a card's history is kept under its ID.
If `groups` could read the prose around an item, editing that prose
could change or drop card IDs, and history would be lost without the item
changing at all.

`make` may differ every time anyway, so it gets the context and the day:
a program can use the note as background, as `examples/kind-llm-generated`
does when it asks an LLM for a question. The context is always in mode
`prose`, whatever mode a frontend asks for, so a card doesn't depend on
the frontend that shows it.

## Why a kind describes itself, and the description only informs

To write an item you need to know its kind's fields, and only the kind
knows them. So every kind program answers `describe`, and
`srl kinds` prints what it says: a person, an editor or an agent learns
the fields from `srl`, not from the program's source or from a template
kept somewhere else. It is required, not optional, so that every kind can
be written the same way, and a program that can't describe its kind
fails loudly instead of leaving an editor with nothing to show.

A description is text for people, never a schema `srl` checks items
against. The rules of a kind are often conditional, as `qa`'s "A may be
left out when Q has cloze deletions", and a schema able to state them
would be a second, partial copy of the kind's own checks, bound to drift
from them. With one place that checks, the kind, a description that
falls behind only misleads, and `srl add --dry-run` still gives the right
answer.

Trade-off: a kind program has a call to write that only informs, and
nothing makes its description agree with what `validate` accepts.

## Why groups of siblings, without names

Siblings are about content: two cards are siblings when seeing one gives
the other away, and only the kind knows. So the kind says, by writing its
cards in groups. Groups are kept out of card IDs: an ID is permanent,
while how cards give each other away is a scheduling choice a kind may
change, so changing it must not orphan any history. Groups have no names,
since a name would be one more thing to invent and to mistype; the
structure says it all.

Trade-off: which cards are siblings can't be seen in a card ID.

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

The revealed prompt does contain the answer, on purpose: a card also
carries the question as it reads once the answer is shown, so a cloze's
blanks can be filled in where they are, and the answer holds only what you
wrote in `A`, not the question again.

Trade-off: a frontend has a third field to show, and one that shows only
`answer` shows nothing for a cloze card without `A`.

## Why no built-in frontend

The study loop is `srl next --json`, showing what it prints, and
`srl review`. Every rule of scheduling is in `srl next`, so a built-in
frontend would add nothing a separate one can't do, and would be the one
part of `srl` that has opinions about appearance: colors, keys, layout.
Those are exactly what people want to change, and a frontend of a few dozen
lines is quick to write, or to have an agent write, from an example.

So `srl` has no interactive study command. It stays a CLI that other
programs drive, the same on every platform, and frontends live outside it:
`examples/` has a web viewer and a terminal frontend to copy (see
[guides/frontends.md](guides/frontends.md)). `srl next --json` gives
everything a frontend shows (the card, its state, its kind, note and context),
so a frontend needs no other command to read (see
[frontends.md](frontends.md)).

Trade-off: `srl` alone has no comfortable way to study. You install or
write a frontend first, and one that is not a shell script, such as the
web viewer, needs more than `srl` to run. In exchange there is no terminal
handling to keep working on every platform, and no frontend that is more
official than yours.

## Why the frontend chooses the context mode

Whether to show the whole note after the answer, the note without its
items, or nothing, depends on the screen and on the person: a terminal
frontend may want less than a browser. That is presentation, so it is a
flag of `srl next` that each frontend sets, not a setting of the store
that every frontend would have to share.

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

## Why notes are chosen by globs

Which files are notes is two lists of globs, `include` and `exclude`, and
`exclude` always wins. Globs say more than a list of extensions (only the
notes under one folder, say) with the same matching that `exclude` and
`path:` already use. Every note is read with the same syntax whatever its
name, so there is no list of file types to keep up to date, and no rule
for each.

Trade-off: a glob can make a file a note whose item syntax makes no sense
for it, such as HTML or source code; `srl` reads it anyway.

## Why flat rules

Every rule in `srl` can be stated in one sentence, and each one picks the
simplest option even when a smarter one exists: whole days instead of
learning steps, one global new-card limit, one card per item per day. The
result is behaviour you can predict without reading code. Because the log
records everything, a smarter rule can always be introduced later and
applied to the whole history.

One exception is accepted on purpose: a card's fields other than `prompt`
may not exist (see [cards.md](cards.md), "Format"). A missing
`prompt_revealed` means "the same as `prompt`", and a missing `answer`
means there is nothing to show. It is one more case for kinds and
frontends to know, but forgetting it costs little: a kind that never
writes `prompt_revealed` still shows every card correctly, without the
answer's highlights in the question, and kinds whose question hides
nothing, which are most of them, never need to know the field exists.
The cost is that a kind which forgets its `answer` isn't stopped: its
cards show no answer, which is a valid card, not an error.
