# Ideas

Changes to `srl` that are not part of the specification yet. They are
recorded here so they are not lost, not as promises. Each would change
`srl`'s data, commands or contracts; ideas for things built on top of
`srl` as it is are in [guides/ideas.md](guides/ideas.md). An idea becomes a
design decision to take, listed in [overview.md](overview.md), "Open
questions", once it is planned.

## Knowledge structure

- **Prerequisites.** Items could name the items they require, for example
  in a header entry. A new header entry changes the item syntax. Finding
  prerequisites in notes, by hand or by an agent, is then work on top.
- **Partial credit for prerequisites.** A correct answer about a card also
  counts, partially, for the cards it requires. Needs a scheduling rule.

## Kind programs

- **Ways to talk to kind programs, chosen by their author.** Today `srl`
  starts a process for every call, and each call is a batch over
  standard input and output (see [cards.md](cards.md), "How srl calls
  them"). Other ways could be offered beside it, each with the same four
  calls and the same answers, so that a program's author picks the one
  that suits it, and `[kinds]` says which one a kind uses. Candidates:
  - **A program that keeps running for a while.** `srl` starts the
    program detached the first time a command needs it, and talks to it
    over a socket file in a folder of its own, with a file holding its
    process ID: one process per kind at a time. The program ends by
    itself after some time without requests, and `srl` starts it again
    when it is needed and isn't running; since it lives for some seconds,
    commands run one after another find it still there. It saves the
    start of a process (and of an interpreter, a dictionary or a model)
    on every call.

    The program needn't manage any of this itself: a **supervisor**
    that comes with `srl` can. `srl` starts the supervisor detached; the
    supervisor starts the kind program as its child, holds the process ID
    and the socket, passes each request it gets on the socket to the
    child and its answers back, and, when no request has come for a
    while, stops the child and ends itself. Ending, locking and cleaning
    up are then written once, in the supervisor, not in every program,
    and a program only has to answer requests as they come, over its
    standard input and output, flushing each answer (as in the session
    below), instead of reading to the end of its input. The supervisor
    could be a hidden command of `srl` itself or a program of its own.

    To be decided: how long the supervisor (or the program) waits before
    ending, and whether a store can set it, where the folder is
    (under `.srl/`), how two `srl` commands starting it at once agree on
    one process (a lock on the folder, with `flock`), how `srl` notices a
    program that died or hangs and cleans up after it (a stale process ID
    or socket, zombie processes), and how a call is still a batch and
    still has the same time limits. Sockets and file locks differ between
    Linux, macOS and Windows (Unix sockets and `LockFileEx` on Windows),
    so it needs a design for each.
  - **A session over standard input and output**, as language servers
    do: one process per command, asked line by line, answering each line
    before it reads the next. Saves a process per call, but a program that
    doesn't flush its output hangs `srl`, so every call needs a time
    limit.
  - **Programs run inside `srl`**, in an embedded language such as Lua or
    Starlark, for `describe`, `validate` and `groups`: no process at all,
    and, in a sandbox without files, clock or network, card IDs that can't
    depend on anything but the fields. `make` needs the world (APIs,
    other programs), so it would stay a process.

## Writing notes

- **Blank space before added items** (low priority). `srl add` puts a
  blank line between a note and the items it appends, and keeps every
  byte already in the note. A note that holds only blank lines, such as
  an "empty" note an editor saved as one newline, is not empty to it, so
  the items start after blank lines; a note that ends in several blank
  lines keeps them all, plus one. Either rule would fix it: a note with
  nothing but blank space counts as empty and the items replace it, or
  trailing blank lines are trimmed so that exactly one blank line comes
  before the items. The second covers both cases, but rewrites blank
  space at the end of the note (see [notes.md](notes.md), "How srl writes
  to notes").

## Stores

- **Export and import between stores.** Move or copy notes from one store to
  another together with the review history of their cards, so that their
  memory state comes along. It needs rules for ID clashes and for merging
  logs. Copying a note by hand moves only the content.
