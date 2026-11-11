# Rationale

Why the main decisions were made. Each section states the decision, then the
reasons and the trade-off accepted.

## Why a database at all

Rereading your notes, or being quizzed on them by a friend, a tutor or a
quiz generator, works, and can teach a lot. Without a record, it cannot
give you spacing and repetition:

- Nothing remembers what you reviewed last week, so nothing knows what is
  due.
- Every session asks new questions on new topics, so the same knowledge is
  never tracked over time.
- It drifts towards what is easy to ask or already known, and skips the
  long tail.
- A question made up on the spot can be wrong each time; a checked question
  stays correct.
- You cannot measure whether you are improving.

So the minimum database is **questions with stable IDs and a log of
reviews.** Everything else must justify itself.

## Why a data layer and one mapping, and nothing else

`srl` is a CLI over its data (items, from notes or other sources, the log, the
configuration) plus one fixed mapping from that data to frontends:
`srl next --json` (see [overview.md](overview.md), "The idea").
Everything around it, such as showing cards, launching programs, choosing
how things look, or generating questions, is left to programs built on
top.

What only `srl` can do well is keep the data correct: read every source
strictly, never damage them, replay the log the same way every time, and
apply one schedule for every frontend. What people want to vary is
everything else. Keeping the two apart means a frontend, a script or any
other program can be replaced without touching `srl`, and `srl` can change inside
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

## Why items are data, apart from notes

Questions could live inside notes, next to what they ask about. In
practice they are rarely written there by hand: they are added through a
frontend or by an agent, and for those a note is the wrong place, since a
program would have to find a spot in prose written for people, and the
questions would then sit in the middle of what you read, looking and
placed as nobody chose. So items are data, kept in structured
collections, and notes are study material, kept as you write them.

- **A note holds no items**, so `srl` never parses prose, never writes to
  a note, and never has to hide items when a note is shown as context:
  the note is shown whole.
- **An item names its note with `see`.** Any number of items, of any
  collection or source, can ask about one note, and a note can be any
  file: Markdown, plain text, a PDF.
- **Collections are made for programs** (JSONL, JSON), with one format
  made for people who write items by hand (XML; see "Why three
  formats").

Trade-off: a question is not next to the knowledge in the same file, so
writing one by hand means two files, or a frontend. Keeping items in
Markdown is possible, as a source program (see "Why source programs"),
but `srl` offers nothing for it, such as hiding the items of a note shown
as context.

## Why text files, not SQLite, as the source of truth

Files can be edited in any editor, diffed by git line by line, searched with
`grep`, and read by any program directly. A database is better at queries and
integrity, but those are covered by a derived index and `srl check`. The
reverse is not true: editing, diffing and plain-text access cannot be added to
a database without export and import machinery.

Text files are the default, and what the built-in `filesystem` source
reads. A store may also name a source program that keeps items in a
database or behind a service (see "Why source programs"). Its items then
have whatever their storage gives: no plain-text history in git and no
`grep` unless the program offers them. That is the store's choice, made
in its configuration; the log, the configuration and the notes stay text
files whatever the sources.

Trade-off: validation happens on read rather than on write.

## Why three formats

A collection is data, so its format is structured enough that every
mistake in it is an error, not text silently left out of an item. Which
structure depends on who writes it (see [formats.md](formats.md)):

- **JSONL** for programs that add and change items one at a time:
  frontends, agents, importers. A JSON object per line is what programs
  write best, and what `srl add` takes; parsing a line is a JSON parse,
  a field holds any text, and an item is a line, so `srl` changes an item
  by writing its line again, and the diff is that line.
- **JSON** for programs and tools that write a file whole. It holds the
  same item objects, but no line is an item, so changing one rewrites the
  file, and its layout is not kept.
- **XML** for people who still write items by hand, with fields of many
  lines: code, poems, worked examples. JSON can't hold a line ending
  without `\n`, which no one types by hand. In XML, a field is an element
  whose text reads as written, indented under it, and CDATA keeps
  anything else exactly; elements and attributes say which text is which
  entry or field, with no syntax of `srl`'s own to learn, and every
  editor knows it. `srl` changes only the element of the item it changes,
  so comments and everything else you wrote stay.

YAML was left out: free text in it is easy to misread silently (a ` #`
starts a comment that drops the rest of a line, and `no`, `1.0` or a
leading `{` change what a value is), it has many ways to write the same
text, and keeping a YAML file as it was while changing one item is hard
to promise (see [ideas.md](ideas.md), "Writing items").

The item object is one format everywhere an item is JSON: a line of a
JSONL collection, an element of a JSON collection, a line given to `srl
add` or `srl edit`, and what `srl show --json` prints. So what `srl`
prints can be changed and given back as it is, and programs need to know
one format only.

A collection's format is decided by its extension, and which files are
collections by the globs of their source: never by what a file holds,
since looking inside can't tell a collection with a mistake from a file
that is not one, so a broken collection would silently give no items.
JSON files are left out by default, since a store often holds JSON
meant for other programs (see "Why files are chosen by globs").

Trade-off: `srl` writes a JSONL or JSON item it changes as the item
object of its entries and fields, without its spacing or key order, so
those formats are not for formatting by hand; XML is.

## Why sources

Files of the store are one place to keep items, and others are easy to
imagine: a database, a web service, another file format. Each one added
inside `srl` blurs what `srl` is: its rules for files, lines and
extensions leak into JSON outputs, selectors, the configuration and the
rules of other formats. What `srl` is, is what
happens to items once they are read: kinds, cards, scheduling, the log,
selectors and the frontend contract. Where items come from is data
access, and it sits behind one boundary, as a database sits behind a
repository in Clean Architecture or hexagonal architecture: every source
does the same few things (see [sources.md](sources.md), "What a source
does"), and nothing else in `srl` knows how any of them keeps its items.

- **A source is chosen by the configuration, not by a file's name.** One
  rule, the same for every source; a store of JSONL and XML collections
  needs no setting at all, since the default source reads them.
- **Each source's settings are its own**: a `filesystem` source takes its
  globs, and a program takes whatever keys it checks itself. No key means
  something for one source and nothing for another. So each source also
  shows its own settings in `srl sources`, and decides by its own rule
  which new collections `srl add` creates in it, rather than every source
  answering in the `filesystem` source's terms, as `include` and
  `exclude` globs that a database has no use for.
- **A collection is named everywhere as its source names it**, so
  that `srl add`, messages and selectors need no case for any source.
  Mounting each source under a prefix of its own, as Unix mounts file
  systems, would need rules for prefixes inside prefixes and for
  collections hidden under a mount, and gives nothing a program can't do
  by naming its collections under a folder of its own. So there is one
  rule: no collection's name is given by two sources.
- **Sources hold items, and nothing else.** Context is a file of the
  store, which `srl` reads itself (see "Why context is a file"), so a
  source never shows anything, and never needs to know what to hide.

## Why source programs

A database, a web service or a file format `srl` doesn't know is each
someone's particular choice, and none belongs in the core. A source
program keeps items its own way, outside `srl`, and answers the same few
things every source does (see [sources.md](sources.md), "Source
programs").

- **A program, not a library or a plugin API.** It can be written in any
  language, with any library its storage needs, and `srl` stays one
  binary with no plugins to load and no client of any database or
  service.
- **The same shape as kind programs**: a command in the configuration,
  one process per call, and every call a batch of JSON lines over
  standard input and output (see "Why calls are batches"). One way to
  write a program for `srl`, not two, and a command calls a source
  program a small, fixed number of times, never once per item.
- **Its items are item objects**, the one JSON format of an item (see
  "Why three formats"), read as strictly as `srl add` reads one. A
  program never makes IDs: `srl id` gives them, as for every source.
- **`srl` verifies every write by reading the source again.** It can
  check a file it parsed itself, but not a program's storage. Reading the
  program before and after its `write`, and comparing the items with what
  the changes should give, holds a program to the same promise as the
  `filesystem` source: only the items asked to change, changed, or
  nothing.
- **`srl` keeps nothing a program answers.** It can't know when a
  database or a service changed, so a kept answer could be stale without
  a word. Caching is the program's business: one that is slow to answer
  may keep a cache of its own, and knows when it is fresh. The
  `filesystem` source isn't cached either, for another reason: parsing a
  file costs about as much as hashing it to tell whether it changed (see
  [store.md](store.md), "The index cache").

Trade-off: a source program is a process started on every command that
reads items, so a slow one slows every command, and one that fails stops
them all, as an error in a collection does. And `srl` can't keep the
bytes of a program's storage as it keeps a file's: how carefully items
are written there is up to the program.

## Why context is a file

An item's context is the note it asks about, so it is that note, a file
of the store: not a collection as its source shows it, and not something
a source computes. `srl` reads the file and gives its text whole, and the
frontend renders it by its extension.

- **Nothing to hide.** Notes hold no items, so a note is shown whole,
  and no source needs to know what to leave out of it: the only choice a
  frontend makes is whether to get the context at all (`--no-context`).
- **An item without `see` has no context.** A collection is data; showing
  a JSONL file after an answer helps no one.
- **Which files are notes is a setting of the store, `[context]`**, not
  of a source, since an item of any source may name any note. Its globs
  keep collections, configuration and other files from being named by
  mistake, and give frontends the list to pick from (`srl notes`). A note
  can be any format, and one that isn't text, such as a PDF, is named but
  not read: its frontend may open it.

Trade-off: a context is read when it is shown, so a note that is
deleted or moved makes its items errors until their `see` is fixed, as
any broken reference does.

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

Trade-off: one card that can't be made, for example while a web service
its kind program needs is down, blocks studying until it is fixed or suspended, or, with
`max_skip`, interrupts it each time it is picked. Tools to make that
less annoying are still to be designed (see [overview.md](overview.md),
"Open questions").

## Why `srl` changes files so little

Your notes are yours: `srl` never writes to one. In a collection, `srl`
changes what it must and keeps the rest as it can: in JSONL, the line of
the item changed; in XML, the element of the item changed, with comments
and layout around it kept; new items are added at the end. Only JSON is
written again whole, since no part of it is an item's alone (see
[formats.md](formats.md), "How srl writes to collections"). So a change
by `srl` can't damage text you wrote, and is easy to see in a diff.

## Why IDs, not positions or lines

An item's position in a collection changes whenever you add a question
above it. Using it as identity would give one question's history to
another without a word. An ID among the item's header entries moves with
the item. For the same reason nothing names an item by its line: items
are named by their collection and their ID, and a line, where a
format has one, only appears in the message of an error.

Trade-off: every item carries an ID, and copying an item copies its ID,
which is an error until you remove it.

## Why `.` separates a card's suffix

A card ID must name its item from its text alone: the log keeps card IDs
of items that no longer exist, and `srl check` and `srl move` work on
exactly those. So the separator is a character no ID contains. `-` is
left to IDs, because people who name an item themselves write
`jp-grammar-desu`. `.` is safe everywhere a card ID goes: file names on
every platform, URLs, shells and selectors. `:` reads well too, but
Windows file names can't contain it, a relative link `a:b` is read as a
URL scheme, and it looks like `key:value`.

Trade-off: `k3m9q1.2` looks a little like a version number, and `.` can't
be used in an ID.

## Why one `is:` for status, schedule and leeches

A card has a status (`active`, `draft`, `suspended`), a place in its
schedule (`new`, `due`, `young`, `mature`) and is a leech or not. These
are three taxonomies: whether the card is studied, where it is in its
schedule, and whether it keeps being forgotten. A key for each would be
three more names to remember, and the obvious ones, `status:` and
`state:`, are too alike to remember which holds which value. Someone
picking cards wants "the drafts" or "the leeches", and doesn't care which
taxonomy the word belongs to. No value of one is a value of another, so
one key can take any of them, and `is:draft` and `is:leech` read as what
they mean.

Outside selectors the three stay apart, as the fields `status`,
`schedule` and `leech` of `srl find --json`, each with one value: a
program reads a field and never has to choose a key. Their names don't
look alike, and `schedule` rather than "state" says what the field is
about. A leech is a field of its own, not a place in the schedule,
because a leech is also due, young or mature.

Trade-off: the selector mixes three taxonomies that are kept apart
everywhere else. No two of their values can share a name, so a new value
must be named unlike all the others, and `is:a,b` can put a status and a
place in one list, which is valid but mixes them.

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
`max_skip` is above 0; the default, 0, keeps it fixed, and so does
`SRL_SEED`, for tests and scripts.

## Why you always rate yourself

Typing answers is slow, and a typed answer that was right in substance often
fails a string comparison: another spelling, a synonym, kana instead of
kanji. Seeing the answer and rating yourself is faster and more honest, as
in Anki. So `srl` never judges answers: there are no answer checks, no
suggested ratings, and every rating is given.

Judging an answer is still possible, by a program outside `srl`: for
example, one that runs code you wrote against tests decides the rating
and records it with `srl review`. The core stays the same either way.

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
shown, and may take long to make, as when a program looks words up in a
large dictionary, or differ every time, as when it draws a new arithmetic
problem. Kept apart, a command that only needs
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
can fix it: an error `validate` gives is your item's, a call that fails as
a whole is the program's, and a card `make` can't make is a problem when
studying, such as an API that is down. If one call did both, a program
that crashed would look exactly like a mistake in your item.

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

`groups` gets an item's ID and fields, and nothing else: not its collection,
not its context, not the day. Card IDs have to stay the same for as long
as the item's fields do, because a card's history is kept under its ID.
If `groups` could read the note an item names, editing that note could
change or drop card IDs, and history would be lost without the item
changing at all.

`make` may differ every time anyway, so it gets the context and the day:
a program can use the context as background, for example to pick a sentence
of it to ask about. It always gets the context, whether a frontend asks
for it or not, so a card doesn't depend on the frontend that shows it.

## Why a kind describes itself, and the description only informs

To write an item you need to know its kind's fields, and only the kind
knows them. So every kind program answers `describe`, and
`srl kinds` prints what it says: a person, an editor or a script learns
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
format of a source or a kind does. Any frontend can show any card.

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
lines is quick to write from an example.

So `srl` has no interactive study command. It stays a CLI that other
programs drive, the same on every platform, and frontends live outside it:
`examples/` has a web viewer and a terminal frontend to copy (see
[guides/frontends.md](guides/frontends.md)). `srl next --json` gives
everything a frontend shows (the card, its schedule, its kind, collection and context),
so a frontend needs no other command to read (see
[frontends.md](frontends.md)).

Trade-off: `srl` alone has no comfortable way to study. You install or
write a frontend first, and one that is not a shell script, such as the
web viewer, needs more than `srl` to run. In exchange there is no terminal
handling to keep working on every platform, and no frontend that is more
official than yours.

## Why the frontend chooses whether to get the context

Whether to show the note after the answer depends on the screen and on
the person: a terminal frontend may want less than a browser, and one
that opens notes itself wants no text at all. That is presentation, so
it is a flag of `srl next` that each frontend sets, `--no-context`, not
a setting of the store that every frontend would have to share.

## Why the core calls no service

The store should be like a notes folder: complete on its own, and open to any
program. `srl` itself calls no server and needs no account, so it works
offline, costs nothing to run, behaves the same every time, and is not tied
to a provider. A program that needs a service brings it itself: a kind
program that looks words up online, a source program that keeps items
behind a web service, or a program outside `srl` that contributes the way
a person does, by adding items and running commands. A store that names
such a program depends on its service; `srl` doesn't.

## Why generated questions are ordinary items

A program that writes questions, such as an importer or a generator,
writes items, in a collection of their own, with `see` pointing to the
note they ask about. There is no separate cache or session for generated
questions: that would be a concept that exists only for such programs,
and would hide content outside the store. As items, you can read, edit,
accept or delete them like any other.

Trade-off: generated collections accumulate in the store, like any
collections.

## Why not tied to Obsidian

Notes are files you write in any editor and read in any viewer, and
`srl` never writes to them, so they look as you wrote them in every app,
and `srl` assumes none.

## Why files are chosen by globs

Which files are a source's collections, and which are notes, is two lists
of globs each, `include` and `exclude`, in the source's table and in
`[context]`, and `exclude` always wins. Globs say more than a list of
extensions (only the files under one folder, say) with the same matching
that `exclude` already uses. A collection's format is chosen
by its extension, so a glob that matches a file of another format makes
it an error rather than silently reading nothing (see "Why three
formats").

Trade-off: a glob can make a file a note that is no use as context, such
as a collection or a binary file; `srl` allows it, and gives no text for a
file that isn't UTF-8.

## Why flat rules

Every rule in `srl` can be stated in one sentence, and each one picks the
simplest option even when a smarter one exists: whole days instead of
learning steps, one global new-card limit, one card per group of siblings
per day. The
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
