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
  - **Kind programs served over HTTP, kept running by a daemon of the
    store.** As Claude Code's hooks can be either a command or an HTTP
    endpoint, a kind could be either a process per call, as today, or an
    HTTP server that keeps running between calls. It saves the start of
    a process (and of an interpreter, a dictionary or a model) on every
    call. Claude Code keeps its servers alive because it runs for hours;
    an `srl` command lasts a moment, so a **daemon** of the store keeps
    them instead.

    - **Choosing HTTP.** A kind's entry in `[kinds]` says that it is
      served over HTTP, for example as a table beside today's list,
      `[kinds.ja-words]` with `command = [...]` and `http = true`. It is
      easy to check: the daemon starts the program and asks it for
      `describe`; a program that doesn't answer in time, or exits, doesn't
      support it, and that is an error about the kind, like any program
      that fails (see [cards.md](cards.md), "How srl calls them"). `srl
      kinds` could show how each kind is called. There is no quiet
      fallback to a process per call: a kind is called one way.
    - **The same four calls.** Each call is a request,
      `POST /describe`, `/validate`, `/groups` or `/make`, whose body is
      the same JSONL as on standard input today, and whose answer is the
      same JSONL; status 200 means every line was answered, any other
      status is a failure of the whole call, with its body as the reason.
      So a program can serve both ways with the same functions, and every
      rule of the calls (batches, one line per input, `groups` only for
      what `validate` accepted, the context only for `make`) is unchanged.
    - **The daemon.** It is `srl` itself, as a hidden command such as
      `srl daemon`, so `srl` still runs no program but its own and your
      kind programs. A command that needs an HTTP kind and finds no
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
      and no fallback to a process per call.

    To be decided, besides the names above:

    - **Who may call it.** Any program on the computer can reach a port
      on `127.0.0.1`, and so can a web page, through DNS rebinding (as
      `srl-web` guards against). The daemon runs your programs, some of
      which cost money (an LLM), so each request would carry a token
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
