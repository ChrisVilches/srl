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
  prerequisites in notes, by hand or by a program, is then work on top.
- **Partial credit for prerequisites.** A correct answer about a card also
  counts, partially, for the cards it requires. Needs a scheduling rule.

## Kind programs

- **Ways to run kind programs, chosen by their author.** Today there is
  one: `srl` starts a process for every call, and each call is a batch
  over standard input and output (see [cards.md](cards.md), "How srl
  calls them"). Other **modes** could be offered beside it, each with the
  same four calls and the same answers, so that a program's author picks
  the one that suits the program.

  Not every mode is about speed. Some would make programs faster (a
  server kept running), others easier to write (one item per process),
  at a cost in speed the author accepts knowingly. Batches stay the
  default, and the mode every example uses.

  - **The user configures the mode, `srl` never asks the program.** A
    kind's entry in `[kinds]` says how to run it, for example as a table
    beside today's list: `[kinds.ja-words]` with `command = [...]` and
    `mode = "batch"`, `"item"` or `"http"`. Asking the program how it
    wants to be run would start it on every command, which is what kept
    answers avoid; it is the same reason `srl` doesn't ask a program for
    its version and you run `srl check` after changing one instead (see
    [rationale.md](rationale.md), "Why answers of kind programs are kept
    under the kind and its command"). So the author documents which
    modes the program supports and gives an example of its `[kinds]`
    entry, and the user copies it. A program run in a mode it doesn't
    support fails like any program that fails, and is reported as a
    problem of the program; there is no quiet fallback to another mode:
    a kind is run one way.
  - **A mode is part of what answers are kept under**, with the kind's
    name and command, so that changing it asks again about every item.

  Candidates:
  - **One item per process**, for programs that are easier to write. The
    program gets one item on standard input and prints one answer, with
    no loop and no reading up to the end of the input. A batch program is
    already one loop around the same functions, but this removes even
    that, which suits a first program, a prototype or a small store.

    The cost is a process start for every item, which kept answers hide
    most of the time but not always. Measured with a minimal Python
    program (about 20 ms to start): with every answer kept, both modes
    take the same few milliseconds; with none kept, 1,000 items take about
    48 seconds instead of about 60 ms, and 5,000 items several minutes.
    No answer is kept after deleting the cache, a new version of `srl`, a
    fresh clone of the store or a new computer (the cache is left out of
    version control), and `srl check` asks about every item every time. `srl
    cards` and `srl add`, which ask about many items at once, are 10 to
    75 times slower too. So a program's author would choose it for its
    simplicity, and say in its documentation what it costs. Since the
    four calls and their answers are the same, a program can move to
    batches later without changing any card.

    This mode is postponed too: batches are enough while there are few
    programs and authors.
  - **Kind programs served over HTTP, kept running by a daemon of the
    store.** As Claude Code's hooks can be either a command or an HTTP
    endpoint, a kind could be either a process per call, as today, or an
    HTTP server that keeps running between calls. It saves the start of
    a process (and of an interpreter, a dictionary or a database) on every
    call. Claude Code keeps its servers alive because it runs for hours;
    an `srl` command lasts a moment, so a **daemon** of the store keeps
    them instead.

    This mode is postponed: kept answers removed most of what it would
    save, so it waits for a program that is slow to start, such as one
    that loads a large dictionary or database before every `make`.

    - **Choosing HTTP.** A kind's entry in `[kinds]` says `mode = "http"`
      (see above). A program that doesn't serve it shows at once: the
      daemon starts the program and asks it for `describe`; a program
      that doesn't answer in time, or exits, is an error about the kind,
      like any program that fails (see [cards.md](cards.md), "How srl
      calls them"). `srl kinds` could show how each kind is run.
    - **The same four calls.** Each call is a request,
      `POST /describe`, `/validate`, `/groups` or `/make`, whose body is
      the same JSONL as on standard input today, and whose answer is the
      same JSONL; status 200 means every line was answered, any other
      status is a failure of the whole call, with its body as the reason.
      So a program can serve both ways with the same functions, and every
      rule of the calls (batches, one line per input, `groups` only for
      what `validate` accepted, the context only for `make`) is unchanged.
    - **The daemon.** It is a program that comes with `srl` (which one is
      still to be decided, see below), so `srl` still runs no program but
      its own and your kind programs. A command that needs an HTTP kind
      and finds no
      daemon for its store starts one, detached, and waits until it
      answers. The daemon listens on `127.0.0.1`, on the store's port
      (set in the configuration, for example `[daemon] port`, since each
      store has its own `[kinds]`), and passes each request to the kind's
      server, which it starts on the first request, on a port of its
      choosing. It stops a server after a while without requests to it,
      and ends itself once no server is running and no request has come
      for a while (for example `[daemon] idle`, in seconds).
    - **One daemon per store.** The daemon holds a lock on a file in
      `.srl/daemon/` for as long as it runs (`flock`; `LockFileEx` on
      Windows): a second daemon that can't take the lock ends at once, so
      two commands that start one at the same moment end up with one,
      and both wait for its port. The lock is the truth; a file with the
      daemon's process ID is only for people and tools, since it can go
      stale.
    - **The port is the store's, and a daemon that can't start is an
      error.** The port is set in the store's configuration, so each
      store has its own, and nothing is chosen behind your back. `srl`
      talks to a port only while its own store's lock is held, so it
      never mistakes another store's daemon, or another program, for its
      own: with no daemon of its store, it starts one. If that daemon
      can't start (the port is taken by another store given the same
      port, or by another program; the lock file can't be written; the
      program can't be detached), the command fails with the reason, as
      when a kind program can't be started, and you fix it, for example
      by giving the store another port. There is no retry on another port
      and no fallback to another mode.

    To be decided, besides the names above:

    - **Which program is the daemon.** `srl` itself, as a hidden
      command, needs no second program to install, but a command in
      `srl`'s own list invites people to run it by hand, and `srl`
      starting itself again as a daemon is unusual. A separate program
      shipped beside `srl` avoids both, at the cost of finding it and
      keeping the two the same version.

    - **Who may call it.** Any program on the computer can reach a port
      on `127.0.0.1`, and so can a web page, through DNS rebinding (as
      `srl-web` guards against). The daemon runs your programs, some of
      which cost money (a paid web service), so each request would carry a token
      from a file in `.srl/daemon/` that only you can read, and the
      daemon would refuse requests for another host.
    - **Changes while it runs.** When `[kinds]` changes, or a program is
      updated, a running server is stale. `srl` could send each request
      with the kind's command, so that the daemon starts the server again
      when it changed; a program updated in place can't be noticed that
      way, so the idle time bounds how long a stale server lives, and a
      command to stop the daemon (`srl daemon stop`) is wanted.
    - **Time limits.** `make` keeps its limit, `card_timeout` per card,
      and a server that runs out of it must be stopped, since it may
      still be working. `validate` and `groups` have no limit today; a
      server that hangs would hang every command, so they need one too.
    - **State between requests.** A server that keeps running can keep
      state, so card IDs could depend on earlier requests. The rule stays
      the same (`groups` depends on the fields alone), but it becomes
      easier to break by accident.
    - **Detaching and cleaning up.** The daemon starts in a session of its
      own (`setsid`; `DETACHED_PROCESS` on Windows), with its output in a
      log in `.srl/daemon/`, waits for every server it started so none is
      left a zombie, and stops them all when it ends; servers should also
      die with it if it is killed (a process group, or a job object on
      Windows). Each of these differs between Linux, macOS and Windows,
      so it needs a design for each (see [rust.md](rust.md), "Platform
      independence").
    - **The store's layout.** `.srl/daemon/` is new, and, like the index
      cache, belongs to one computer: it is left out of version control
      and syncing (see [store.md](store.md), "Version control and
      syncing").
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

- **Item syntaxes for other file formats, as plugins.** Items are written
  in srl blocks, which are fenced code blocks: Markdown's syntax, which
  also suits the formats built on it (MDX, Quarto), but not files where a
  fenced block means nothing, such as Org, AsciiDoc, reStructuredText or
  the comments of source code; or in JSONL notes, which hold nothing but
  items. `srl` is not meant to tie you to Markdown, even though Markdown
  is what makes the most sense for most notes. A
  program could take over the item syntax for the notes matched by a glob,
  as kind programs take over making cards: find the items in a note (their
  lines, header entries and fields), say which lines change for `srl id`,
  `srl accept`, `srl add` and `srl edit`, give the context, and look for
  likely mistakes for `srl lint`. Everything after reading the items
  (kinds, scheduling, the log) would stay the same. In `srl`, these are
  already the functions that each of the two built-in syntaxes provides
  (see [rust.md](rust.md), "The item syntax"), so a program would be a
  third implementation of them. Since those functions only say which lines
  change, `srl` would still check and write every change itself, keeping
  the guarantees of [notes.md](notes.md), "How srl writes to notes". It
  needs a plugin protocol of its own, much like the one of kind programs,
  so it is postponed.

- **Items without a line.** An item's line is only shown to people
  (`PATH:LINE`, `line` in JSON), and nothing finds an item by it (see
  [rust.md](rust.md), "Lines are for people"). Some syntaxes can't say
  well where an item starts: in a pretty-printed JSON document, an item
  could start on the line of its `{` or of its first key. Such a syntax
  could give no line, or `srl` could drop lines altogether, and leave
  finding an item to its ID: `srl show ID` says which note it is in, and
  each syntax finds it there its own way. `PATH:LINE` would become `PATH`
  where there is no line, and `line` in JSON `null`; a person opening the
  note would search for the ID.

  Changes would need the same thought: today every change a syntax makes
  is a range of whole lines replaced, which fits syntaxes whose items have
  lines of their own, but not one where items share a line (see
  [rust.md](rust.md), "What still assumes lines"). Two ways out: a rule
  that every syntax keeps each item on lines of its own and can name the
  line it starts on, which keeps everything as it is and rules some
  formats out; or changes as byte ranges, and lines made optional, which
  lets any format in and takes the line out of `PATH:LINE` for some
  notes.

- **Blank space before added items** (low priority). In a note of srl
  blocks, `srl add` puts a blank line between the note and the items it
  appends, and keeps every byte already in the note. A note that holds
  only blank lines, such as
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

## Performance

- **Reading every note on every command may become slow in a very large
  store.** Every command that reads items reads and parses every note
  (see [store.md](store.md), "The index cache"), so its cost grows with
  the store. Today it is negligible: a store of 40 notes takes a few
  milliseconds for a whole command. If a store grows large enough for it
  to be felt, it needs measuring first (reading, hashing or parsing), and
  then an optimization that keeps every result the same. Directions
  to start from, none analyzed: keeping parsed items per note, keyed on
  its content; reading notes in parallel; trusting modification times or
  watching files, which `srl` doesn't do today.
