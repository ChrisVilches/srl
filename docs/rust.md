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
  srl-core/             the core, as a library
    src/lib.rs          the public API: re-exports only
    src/error.rs        the core's Error type
    src/store.rs        one module per area of the specification:
                          store.md: finding, creating and listing a store and its notes
    src/note.rs           notes.md: the item model (entries, fields, IDs, status), reading notes into items
    src/note/syntax.rs    notes.md, "Item syntaxes": the `Syntax` trait, and choosing a note's syntax by its extension (see "The item syntax" below)
    src/note/syntax/blocks.rs  notes.md, "The srl block syntax": `Syntax` for srl blocks, with their context
    src/note/syntax/jsonl.rs   notes.md, "The JSONL syntax": `Syntax` for JSONL notes, with their context
    src/note/object.rs    notes.md, "The item object": reading and writing items as JSON objects
    src/context.rs        notes.md, "Context": which note is an item's context; what it shows is its syntax's
    src/write.rs          notes.md, "How srl writes to notes" (see "Note editing" below)
    src/check.rs          cli.md, "Checking": `srl check`, and `srl lint` but for the syntax's own findings
    src/stats.rs          cli.md, "Analysis": `srl stats`
    src/config.rs         store.md, "Configuration"; the study day
    src/card.rs           cards.md: the card format and card IDs
    src/kind.rs           cards.md, "Kinds": the `qa` kind, asking kinds about items and making cards, describing kinds
    src/kind/program.rs   cards.md, "Kind programs": running the calls in batches, keeping their answers, checking what they print
    src/kind/cloze.rs     cards.md, "Cloze deletions"
    src/log.rs            log.md: reading and appending events
    src/review.rs         log.md and scheduling.md: recording reviews, resets, moves, suspensions
    src/schedule.rs       scheduling.md: replaying the log through FSRS, states
    src/next.rs           scheduling.md, "Choosing the next card" and "Siblings"
    src/selector.rs       selectors.md
    src/index.rs          store.md, "The index cache": `.srl/cache/index.sqlite`
    src/random.rs         reproducible random numbers for new IDs and `srl next`
  srl/                  the `srl` binary
    src/main.rs         parse, run, map the result to an exit code
    src/args.rs         clap definitions, no logic
    src/argv.rs         lets selector terms start with `-`
    src/commands.rs     one function per command: gather inputs, call the core, print
    src/output.rs       the only module that prints
    tests/cli.rs        tests that run the real binary, one rule at a time
    tests/scenarios.rs  tests that run the real binary through a person's usual days
  srl-test-kind/        a kind program for the tests (see "Testing"); not published
```

### The core is a library, the CLI is a thin layer on top

`srl-core` holds every rule from the specification: notes, items, cards,
scheduling, the log, selectors and the store. `srl` turns
command-line arguments into calls to `srl-core` and prints the results.

- **`srl-core` does not depend on clap**, and has no other dependency that
  only makes sense for a command line. The workspace split makes this a
  compile-time guarantee rather than a convention.
- **`srl-core` takes its inputs explicitly.** It does not read environment
  variables, the current directory, the clock or standard input on its own.
  The CLI gathers them and passes them in (see `Lookup` in `store.rs`, the
  study day, and the current time `now` that `review.rs` stamps events
  with). That keeps every rule testable without a real process environment.
  The one exception is the wait for the log's lock in `log.rs`, measured
  with the monotonic clock (`Instant`); it decides nothing the
  specification describes in terms of time.
- **`srl-core` never prints.** It returns values and errors. Turning them
  into text or JSON is the CLI's job.
- **`srl` contains no domain logic.** If a command needs a decision about
  items, cards or schedules, that decision belongs in `srl-core`, with tests
  there.

Each specification document maps to one module of `srl-core` (`store.md` to
`store.rs`, `selectors.md` to `selector.rs`, and so on). A module that grows
too big gets submodules in a folder of the same name (`selector.rs` plus
`selector/parse.rs`), never a `mod.rs`.

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

- **Closed sets are enums**: statuses, scheduling states,
  ratings, selector terms. A `match` on them lists every variant. Don't use a
  `_ =>` arm on our own enums (the `wildcard_enum_match_arm` lint flags it),
  so that adding a variant shows every place that must handle it.
- **Parse, don't validate.** Input from outside (notes, log lines,
  JSON on standard input) is parsed once, at the boundary, into
  types that can only hold valid values. After that, code doesn't check again.
  For example, a note is read once by the item syntax (see "The item
  syntax") and turned into `Item`s whose entries and fields are known to
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
  every error in every note), return the failure next to the results so the CLI can
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
  are returned.** Bad input, missing files, errors in notes, invalid log lines:
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
- **Use the current edition** (2024) and the toolchain pinned in
  `rust-toolchain.toml`, which rustup picks up by itself. Moving to a newer
  stable release means changing its `channel` and the workspace's
  `rust-version` in `Cargo.toml` together.

### Platform independence

`srl` must work the same on Linux, macOS and Windows. Write code that doesn't
depend on the platform, and flag every place that does.

- **Paths:** build them with `Path::join`, never by gluing strings with `/`
  or `\`. Note paths are `String`s with `/` between folders, because they are
  part of the formats (JSON output, `path:` selectors, `see`); turn them
  into file paths only with `Store::note_file`.
- **Platform locations** come from a crate, never from hard-coded paths such
  as `~/.local/share`: the default store uses `dirs::data_dir()`.
- **Finding programs** uses `which`, which follows `PATHEXT` on Windows.
  Running a bare name with `std::process::Command` only finds `.exe` there.
- **Text files** `srl` creates are written as UTF-8 with `\n`, and read
  with either `\n` or `\r\n`. A note `srl` changes keeps its own line
  endings (see "Note editing").
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
| `store.rs`, `note_paths`, `nfc`; [store.md](store.md) | macOS often lists file names in Unicode NFD, so paths could sort and match differently | Handled: paths are sorted and matched against `path:`, `in:` and `exclude` in NFC, on every platform |
| `write.rs`; [notes.md](notes.md) | Replacing a note atomically: a rename over an existing file, which Windows refuses while another program holds the file open without sharing | Handled: the rename's error is reported and the note left as it was. **TODO in code and spec**: not run on Windows |
| `log.rs`, `lock`; [log.md](log.md) | Concurrent appends are serialized by an OS lock on `.srl/log/.lock` (`File::lock`: `flock` on Unix, `LockFileEx` on Windows). On network file systems locks depend on the server, and may fail or not be shared between machines | Handled on local disks, the same on every platform. **TODO in code and spec** for network file systems: a lock that fails makes the append fail |
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
  usually does over several days (writing notes, studying as a frontend
  does, coming back the next day), one ordinary case per test.
- **Kind programs are tested with `srl-test-kind`**, a small Rust program
  whose behaviour is set by the fields of the items it gets (their errors,
  cards and groups, failures of a card or of a whole call, slowness,
  invalid output, and a log of its calls; see its `main.rs`), and, for
  `describe`, which has no item, by an argument before the call. It needs
  no interpreter, so the tests run the same on every platform. Tests find
  it next to their own binary, where `cargo test --workspace` builds it;
  with `cargo test -p srl` alone it isn't built, and those tests say so.
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
| `which` | `srl-core` | Finding kind programs on `PATH`, with `PATHEXT` on Windows |
| `jiff` | both | The study day: the local date and time zone (CLI), dates (core) |
| `toml` | `srl-core` | Parsing `.srl/config.toml` |
| `fsrs` | `srl-core` | FSRS scheduling, from the algorithm's authors. Its optimizer brings `rayon` and `ndarray` along, unused |
| `sha2` | `srl-core` | The `hash` of review events |
| `unicode-normalization` | `srl-core` | NFC for comparing note paths |
| `thiserror` | `srl-core` | Deriving the core's `Error` |
| `globset` | `srl-core` | `path:` globs in selectors, `exclude` globs |
| `serde` (derive), `serde_json` (`preserve_order`, `float_roundtrip`) | both | The JSON formats; fields keep their order, and numbers read back from the index cache are exactly the ones written |
| `rusqlite` (`bundled`, `cache`) | `srl-core` | The index cache. `bundled` compiles SQLite in (a C compiler is needed to build), so no system library is required; `cache` reuses prepared statements |
| `blake3` | `srl-core` | Hashing every file on every use for the index cache. Several times faster than SHA-256 on CPUs without SHA instructions |
| `tempfile` | `srl-core`; tests of both crates | The temporary file a changed note is written to before it is renamed over the note; temporary store trees in tests |

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

## The item syntax

How items are written in a note (see [notes.md](notes.md), "Item
syntaxes") is known to one module only, `note/syntax.rs`, and its
submodules, one per syntax: `blocks.rs` for srl blocks, `jsonl.rs` for
JSONL notes. It is the boundary between notes as text and items as data.

**A syntax is an implementation of the trait `Syntax`**, and
`syntax::of(path)` picks the one for a note by its extension alone
(`&Jsonl` for `.jsonl`, `&Blocks` otherwise). That choice is the only
thing the rest of `srl` decides: it calls the trait's methods on what
`of` gives, and knows nothing of fences, `@` lines or JSON lines, nor
which syntax it holds. No code outside `note/syntax/` matches on a syntax.

| Method | Gives |
|---|---|
| `parse(text)` | The note's items, as `ParsedItem`s (entries, fields, the line that names the item in messages, where each field is written, the item written on its own), in the order they are written, and its errors. |
| `own_context(text, mode)` | What the note shows as the context of its own items, those without `see`. srl blocks: the note without its items in mode `prose`. JSONL: nothing, ever. |
| `context(text, mode)` | What the note shows as the context of the items that name it with `see`, in any syntax. srl blocks: the same as `own_context`. JSONL: the note as it is, without a byte order mark. |
| `with_id(text, item, id)` | The `Edit` that gives item number `item` (its position among the items `parse` gives) an ID. |
| `without_draft(text, item)` | The `Edit` that removes `draft` from it. |
| `replace(text, item, entries, fields)` | The `Edit` that replaces it with an item of these entries and fields, as `srl edit --stdin` takes them. |
| `format(entries, fields)` | A new item, as `srl add` appends it. |
| `gap()` | What `srl add` puts before each item it appends: a blank line, or a line ending. |
| `lint(text)` | The likely mistakes of the syntax. None by default. |
| `field_lines(text, item, field)` | The lines of a field as `srl lint` looks at them, each with the line its messages name. By default, the field's text, all named by the item's line; srl blocks give the lines of the note it is written on. |

The item object (see [notes.md](notes.md), "The item object") is read and
written by `note/object.rs`, for the JSONL syntax and for the standard
input of `srl add` and `srl edit`. `Item::json` writes it for `srl show
--json`.

- **Context rules belong to the note shown, not to the item.**
  `context.rs` only picks the note: the one `see` names, through its
  syntax's `context`, or the item's own, through its syntax's
  `own_context`. So an item of one syntax can name a note of another,
  and each note is shown by its own rules. Mode `none` shows nothing
  whatever the syntax, and reads nothing.
- **An `Edit` is lines replaced by other lines**, numbered from 1 as
  `note::lines` splits them. The syntax decides which lines and what they
  become; `write.rs` applies the edit, keeping line endings and a byte
  order mark, and makes the change safe (see "Note editing").
- **What doesn't depend on the syntax stays outside it**: the entries and
  fields themselves, the rule for field names, splitting a note into lines
  (`note.rs`); picking which note is the context (`context.rs`); the lint
  of the `qa` kind, which looks at the lines of `Q` that `field_lines`
  gives (`check.rs`).
- **New items are checked without a syntax.** `srl add` and `srl edit`
  check their items' entries and fields with the kinds directly
  (`note::check_new`), so an item is judged the same whatever note it
  goes to.

So another syntax, such as the plugins of [ideas.md](ideas.md), "Writing
notes", would be another implementation of `Syntax`, and another case in
`syntax::of`.

### Lines are for people

**Rely on an item's line as little as possible.** A line is a convenience
for people: `PATH:LINE` in listings, messages and `srl show`, and `line`
in JSON. Nothing in `srl` finds an item by it:

- **Changes find the item again in the note as it is right before the
  change**: by its ID, or, for `srl id`, by having none. They name it to
  the syntax by its position among the note's items (see "Note
  editing").
- **The order of items** is the order `parse` gives them in, not an order
  of lines.

The line stays because most syntaxes can say where an item starts, and
people need it to open the note there. But it is a guess for some: in a
pretty-printed JSON document, say, an item could be said to start on the
line of its `{` or of its first key, and neither says where its fields
are. A syntax gives the line that helps a person most, and new code must
not make anything depend on it beyond showing it. Whether to keep the
line at all, and leave finding an item to its ID alone, each syntax its
own way, is open (see [ideas.md](ideas.md), "Writing notes").

### What still assumes lines

Not finding items, but two other things still take for granted that a
syntax writes its items in whole lines. Both hold for the two syntaxes
there are; they are worth keeping in mind before adding another, not
problems today.

- **Changes are line ranges.** Every change a syntax gives is an `Edit`:
  lines `first` to `last` replaced by other lines. `write.rs` applies it,
  keeping line endings, a byte order mark and every other line as they
  are, which is how "only the lines of the items changed are written"
  holds (see [notes.md](notes.md), "How srl writes to notes"). That
  needs each item to have lines of its own. In a syntax where items can
  share a line, such as a JSON array written on one line, or a
  pretty-printed one where one item's `},` and the next's `{` meet on a
  line, a change to one item would have to replace lines that hold others
  too: up to the whole file. Verification would still catch a change that
  reads back wrong, since it compares the items, not the lines, but the
  promise that nothing else is touched would be the syntax's to keep,
  not `write.rs`'s. Such a syntax would want byte ranges instead of line
  ranges, or would rewrite its file whole, as the JSONL syntax already
  does with an item's line.
- **Messages name lines.** Errors, `srl lint` and `PATH:LINE` everywhere
  name a line, so a syntax must give one for every item and every
  problem, even where it can only guess.

All of this assumes syntaxes are free to write items any way they like.
They need not be: a rule every syntax must follow, such as "each item
takes whole lines of its own, and starts on a line it can name", would
keep both assumptions true for any syntax, at the cost of the formats
that can't follow it. Whether to set such a rule, or to let a syntax
change bytes and give no line, is a choice for when a syntax that needs
it comes (see [ideas.md](ideas.md), "Writing notes").

## Note editing

Notes are written by hand, so `srl` must never rewrite one: no
re-serializing, no normalized whitespace or line endings. The one thing it
writes again is the line of a JSONL item it changes, which is data, not
text written for people. The commands that
change notes (see [notes.md](notes.md), "How srl writes to notes") go
through one module, `write.rs`, which asks the item syntax for each change
as an `Edit` (see "The item syntax"), with these rules:

1. **It edits lines, not data.** It changes header lines (adding `id:ID`,
   removing `draft`), replaces the lines of an item or of its block
   (`srl edit --stdin`), writes the line of a JSONL item again, or appends
   lines at the end. Every other byte of the file
   stays the same, including a UTF-8 BOM, `\r\n` line endings and a missing
   final newline. New lines get the line ending of the note's first line,
   or `\n` in a note without one.
2. **It finds each item in the note as it is now.** It reads the note
   again, finds the item by its ID (or, for `srl id`, every item without
   one), and names it to the syntax by its position. An item that is gone,
   or a note that has an error now, means the note changed since it was
   read: the command stops, and says so.
3. **It verifies its own work.** After editing, it parses the new text and
   compares the items with the old parse: they must be equal except for
   the change it meant to make. If not, the edit is abandoned and the file
   is left untouched.
4. **It checks the file didn't change underneath it**, by comparing the
   bytes it read with the bytes on disk right before writing.
5. **It writes atomically**: to a temporary file `.srl-*.tmp` in the
   note's folder (`tempfile`), then a rename over the note. The name is part
   of the specification, because users ignore it in git (see
   [store.md](store.md), "Version control and syncing").
6. **It has thorough tests**, including CRLF line endings, a UTF-8 BOM, no
   trailing newline, fences with extra spaces, fences of tildes and of more
   than three backticks, `@` lines, quoted values, non-ASCII text around
   the edited line, and a note changed between reading and writing. The
   edits themselves (which lines, what they become) are tested in the item
   syntax, and applying them to real files in `write.rs`.

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
  `kind::Cache`: `Notes::load` uses the kept answers, `Notes::load_asking`
  (`srl check`) asks again, and `note::check_new` (checking items about to be
  written) keeps nothing.
