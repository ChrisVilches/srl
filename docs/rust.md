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
scripts/check-windows.sh  lints the Windows build from Linux or macOS (see "Platform independence")
crates/
  srl-core/             the core, as a library
    src/lib.rs          the public API: re-exports only
    src/error.rs        the core's Error type
    src/store.rs        one module per area of the specification:
                          store.md: finding, creating and listing a store
    src/atom.rs           atoms.md: the atom file, IDs, status, reading atoms
    src/add.rs            atoms.md, "Creating atoms"
    src/edit.rs           atoms.md, "Status" and "Identity and editing"
    src/check.rs          atoms.md, "Validation"
    src/config.rs         store.md, "Configuration"; the study day
    src/card.rs           cards.md
    src/drill.rs          drills.md: finding and running drills
    src/drill/builtin.rs  drills.md, "Built-in drills"
    src/grade.rs          grading.md
    src/log.rs            log.md: reading and appending events
    src/review.rs         log.md and scheduling.md: recording reviews and resets
    src/schedule.rs       scheduling.md: replaying the log through FSRS, states
    src/next.rs           scheduling.md, "Choosing the next card"
    src/selector.rs       selectors.md
    src/session.rs        cli.md, "Sessions"
    src/index.rs          store.md, "The index cache": `.cache/index.sqlite`
    src/yaml.rs         front-matter between YAML text and JSON values
    src/frontmatter.rs  setting one key in an existing atom file (see below)
    src/process.rs      running a program (drill, grader) with an optional time limit
    src/random.rs       reproducible random numbers (IDs, drill seeds)
  srl/                  the `srl` binary
    src/main.rs         parse, run, map the result to an exit code
    src/args.rs         clap definitions, no logic
    src/argv.rs         lets selector terms start with `-`
    src/commands.rs     one function per command: gather inputs, call the core, print
    src/editor.rs       opening the editor (`srl edit ID`, `e` in `srl study`)
    src/external.rs     `srl NAME` runs `srl-NAME`
    src/output.rs       the only module that prints
    src/study.rs        `srl study`, the terminal frontend: writes to the writer it is given
    tests/cli.rs        tests that run the real binary
```

### The core is a library, the CLI is a thin layer on top

`srl-core` holds every rule from the specification: atoms, drills, cards,
grading, scheduling, the log, selectors and the store. `srl` turns
command-line arguments into calls to `srl-core` and prints the results.

- **`srl-core` does not depend on clap**, and has no other dependency that
  only makes sense for a command line. The workspace split makes this a
  compile-time guarantee rather than a convention.
- **`srl-core` takes its inputs explicitly.** It does not read environment
  variables, the current directory, the clock or standard input on its own.
  The CLI gathers them and passes them in (see `Lookup` in `store.rs`,
  `Search` and the study day in `drill.rs`, and the current time `now` that
  `review.rs` stamps events with). That keeps every rule testable without a
  real process environment. Two exceptions come with running programs
  (drills and the external grader): `which` reads `PATHEXT` itself on
  Windows, and a drill's time limit is measured with the monotonic clock
  (`Instant`). The wait for the log's lock in `log.rs` uses `Instant` too.
  None of them decides anything the specification describes in terms of
  time or environment.
- **`srl-core` never prints.** It returns values and errors. Turning them
  into text or JSON is the CLI's job.
- **`srl` contains no domain logic.** If a command needs a decision about
  atoms, cards or schedules, that decision belongs in `srl-core`, with tests
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

- **Closed sets are enums**: grading modes, statuses, scheduling states,
  ratings, selector terms. A `match` on them lists every variant. Don't use a
  `_ =>` arm on our own enums (the `wildcard_enum_match_arm` lint flags it),
  so that adding a variant shows every place that must handle it.
- **Parse, don't validate.** Input from outside (drill output, front-matter,
  log lines, JSON on standard input) is parsed once, at the boundary, into
  types that can only hold valid values. After that, code doesn't check again.
  For example, a card from a drill is read as loose JSON and turned into a
  `Card` whose grading variant carries exactly the fields it needs.
- **serde derives are the formats.** A contract's struct, with its
  `#[derive(Serialize, Deserialize)]` and attributes, is the readable
  definition of that format in code.

### Errors

- **Fail loudly** (see [overview.md](overview.md), principle 6). Never
  discard an error. Don't write `let _ = …` on a `Result`, don't turn an
  error into a default with `.ok()`, `.unwrap_or_default()` or
  `.unwrap_or(…)`, don't drop the cause with `.map_err(|_| …)`, and don't
  leave an `Err(_) => {}` arm empty. Propagate errors with `?`. Where the
  specification says to carry on after a failure (a drill that fails, an
  invalid card), return the failure next to the results so the CLI can
  report it on standard error. Never skip it without a word.
- Turning a missing value into a default is fine when the specification
  says so, as with an absent `srl.toml` key. Turning a *failure* into a
  default is not.
- `srl-core` has one `Error` enum (with `thiserror`) and a `Result<T>` alias.
  Each variant's message is one plain sentence that says what to do about it,
  because the CLI prints it as is (see [cli.md](cli.md)).
- `srl` uses `anyhow::Result`, adds context where a message would be unclear
  without it (`.context("cannot read the current directory")`), and turns
  errors into exit code 2 in one place (`main.rs`).
- **Errors that can be recovered from, or that the user should hear about,
  are returned.** Bad input, missing files, invalid atoms, failing drills:
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
- **Use the current edition** (2024) and the current stable toolchain.

### Platform independence

`srl` must work the same on Linux, macOS and Windows. Write code that doesn't
depend on the platform, and flag every place that does.

- **Paths:** build them with `Path::join`, never by gluing strings with `/`
  or `\`. Atom paths are `String`s with `/` between folders, because they are
  part of the formats (JSON output, `path:` selectors); turn them into file
  paths only with `Store::atom_file`.
- **Platform locations** come from a crate, never from hard-coded paths such
  as `~/.local/share`: the default store uses `dirs::data_dir()`.
- **Finding programs** uses `which`, which follows `PATHEXT` on Windows.
  Running a bare name with `std::process::Command` only finds `.exe` there.
- **File names `srl` creates** must be valid everywhere: no characters any
  platform forbids, no Windows device names, lowercase (see
  [atoms.md](atoms.md), "Creating atoms").
- **Text files** are written as UTF-8 with `\n`, and read with either `\n` or
  `\r\n`.
- **Tests** must not assume a platform either: build paths with `join`, and
  don't rely on Unix-only tools.

When something can't be platform-independent yet, mark it in both places:

1. a comment starting with `TODO(platform):` right above the code, saying
   what differs and what to decide;
2. a note starting with **TODO (platform):** in the specification document
   that describes the behaviour, or in the table below if it only concerns
   the code.

`grep -rn 'TODO(platform)\|TODO (platform)' crates docs` lists them all.

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
| `store.rs`, `atom_paths`, `nfc`; [store.md](store.md) | macOS often lists file names in Unicode NFD, so paths could sort and match differently | Handled: paths are sorted and matched against `path:` globs in NFC, and new names are NFC, on every platform |
| `add.rs`, `path_room`; `check.rs`, `long_paths`; [atoms.md](atoms.md) | Windows' 260-character path limit | Handled: new paths are cut to 259 UTF-16 units on every platform, and `srl check` reports existing files over it. **TODO in code and spec**: not run on Windows |
| `editor.rs`, `DEFAULT_EDITOR`; [cli.md](cli.md) | `EDITOR`/`VISUAL` are rarely set on Windows, and editor paths there often contain spaces | Handled: the default is `notepad` on Windows and `vi` elsewhere, and a quoted program may contain spaces. **TODO in code and spec**: not run on Windows; Windows 11's Notepad may return at once (then `srl` reports the file unchanged) |
| `study.rs`, `colors_work`; [frontends.md](frontends.md) | ANSI colors show as text in the old Windows console | Handled: virtual terminal mode is turned on (with `anstyle-query`), else highlights are printed raw and the reason reported. **TODO in code and spec**: not run on Windows |
| `drill.rs`, `programs`; [drills.md](drills.md) | "Executable" and `#!` scripts are Unix concepts | Handled: `which` decides, with the execute bit on Unix and `PATHEXT` on Windows. The Windows test drills (`.cmd`) compile (`scripts/check-windows.sh`) but have not been run yet |
| `log.rs`, `lock`; [log.md](log.md) | Concurrent appends are serialized by an OS lock on `log/.lock` (`File::lock`: `flock` on Unix, `LockFileEx` on Windows). On network file systems locks depend on the server, and may fail or not be shared between machines | Handled on local disks, the same on every platform. **TODO in code and spec** for network file systems: a lock that fails makes the append fail |
| `index.rs`, `connect`; [store.md](store.md) | SQLite's file locking is unreliable on network file systems (NFS, SMB) on every platform: concurrent updates could damage the index cache | **TODO in code and spec**: damage is usually detected and the cache rebuilt; keep stores on a local disk for now |
| `external.rs` | A process killed by a signal has no exit code (Unix only) | Handled: reported as an error |

## Testing

Write tests for every module that has logic, which is nearly all of
`srl-core` and the parts of `srl` that do more than pass values along.

- **Unit tests** go in the same file, in a `#[cfg(test)] mod tests` at the
  bottom. They can test private functions directly.
- **Integration tests** in `crates/srl/tests/` run the real binary
  (`env!("CARGO_BIN_EXE_srl")`) and check what a user or script sees: exit
  codes, standard output and standard error.
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
`srl find tag:japanese -tag:jlpt/n5` excludes N5, there should be a test
showing that it does.

## Dependencies

Use a crate when writing the code yourself would be cumbersome or easy to
get wrong. Write it yourself when the functionality is small. Every
dependency is code we didn't review, compile time and something that can be
abandoned or deprecated, so it has to be worth it.

**Use a crate** for standard, well-specified problems where a maintained
crate is the usual choice: argument parsing, JSON, YAML and TOML parsing,
SQLite, Unicode normalization, FSRS, temporary directories in tests.

**Write it yourself** when the code fits in a screen and has no tricky edge
cases. Examples: joining selector terms, finding the store directory,
building a file name from a title, mapping outcomes to exit codes. Running the
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
| `getrandom` | `srl` | The random seed for new atom IDs (the core derives IDs from it) |
| `dirs` | `srl` | The platform's data directory, for the default store |
| `which` | both | Finding `srl-NAME` and drill programs, with `PATHEXT` on Windows |
| `anstyle-query` | `srl`, on Windows only | Turning on ANSI colors in Windows consoles, without `unsafe` code of our own. Already part of the build through `clap` |
| `jiff` | both | The study day: the local date and time zone (CLI), dates (core) |
| `toml` | `srl-core` | Parsing `srl.toml` |
| `fsrs` | `srl-core` | FSRS scheduling, from the algorithm's authors. Its optimizer brings `rayon` and `ndarray` along, unused |
| `sha2` | `srl-core` | The `hash` of review events |
| `unicode-normalization` | `srl-core` | NFKC in answer normalization |
| `thiserror` | `srl-core` | Deriving the core's `Error` |
| `saphyr` | `srl-core` | YAML 1.2 parsing and writing of front-matter |
| `globset` | `srl-core` | `path:` globs in selectors |
| `serde` (derive), `serde_json` (`preserve_order`, `float_roundtrip`) | both | The JSON formats; facets keep their file order, and numbers read back from the index cache are exactly the ones written |
| `rusqlite` (`bundled`, `cache`) | `srl-core` | The index cache. `bundled` compiles SQLite in (a C compiler is needed to build), so no system library is required; `cache` reuses prepared statements |
| `blake3` | `srl-core` | Hashing every file on every use for the index cache. Several times faster than SHA-256 on CPUs without SHA instructions |
| `tempfile` | `srl`; tests of both crates | The temporary copy the editor works on; temporary store trees in tests |

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
| `print_stdout`, `print_stderr` | Only `output.rs` prints, with an `#[expect]`. `study.rs`, the interactive frontend, writes with `write!` to the writer it is given (the terminal), so that tests can drive it; its problems still go through `output`. |
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

## Front-matter editing: the YAML caveat

Rust has good YAML parsers, but no mature library that changes a YAML
document while preserving its comments and layout. A serde round trip
(parse, change, serialize) would rewrite a person's file: comments dropped,
flow lists turned into block lists, quoting and key order changed. Atom
files are edited by hand, so `srl` must never do that.

Only a few commands change front-matter that already exists: `srl check
--fix` (adds `id`), `srl accept`, `srl suspend` and `srl unsuspend` (set
`status`). All of them set one top-level scalar key. Commands that write new files (`srl add --title`,
`srl add --stdin`, `srl new`) can serialize normally. `srl edit` takes the
whole file from the user and only parses it.

So:

1. **One module edits existing front-matter**: `frontmatter.rs` in
   `srl-core`, with one entry point that sets a top-level key to a scalar
   value. Nothing else writes into an existing atom file.
2. **It edits text, not data.** It finds the line of the key (a line that
   starts, unindented, with `key:`, `"key":` or `'key':`) and replaces the
   value on that line, keeping anything after the value such as a
   `# comment`. A missing key gets a new line right after the opening `---`,
   with the line ending of that line. Every other byte of the file stays the
   same.
3. **It refuses what it doesn't handle** and returns an error rather than
   guessing: front-matter that isn't a block mapping (`{id: x}`), a key whose
   value spans several lines, or a key that appears twice.
4. **It verifies its own work.** After editing, it parses the new text and
   compares it with the old parse: they must be equal except for the one key
   that was set. If not, the edit is abandoned and the file is left
   untouched.
5. **It has thorough tests**, including CRLF line endings, a UTF-8 BOM, no
   trailing newline, empty front-matter, a file with no front-matter, quoted
   keys (`"status": draft`), trailing comments and non-ASCII text around the
   edited line.
6. **The parser must follow YAML 1.2**, so that `no`, `yes`, `on` and `off`
   stay strings. Under YAML 1.1 rules an atom with `title: no` would silently
   become `false`. Test this explicitly.

The YAML crate is `saphyr`: it follows YAML 1.2 and rejects duplicate keys.
`yaml.rs` converts between YAML and the JSON values the rest of the core
uses, and refuses YAML features atoms don't need (tags such as `!x`,
aliases, non-text keys). New files are written through the same module and
parsed again before they are saved, so a writer bug fails instead of storing
a different atom. Step 2 scans lines rather than using parser positions. That
is enough for one-line scalars, and step 4 catches any line it misreads.

Format-preserving YAML crates were checked when `frontmatter.rs` was
written (October 2026). `yaml-edit`, `yaml-splice` and `oneyaml` exist, but
all are young, at 0.x versions or recent 1.x, and `yaml-edit` has a fork that
exists to fix its source-preserving edits. None was mature enough. Check
again when this module needs to do more than set one scalar. A mature crate
can replace steps 2 and 3, but steps 4 to 6 still apply.

## The command line

- `args.rs` describes the commands with clap's derive API and holds no
  logic. Options shared by many commands (`--json`, `--dry-run`, the
  selector) are small `Args` structs flattened into each command.
- Selector terms may start with `-` (`-tag:a`), which clap would read as an
  option. `argv.rs` applies one rule before clap parses: on a command that
  takes a selector, every argument that is not one of that command's options
  (or an option's value) is a selector term. It moves the options first and
  the terms after `--`. The option list comes from the clap definition, so
  nothing is duplicated.
- `commands.rs` matches on the command, gathers inputs from the environment,
  calls `srl-core` and passes the result to `output.rs`. Commands that are
  not implemented yet return an error that says so.
- Unknown commands run `srl-NAME` from `PATH` (`external.rs`). Its exit codes
  0 and 1 pass through, and anything else becomes an error.

## The index cache

`index.rs` keeps `.cache/index.sqlite` (see [store.md](store.md), "The index
cache"). It must never change a result, only skip work:

- **One code path for both.** A cached atom goes through
  `Content::from_fields` like a parsed one, and new log lines go through the
  same `Replayer::apply` as a full replay. Don't add a second way to compute
  anything the cache stores.
- **Commands read through `Index`**, not `Atoms::load` or `Log::load`, where
  speed matters. `Atoms::load`, `Log::load` and `History::replay` stay as the
  reference that the cache is tested against, and as the fallback when the
  cache can't be used.
- **Bump `SCHEMA_VERSION`** whenever the tables, or what a stored value
  means, change. The cache is also rebuilt when the crate version changes,
  but during development that version stays the same.
- **Tests compare with and without the cache**: `assert_same` in
  `index.rs` after every kind of change to the files, and
  `the_index_cache_never_changes_what_commands_print` in `tests/cli.rs`.
  A new command or a new kind of change to the store gets a case there.
