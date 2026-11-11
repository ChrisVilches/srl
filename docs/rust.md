# Rust in srl

This document explains how the Rust code of `srl` is organized and written.
Unlike the other documents, it does not specify how `srl` behaves. It
describes how the implementation is built, and it applies to anyone working
on the code, people and agents alike.

## Workspace layout

```
Cargo.toml              workspace: members, shared dependencies, lint policy
clippy.toml             clippy settings (test exceptions)
rustfmt.toml            formatter settings
LICENSE-MIT, LICENSE-APACHE  the licenses (MIT OR Apache-2.0); each crate has a copy, for its package
README.md               the repository's and the srl crate's page; srl-core has its own README.md
scripts/check-windows.sh  lints the Windows build from Linux or macOS (see "Platform independence")
examples/               not part of srl, and not built with it (see overview.md, "Specification, guides and examples")
crates/
  srl-core/             the core, as a library, in layers (see "Layers" below)
    src/lib.rs          the public API: re-exports only
    src/error.rs        the core's Error type
    src/config.rs       store.md, "Configuration"; the study day
    src/store.rs        store.md: finding, creating and listing a store and its files
    src/domain.rs       the domain: items, cards, selectors, scheduling, as values
    src/domain/item.rs       items.md: the item model (entries, fields, IDs, status), where an item comes from (`Origin`)
    src/domain/item/object.rs  items.md, "The item object": reading and writing items as JSON objects
    src/domain/card.rs       cards.md: the card format and card IDs
    src/domain/selector.rs   selectors.md
    src/domain/schedule.rs   scheduling.md: replaying the log through FSRS, places in the schedule
    src/domain/stats.rs      cli.md, "Analysis": `srl stats`
    src/domain/random.rs     reproducible random numbers for new IDs and `srl next`
    src/app.rs          the application: what the commands do with the domain and the ports
    src/app/catalog.rs       every item of the store, read from its sources and checked by their kinds; the notes, and the context `see` names
    src/app/write.rs         items.md, "How srl changes items": what `srl id`, `srl accept`, `srl add` and `srl edit` change, checked before any source writes it
    src/app/check.rs         cli.md, "Checking": orphans and the warnings of `srl check`
    src/app/review.rs        log.md and scheduling.md: recording reviews, resets, suspensions
    src/app/next.rs          scheduling.md, "Choosing the next card" and "Siblings"
    src/source.rs       the `Source` port: what every source of items answers, and the store's sources (see "Sources" below)
    src/source/filesystem.rs  formats.md: the `filesystem` source, which picks a format by a file's extension
    src/source/filesystem/jsonl.rs  formats.md, "JSONL"
    src/source/filesystem/json.rs   formats.md, "JSON"
    src/source/filesystem/xml.rs    formats.md, "XML"
    src/source/files.rs      what the formats share: which files, lines, and "How srl writes to collections" (see "Collection editing" below)
    src/source/program.rs    sources.md, "Source programs": a source program, its calls, and checking what it prints
    src/process.rs      running the programs a store names, kind programs and source programs: one process per call
    src/kind.rs         cards.md, "Kinds": the `qa` kind, asking kinds about items and making cards, describing kinds
    src/kind/program.rs      cards.md, "Kind programs": running the calls in batches, keeping their answers, checking what they print
    src/kind/cloze.rs        cards.md, "Cloze deletions"
    src/log.rs          log.md: reading and appending events
    src/index.rs        store.md, "The index cache": `.srl/cache/index.sqlite`
  srl/                  the `srl` binary
    src/main.rs         parse, run, map the result to an exit code
    src/args.rs         clap definitions, no logic
    src/argv.rs         lets selector terms start with `-`
    src/commands.rs     one function per command: gather inputs, call the core, print
    src/output.rs       the only module that prints
    tests/cli.rs        tests that run the real binary, one rule at a time
    tests/scenarios.rs  tests that run the real binary through a person's usual days
  srl-test-kind/        a kind program for the tests (see "Testing"); not published
  srl-test-source/      a source program for the tests (see "Testing"); not published
```

### The core is a library, the CLI is a thin layer on top

`srl-core` holds every rule from the specification: sources, items, cards,
scheduling, the log, selectors and the store. `srl` turns
command-line arguments into calls to `srl-core` and prints the results.

- **`srl-core` does not depend on clap**, and has no other dependency that
  only makes sense for a command line. The workspace split makes this a
  compile-time guarantee rather than a convention.
- **`srl-core` takes its inputs explicitly.** It does not read environment
  variables, the current directory, the clock or standard input on its own.
  The CLI gathers them and passes them in (see `Lookup` in `store.rs`, the
  study day, and the current time `now` that `app/review.rs` stamps events
  with). That keeps every rule testable without a real process environment.
  The one exception is the wait for the log's lock in `log.rs`, measured
  with the monotonic clock (`Instant`); it decides nothing the
  specification describes in terms of time.
- **`srl-core` never prints.** It returns values and errors. Turning them
  into text or JSON is the CLI's job.
- **`srl` contains no domain logic.** If a command needs a decision about
  items, cards or schedules, that decision belongs in `srl-core`, with tests
  there.

### Layers

`srl-core` is in layers, so that the rules of spaced repetition aren't
mixed with where data comes from, as a database sits behind a repository
in Clean Architecture or hexagonal architecture:

- **The domain** (`domain/`): items, cards, the item object, selectors,
  scheduling and statistics, as values. It reads no file, starts no
  process and doesn't know the clock: it is given what it needs.
- **The application** (`app/`): what each command does. It reads the
  sources into the catalog, calls the domain, and asks the ports to
  change things; the rules that cross sources (unique IDs, `see`,
  checking every change before writing) are here.
- **The ports and their adapters**, around them: sources (`source.rs`,
  with the notes' sources and source programs under `source/`), kinds
  (`kind.rs`, with kind programs in `kind/program.rs`), the log
  (`log.rs`) and the index cache (`index.rs`), with the configuration and
  the store (`config.rs`, `store.rs`) and the running of programs
  (`process.rs`).

Within a layer, each area of the specification has its module
(`selectors.md` in `domain/selector.rs`, `store.md`, "The index cache" in
`index.rs`, and so on); one document may be spread over several layers,
as `items.md` is. A module that grows too big gets submodules in a folder
of the same name (`selector.rs` plus `selector/parse.rs`), never a
`mod.rs`.

## After every change

Run these four commands, in this order, after every change and before every
commit:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo install --path crates/srl --locked
```

- **Format** so that diffs only show real changes.
- **Lint** with warnings as errors. Clippy must pass with no warnings.
- **Test** the whole workspace, not only the crate you changed.
- **Install** so that the `srl` on your `PATH` is always the latest build.
  When you develop `srl`, you are also using it, and you should be using the
  code you just wrote. `--locked` builds with the exact versions in
  `Cargo.lock`, which is committed.

A change is finished when all four pass. If one fails, fix it before going
on. Don't skip it, and don't silence a lint to get past it.

### Before a release

A release contains no unfinished code: no `todo!`. It is allowed while
developing, so the normal lint run lets it through; before a release, this
must also pass:

```sh
cargo clippy --workspace -- -D warnings -D clippy::todo
```

It checks only the code that ships (no `--all-targets`, so tests may still
panic).

## Writing code

### Immutable by default

Bind values with `let`, not `let mut`. Build new values instead of changing
existing ones: iterator chains (`map`, `filter`, `find`, `collect`),
`Option` and `Result` combinators, and struct update syntax
(`Lookup { cwd, ..Lookup::default() }`).

Use mutation only when it is the only way, or clearly the simplest way, to
do something. Examples: an API that takes `&mut self` (`clap::Command::build`),
a loop that consumes an iterator by hand, or a buffer filled piece by piece.
Keep it local: a `mut` binding inside one function, never shared mutable
state across modules.

### Private by default

Expose as little as possible. Anything that isn't exposed can be changed
freely later.

- Modules are private (`mod store;`). `lib.rs` re-exports the few items that
  form the public API (`pub use store::{Lookup, Store};`).
- Struct fields are private unless the struct is plain data meant to be
  built by the caller (like `Lookup`). Otherwise give it a constructor and
  accessors (`Store::locate`, `Store::root`).
- Inside the binary crate nothing is public: items shared between modules
  are `pub(crate)`. The `unreachable_pub` lint flags any `pub` that can't
  actually be reached from outside.
- Helper functions stay private to their module.

### Types carry the specification

The specification is mostly closed sets and fixed formats. Write them as
types, so that the compiler checks them:

- **Closed sets are enums**: statuses, places in the schedule,
  ratings, selector terms. A `match` on them lists every variant. Don't use a
  `_ =>` arm on our own enums (the `wildcard_enum_match_arm` lint flags it),
  so that adding a variant shows every place that must handle it.
- **Parse, don't validate.** Input from outside (what sources read, log lines,
  JSON on standard input) is parsed once, at the boundary, into
  types that can only hold valid values. After that, code doesn't check again.
  For example, a note is read once by its source (see "Sources") and
  turned into `Item`s whose entries and fields are known to
  be valid.
- **serde derives are the formats.** A contract's struct, with its
  `#[derive(Serialize, Deserialize)]` and attributes, is the readable
  definition of that format in code.

### Errors

- **Fail loudly** (see [overview.md](overview.md), principle 6). Never
  discard an error. Don't write `let _ = …` on a `Result`, don't turn an
  error into a default with `.ok()`, `.unwrap_or_default()` or
  `.unwrap_or(…)`, don't drop the cause with `.map_err(|_| …)`, and don't
  leave an `Err(_) => {}` arm empty. Propagate errors with `?`. Where the
  specification says to carry on after a failure (an invalid log line,
  every error in every source), return the failure next to the results so the CLI can
  report it on standard error. Never skip it without a word.
- Turning a missing value into a default is fine when the specification
  says so, as with an absent `config.toml` key. Turning a *failure* into a
  default is not.
- `srl-core` has one `Error` enum (with `thiserror`) and a `Result<T>` alias.
  Each variant's message is one plain sentence that says what to do about it,
  because the CLI prints it as is (see [cli.md](cli.md)).
- `srl` uses `anyhow::Result`, adds context where a message would be unclear
  without it (`.context("cannot read the current directory")`), and turns
  errors into exit code 2 in one place (`main.rs`).
- **Errors that can be recovered from, or that the user should hear about,
  are returned.** Bad input, missing files, errors in sources, invalid log lines:
  every one becomes an `Err` that travels up to the CLI, which reports it on
  standard error with what to do about it. This is nearly every error.
- **Where recovering makes no sense, use `expect("…")` with a good
  message.** This is for broken invariants: a case the code itself rules
  out, where an error would only be noise for the caller (a digit below 36
  that must index a 36-character table, an endless iterator that must yield).
  The message says what was expected, in the "should" style:
  `.expect("a digit below 36 should index ID_CHARS")`. If it ever fires, it
  is a bug in `srl`, and the message says which assumption broke. If you're
  unsure which kind an error is, return it.
- **Outside tests, never `unwrap()`.** At minimum use `expect()` with a
  message. For the same reason, never index or slice in a way that can panic
  without a message (`v[i]`, `&s[a..b]`): use `get`, `split_at_checked`,
  `strip_prefix` and similar, and `expect` on the result if it truly can't
  fail. Don't write `panic!`, `unreachable!` or `unimplemented!` either:
  `expect` covers the cases that can't happen.
- **`todo!("…")` marks code that isn't written yet**, during development.
  It says what is missing, and must be gone before a release (see "Before a
  release"). A whole command that isn't written yet doesn't panic: it
  returns an error saying so, because users run it.
- `main` returns `ExitCode`. Never call `std::process::exit`.

### Style

- **Let the compiler infer types** where the code reads fine without them:
  `let terms = …collect::<Vec<_>>()` or `let store = Store::locate(&lookup)?`.
  Write the type when it helps the reader.
- **End functions with a tail expression**, not `return x;`. Use `return`
  only for early exits.
- **Small functions with plain names** for each step. A function that needs
  a comment to explain its parts should probably be split.
- **Doc comments (`///`) on every item that isn't obvious**, saying what it
  is or what it promises, and pointing to the specification where it
  implements one (`see docs/store.md, "Finding the store"`). Ordinary
  comments explain *why*, never *what*.
- **No `unsafe`**, anywhere. The lint is set to `forbid`, so it can't be
  allowed locally either.
- **Nothing deprecated**: no deprecated functions, macros or attributes, and
  no deprecated or unmaintained crates. If the compiler or a crate marks
  something as deprecated, replace it in the same change.
- **Use the current edition** (2024) and a stable toolchain no older than
  the workspace's `rust-version` in `Cargo.toml`. Cargo refuses to build
  with an older one, and resolves dependencies to versions that support it.
  There is no `rust-toolchain.toml`: `cargo install` warns whenever a
  toolchain file chooses the toolchain, and the commands above should run
  without warnings. Moving to a newer stable release means raising
  `rust-version`.

### Platform independence

`srl` must work the same on Linux, macOS and Windows. Write code that doesn't
depend on the platform, and flag every place that does.

- **Paths:** build them with `Path::join`, never by gluing strings with `/`
  or `\`. Collections' names and the paths of files are `String`s with `/` between
  folders, because they are part of the formats (JSON output, `see`);
  turn such a path into a file path only with
  `Store::file`.
- **Platform locations** come from a crate, never from hard-coded paths such
  as `~/.local/share`: the default store uses `dirs::data_dir()`.
- **Finding programs** uses `which`, which follows `PATHEXT` on Windows.
  Running a bare name with `std::process::Command` only finds `.exe` there.
- **Text files** `srl` creates are written as UTF-8 with `\n`, and read
  with either `\n` or `\r\n`. A collection `srl` changes keeps its own line
  endings (see "Collection editing").
- **Tests** must not assume a platform either: build paths with `join`, and
  don't rely on Unix-only tools.

When something can't be platform-independent yet, mark it in both places:

1. a comment starting with `TODO(platform):` right above the code, saying
   what differs and what to decide;
2. a note starting with **TODO (platform):** in the specification document
   that describes the behaviour, or in the table below if it only concerns
   the code.

`grep -rn 'TODO(platform)\|TODO (platform)' crates docs` lists them all.

None of this applies to `examples/` (see [overview.md](overview.md),
"Specification, guides and examples"): an example that works on some
platforms only says so in a comment at its top, and needs no
`TODO(platform):`. Design decisions still to take are marked
**TODO (design):** instead (see [overview.md](overview.md), "Open
questions").

Code behind `cfg(windows)` isn't compiled on Linux or macOS, so it can
break unnoticed. `scripts/check-windows.sh` runs clippy for a Windows
target from Linux or macOS (it needs `rustup`; the C compiler for SQLite is
replaced by a stub, since nothing is linked). Run it after changing such
code, including the Windows helpers in tests. It shows that the code
compiles and passes the lints, not that it works: that still needs a
Windows machine.

Known places:

| Where | What differs | Status |
|---|---|---|
| `store.rs`, `note_paths`, `nfc`; [store.md](store.md) | macOS often lists file names in Unicode NFD, so paths could sort and match differently | Handled: paths are sorted and matched against `in:` and `exclude` in NFC, on every platform |
| `source/files.rs`, `replace_file`; [formats.md](formats.md) | Replacing a collection atomically: a rename over an existing file, which Windows refuses while another program holds the file open without sharing | Handled: the rename's error is reported and the file left as it was. **TODO in code and spec**: not run on Windows |
| `log.rs`, `lock`; [log.md](log.md) | Concurrent appends are serialized by an OS lock on `.srl/log/.lock` (`File::lock`: `flock` on Unix, `LockFileEx` on Windows). On network file systems locks depend on the server, and may fail or not be shared between machines | Handled on local disks, the same on every platform. **TODO in code and spec** for network file systems: a lock that fails makes the append fail |
| `process.rs`, `Running`, `stop_programs`; `srl`'s `main.rs`; [cards.md](cards.md) | Stopping a program with every process it started: a process group and signal handlers on Unix, a job object on Windows | Handled. **TODO in code and spec**: on Unix, `SIGKILL` can't be caught; on Windows, a process started before the program is put in its job escapes it, and it is not run on Windows |
| `index.rs`, `connect`; [store.md](store.md) | SQLite's file locking is unreliable on network file systems (NFS, SMB) on every platform: concurrent updates could damage the index cache | **TODO in code and spec**: damage is usually detected and the cache rebuilt; keep stores on a local disk for now |

## Testing

Write tests for every module that has logic, which is nearly all of
`srl-core` and the parts of `srl` that do more than pass values along.

- **Unit tests** go in the same file, in a `#[cfg(test)] mod tests` at the
  bottom. They can test private functions directly.
- **Integration tests** in `crates/srl/tests/` run the real binary
  (`env!("CARGO_BIN_EXE_srl")`) and check what a user or script sees: exit
  codes, standard output and standard error. `tests/cli.rs` checks
  commands one rule at a time; `tests/scenarios.rs` follows what a person
  usually does over several days (adding items, studying as a frontend
  does, coming back the next day), one ordinary case per test.
- **Kind programs are tested with `srl-test-kind`**, a small Rust program
  whose behaviour is set by the fields of the items it gets (their errors,
  cards and groups, failures of a card or of a whole call, slowness,
  invalid output, and a log of its calls; see its `main.rs`), and, for
  `describe`, which has no item, by an argument before the call. It needs
  no interpreter, so the tests run the same on every platform. Tests find
  it next to their own binary, where `cargo test --workspace` builds it;
  with `cargo test -p srl` alone it isn't built, and those tests say so.
- **Source programs are tested with `srl-test-source`**, a small Rust
  program that keeps its items in a JSON file and whose settings, its keys
  in `[sources]`, make it fail a call, print given output, refuse every
  change, or claim changes it doesn't make, and log its calls (see its
  `main.rs`). With `url`, it keeps its items on a small web service
  instead, which a test starts on `127.0.0.1` with the standard library:
  it checks a token, answers with an `ETag`, and refuses a stale `PUT`, so
  that the test can count the requests a command makes and see the
  program's own cache save them, with no network. Tests find it next to
  their own binary, as `srl-test-kind`.
- **Integration tests control the time and the random numbers** with
  `SRL_NOW` and `SRL_SEED`, setting `TZ` too when the time zone matters
  (see [cli.md](cli.md), "Conventions"). Tests that span several days set
  `SRL_NOW` for each command instead of waiting or editing the log.
  `srl_command()` in `tests/cli.rs` removes the `SRL_` variables, so the
  environment of whoever runs the tests changes nothing.
- **Tests that need files** create a temporary directory with `tempfile` and
  build exactly the tree they need. They never touch a real store, the home
  directory or the current directory.
- **Name tests after the rule they check**, as a sentence:
  `nearest_parent_with_config_is_found`, `rating_must_be_between_1_and_4`. A
  failing test name should say which rule broke.
- **When fixing a bug, first write a test that fails because of it.**
- `unwrap`, `panic!` and indexing are allowed in tests (see `clippy.toml`).
  In helper functions outside a `#[test]` function, add
  `#[expect(clippy::unwrap_used, reason = "…")]`.

The specification's examples are good test cases. If a document says
`srl find in:japanese -in:n5` excludes N5, there should be a test
showing that it does.

## Dependencies

Use a crate when writing the code yourself would be cumbersome or easy to
get wrong. Write it yourself when the functionality is small. Every
dependency is code we didn't review, compile time and something that can be
abandoned or deprecated, so it has to be worth it.

**Use a crate** for standard, well-specified problems where a maintained
crate is the usual choice: argument parsing, JSON and TOML parsing,
SQLite, Unicode normalization, FSRS, temporary files.

**Write it yourself** when the code fits in a screen and has no tricky edge
cases. Examples: joining selector terms, finding the store directory,
parsing the item syntax, mapping outcomes to exit codes. Running the
binary in integration tests needs no crate either, because
`std::process::Command` is enough.

Before adding a crate, check that:

- it is maintained, widely used, not deprecated, and has no maintained
  successor you should use instead;
- you use its latest version (`cargo search NAME`);
- you enable only the features you need.

Declare every dependency once, in `[workspace.dependencies]` in the root
`Cargo.toml`, and refer to it from a crate with `NAME.workspace = true`.
Dependencies used only by tests go in `[dev-dependencies]`.

Current dependencies:

| Crate | Used by | Why |
|---|---|---|
| `clap` (derive) | `srl` | Argument parsing, help and usage errors |
| `anyhow` | `srl` | Error type with context in the CLI |
| `getrandom` | `srl` | The random seeds for new item IDs and for `srl next`'s pick (the core derives both from them), unless `SRL_SEED` gives one |
| `dirs` | `srl` | The platform's data directory, for the default store |
| `which` | `srl-core` | Finding kind programs and source programs on `PATH`, with `PATHEXT` on Windows |
| `rustix` (`process`) | `srl-core` on Unix; tests of `srl` on Unix | Stopping a program's process group, without `unsafe` (see [cards.md](cards.md), "Stopping a program"); sending `SIGTERM` to `srl` in tests |
| `signal-hook` (`iterator`) | `srl` on Unix | Stopping the programs `srl` runs when it gets `SIGINT`, `SIGTERM` or `SIGHUP`, then ending by the same signal |
| `win32job` | `srl-core` on Windows | The job object each program runs in, which stops its processes when `srl` ends, without `unsafe` |
| `jiff` | both | The study day: the local date and time zone (CLI), dates (core) |
| `toml` | `srl-core` | Parsing `.srl/config.toml` |
| `quick-xml` | `srl-core` | Reading XML collections, with the byte position of each element, so that a change writes only its item's element |
| `fsrs` | `srl-core` | FSRS scheduling, from the algorithm's authors. Its optimizer brings `rayon` and `ndarray` along, unused |
| `sha2` | `srl-core` | The `hash` of review events |
| `unicode-normalization` | `srl-core` | NFC for comparing collections' names and the paths of files |
| `thiserror` | `srl-core` | Deriving the core's `Error` |
| `globset` | `srl-core` | The `include` and `exclude` globs of sources and `[context]` |
| `serde` (derive), `serde_json` (`preserve_order`, `float_roundtrip`) | both | The JSON formats; fields keep their order, and numbers read back from the index cache are exactly the ones written |
| `rusqlite` (`bundled`, `cache`) | `srl-core` | The index cache. `bundled` compiles SQLite in (a C compiler is needed to build), so no system library is required; `cache` reuses prepared statements |
| `blake3` | `srl-core` | Hashing every file on every use for the index cache. Several times faster than SHA-256 on CPUs without SHA instructions |
| `tempfile` | `srl-core`; tests of both crates | The temporary file a changed collection is written to before it is renamed over it; temporary store trees in tests |

## Linting and formatting

The formatter is `rustfmt` with default settings for the 2024 edition. The
linter is clippy, configured in `[workspace.lints]` in the root `Cargo.toml`
so that every crate follows the same policy (each crate opts in with
`[lints] workspace = true`).

The policy is strict where strictness catches real problems, and leaves
ordinary idiomatic Rust alone:

| Setting | Why |
|---|---|
| `unsafe_code = "forbid"` | No unsafe code, without exceptions. |
| `unreachable_pub`, `missing_debug_implementations`, `rust_2018_idioms`, `unused_qualifications` | Keep visibility tight and code idiomatic. |
| clippy `pedantic` | Catches many small quality issues. |
| …except `module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc` | They add noise and little value in this codebase. |
| `unwrap_used`, `panic`, `unreachable`, `unimplemented`, `indexing_slicing`, `string_slice`, `dbg_macro` | No unexplained panics or leftovers outside tests: a panic that can't be avoided is an `expect` with a message (see "Errors"). `todo` is left off so that `todo!` can mark unwritten code during development; "Before a release" checks it. |
| `let_underscore_must_use`, `map_err_ignore`, `unused_result_ok` | Errors are never discarded (fail loudly). |
| `print_stdout`, `print_stderr` | Only `output.rs` prints, with an `#[expect]`. |
| `exit` | Exit codes come from `main`'s return value. |
| `wildcard_enum_match_arm` | Matches on enums list every variant. |
| `allow_attributes`, `allow_attributes_without_reason` | Use `#[expect(lint, reason = "…")]`. It explains itself and warns when it is no longer needed. |
| `mod_module_files` | Modules are `name.rs`, never `name/mod.rs`. |
| `clone_on_ref_ptr`, `str_to_string`, `if_then_some_else_none`, `rest_pat_in_fully_bound_structs`, `verbose_file_reads`, `undocumented_unsafe_blocks` | Small idioms that make code clearer. |

These are deliberately **not** enabled: `implicit_return` (tail expressions
are fine), `missing_docs_in_private_items`, `arithmetic_side_effects`,
`pattern_type_mismatch`, and the rest of the
`restriction` group, which conflicts with itself and with normal Rust.

Silencing a lint is an exception, not a way around a failing check. When it
is really justified, use `#[expect(clippy::…, reason = "…")]` on the
smallest item possible, never `#[allow]` and never a crate-wide exception.

Clap turns doc comments into `--help` text, where the backticks that
`doc_markdown` asks for would show literally. For help text that names
things like `SRL_DIR`, use the `help = "…"` attribute instead of a doc
comment.

## Sources

Where items come from, and how they are written back, is behind one
boundary: the trait `Source` in `source.rs`. The rest of `srl-core` gets
items from it, with where they came from, and asks it to change them; it
knows nothing of files, notes or how items are written. A source is an
implementation of the trait, and nothing outside a source's own module
knows how it keeps its items.

| Method | Gives |
|---|---|
| `collections()` | The names of the source's collections, by which `srl` names them everywhere. A file of the `filesystem` source is a collection, named by its path. |
| `read()` | Every item, with its collection, its handle (what names it to the source when it changes) and its entries and fields; and every problem found, each at its collection. |
| `refuses(collection)` | Why `srl add` can't create a collection, or nothing, by the source's own rule: `filesystem`'s globs and formats, or a source program's `accepts`. |
| `write(changes, dry_run)` | Makes a list of changes: `Add` an item to a collection, or `Replace` the item of a handle, read as `before`, with `after`. Every change is made and checked before anything is written; a change refused is named by its place. |
| `describe()` | What the source is, in a sentence, and the settings it shows, each by its name, for `srl sources`. |

- **The catalog holds every item in memory** (`app/catalog.rs`, the type
  `Catalog`). It reads every source, turns what they read into `Item`s,
  each with its `Origin` (source, collection, handle, its position in its
  collection, and the item as read), checks them with their kinds, and
  keeps the rules that cross sources: IDs unique in the store, no
  collection's name given by two sources, `see` naming a note, collections
  and items in the order of the collections' names. Everything else works
  on the catalog, never on a source.
- **A collection is named by its source**, and by that name everywhere
  outside its source.
- **Context is not a source's business.** The catalog checks that `see`
  names a note, a file `[context]` allows (`Notes` in `app/catalog.rs`),
  and reads its text when it is shown. No source shows anything, so no
  source knows another's rules.
- **Changes give back the item as it was read.** `app/write.rs` decides what
  changes and checks it (item objects, IDs, kinds, `see`), then gives
  each source its changes; a `Replace` carries the item as read, and the
  source refuses it when the item is no longer that.
- **New items are checked without a source.** `srl add` and `srl edit`
  check their items' entries and fields with the kinds directly
  (`catalog::check_new`), so an item is judged the same whatever
  collection it goes to.

### The filesystem source

The collections of a store are read by the `filesystem` sources of
`[sources]` (see [sources.md](sources.md)), `source/filesystem.rs`, which
picks a format for each file by its extension (see
[formats.md](formats.md)): `filesystem/jsonl.rs`, `filesystem/json.rs`
and `filesystem/xml.rs`. `Sources::new` in `source.rs` builds the sources
from the configuration.

- **Each format is its own module, whole**: it parses its files into
  items, and makes a list of changes to a file's text, keeping what it can
  of the rest. There is no trait for formats: `filesystem.rs` dispatches on
  an enum, `Format`, and each format calls its own functions.
- **What doesn't depend on a format is shared**, in `source/files.rs`:
  which files a source reads (`CollectionFiles`, its globs), splitting a
  file into lines, finding the item a change is about, and writing a file
  safely (see "Collection editing").
- **The item object** (see [items.md](items.md), "The item object") is
  read and written by `domain/item/object.rs`, for JSONL and JSON
  collections, source programs and the standard input of `srl add` and
  `srl edit`; `object::from_pairs` reads one from keys and values, which
  XML gives too. `Item::json` writes it for `srl show --json`.
- **Likely mistakes are the core's** (`app/check.rs`): the warnings of
  `srl check`, in the valid items of every source. A format's mistakes are
  errors of its parse.

### Source programs

A source can be a program (see [sources.md](sources.md), "Source
programs"), `source/program.rs`: the same port, answered over standard
input and output, one process per call, as `process.rs` runs kind
programs too.

- **Every line it prints is parsed strictly**, and any output that
  doesn't follow the protocol is a failure of the whole call, as
  `Error::SourceFailed`.
- **Its changes are verified by reading it again**: `write` reads the
  program before and after the call, and compares the items with what
  the changes should give, since `srl` can't check the program's own
  storage as it checks a file it parsed itself.
- **It is tested with `srl-test-source`**, a source program for the tests
  (see "Testing").

### Items are named by their IDs

**Nothing in `srl` names an item by where it is in its collection.** An
item is its collection and its ID:

- **Changes find the item again in the collection as it is right before
  the change**: by its ID, or, for `srl id`, by its handle, its position
  among the collection's items (see "Collection editing").
- **Messages name an item** as `COLLECTION: item ID`, or, for an item read
  without a valid ID, by its position, `item N` (`Spot` in `error.rs`).
  A format that can say where in a file a problem is says it in the
  message, such as `line 3:` for JSONL and XML, as plain text.
- **The order of items** is the order a source reads them in.

## Collection editing

Collections may be written by hand, as XML is meant to be, so `srl` keeps
a file as close to how it was as its format allows. The formats change
files (see [formats.md](formats.md), "How srl writes to collections") by
these rules, with the helpers of `source/files.rs`:

1. **It changes as little as the format allows.** JSONL writes the line
   of an item again; XML writes the element of an item again, in place,
   by its byte range, and inserts new elements before `</items>`; JSON
   writes the whole array again, since no part of it is an item's alone.
   In JSONL and XML every other byte of the file stays the same,
   including a UTF-8 BOM, `\r\n` line endings and a missing final newline;
   new text gets the line ending of the file's first line, or `\n` in a
   file without one.
2. **It finds each item in the collection as it is now.** It reads the
   file again, and finds the item by its ID, or, for `srl id`, by its
   handle, its position. An item that is gone, or no longer as it was read
   (the `before` of the change), or a file that has an error now, means the
   file changed since it was read: the command stops, and says so.
3. **It verifies its own work.** After editing, it parses the new text and
   compares the items with the old parse: they must be equal except for
   the change it meant to make. If not, the edit is abandoned and the file
   is left untouched.
4. **It checks the file didn't change underneath it**, by comparing the
   bytes it read with the bytes on disk right before writing.
5. **It writes atomically**: to a temporary file in `.srl/tmp/`
   (`tempfile`), then a rename over the file. Keeping it in `.srl/` keeps
   the rest of the store clean, and lets `.srl/.gitignore` ignore it (see
   [store.md](store.md), "Version control and syncing").
6. **It has thorough tests**, including CRLF line endings, a UTF-8 BOM, no
   trailing newline, XML comments and indentation kept, fields that must be
   written as CDATA, non-ASCII text around the edited item, and a file
   changed between reading and writing. Each format tests its own edits,
   and `app/write.rs` the commands that change items.


## The command line

- `args.rs` describes the commands with clap's derive API and holds no
  logic. Options shared by many commands (`--json`, `--dry-run`, the
  selector) are small `Args` structs flattened into each command.
- Selector terms may start with `-` (`-in:a`), which clap would read as an
  option. `argv.rs` applies one rule before clap parses: on a command that
  takes a selector, every argument that is not one of that command's options
  (or an option's value) is a selector term. It moves the options first and
  the terms after `--`. Arguments after a `--` given on the command line
  are terms. The option list comes from the clap definition, so nothing is
  duplicated.
- `commands.rs` matches on the command, gathers inputs from the environment,
  calls `srl-core` and passes the result to `output.rs`.
- Unknown commands are usage errors, reported by clap.

## The index cache

`index.rs` keeps `.srl/cache/index.sqlite` (see [store.md](store.md), "The
index cache"). It must never change a result, only skip work; the one
exception, by design, is a kind program changed in place, whose kept
answers stay in use until `srl check` or until the cache is deleted:

- **One code path for both.** New log lines go through the same
  `Replayer::apply` as a full replay. Don't add a second way to compute
  anything the cache stores.
- **Commands read memories through `Index`**, not `Log::load`, where
  speed matters. `Log::load` and `History::replay` stay as the
  reference that the cache is tested against, and as the fallback when the
  cache can't be used.
- **Bump `SCHEMA_VERSION`** whenever the tables, or what a stored value
  means, change. The cache is also rebuilt when the crate version changes,
  but during development that version stays the same.
- **Tests compare with and without the cache**: `assert_same` in
  `index.rs` after every kind of change to the files, and
  `the_index_cache_never_changes_what_commands_print` in `tests/cli.rs`.
  A new command or a new kind of change to the store gets a case there.
- **Kind answers are kept as printed.** `Programs::answers` in
  `kind/program.rs` keeps each line a kind program printed, and a kept
  line goes through the same parsing as a fresh one, so it gives the same
  items and the same messages. Whether a call runs at all is decided by
  `kind::Cache`: `Catalog::load` uses the kept answers, `Catalog::load_asking`
  (`srl check`) asks again, and `catalog::check_new` (checking items about to be
  written) keeps nothing.
