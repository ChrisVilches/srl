# Specification audit

Audit of `srl`'s specification (`docs/`), its guides and examples, and the
structure of the Rust workspace, as of 2026-10-29 (commit `27b8140`). It
looks at vision, scope, consistency and brittleness, not at coding style.

This file is not part of the specification. It records findings to act on;
delete it, or move what survives into `docs/`, once they are dealt with.

## Rating scales

| Severity | Meaning |
|---|---|
| **High** | Can lose or corrupt review history, or breaks a promise the docs make to users. |
| **Medium** | A contradiction in the vision or the rules, or a design that will cost real work later. |
| **Low** | Friction, drift or untidiness; worth fixing when nearby. |

| Difficulty | Meaning |
|---|---|
| **Trivial** | A doc edit. |
| **Easy** | A small, local change to spec and code. |
| **Moderate** | Changes a contract or several modules; needs a design decision. |
| **Hard** | Rethinks a core idea. |

## Summary

`srl` has a clear and unusual idea: a strict, plain-text data layer for
spaced repetition, with one mapping to frontends and everything else built
on top. Most of the specification holds that idea well: the log as the only
truth for memory, the cache that never decides anything, IDs over
positions, careful note writes, and the split between specification, guides
and examples.

The weak points cluster in three places:

1. **Multi-device use.** The docs promise that a store "works well with git
   or any file syncing tool". The log format and kind programs make that
   untrue in common cases (H1, H3).
2. **Strictness at the wrong moment.** "One error stops everything" also
   stops `srl review`, so a half-typed note loses the rating of the card
   just studied (H2).
3. **Kind programs.** They have grown from "how fields become cards" into a
   plugin runtime with its own protocol, cache, migration policy and a
   designed daemon. They carry the main threat to history (H4), the main
   vision drift (M1, M4), and most of the remaining complexity.

| ID | Finding | Area | Severity | Difficulty |
|---|---|---|---|---|
| H1 | The log does not survive syncing as the docs claim | Brittleness | High | Moderate |
| H2 | A broken note anywhere makes `srl review` fail, losing the rating | Specification | High | Moderate |
| H3 | Kind programs make a store machine-specific, and a cloned store runs code on any command | Brittleness | High | Moderate |
| H4 | Card IDs of kind programs can change silently, orphaning history | Brittleness | High | Easy |
| M1 | LLM-generated cards contradict `srl`'s own case for a database | Vision | Medium | Easy |
| M2 | The authoring model drifted from "questions live in your notes" | Vision | Medium | Hard |
| M3 | A JSONL context gives away the answers of other items | Specification | Medium | Easy |
| M4 | Kind programs are scope creep beyond "two things, and nothing else" | Scope | Medium | Easy |
| M5 | Invalid log lines are skipped, against "fail loudly" | Specification | Medium | Easy |
| M6 | "Every rule in one sentence" no longer describes the specification | Vision | Medium | Moderate |
| L1 | The review `hash` is defined by a display format | Specification | Low | Easy |
| L2 | Exit codes are surprising for linters and write commands | Specification | Low | Easy |
| L3 | The default store is used silently from any directory | Specification | Low | Easy |
| L4 | The card ID separator is described as changeable | Specification | Low | Trivial |
| L5 | The frontend contract is larger in practice than "one mapping" | Vision | Low | Trivial |
| L6 | `srl-core` is a second, public interface | Structure | Low | Easy |
| L7 | `srl-core` mixes pure rules with process, SQLite and file I/O | Structure | Low | Moderate |
| L8 | Documentation drift: overview misses two examples | Docs | Low | Trivial |

---

## High

### H1. The log does not survive syncing as the docs claim

**Severity High · Difficulty Moderate**

`docs/store.md`, "Version control and syncing", says the log "is
append-only, so it almost never conflicts on merge". Three things make that
untrue:

- **Git conflicts are the normal case.** Two devices that both study in the
  same month both append to the end of `.srl/log/YYYY-MM.jsonl`. Git sees
  two edits at the same place and reports a conflict every time, not
  "almost never".
- **Sync-tool conflict copies are read as logs.** Dropbox, Syncthing and
  others resolve conflicts by keeping a second file, such as
  `2026-10 (conflicted copy).jsonl` or `2026-10.sync-conflict-….jsonl`. The
  log reader takes every `*.jsonl` in `.srl/log/`
  (`crates/srl-core/src/log.rs`, around line 206), so the events in both
  copies are replayed twice. Duplicated Agains double the lapses and can
  make a card a leech; duplicated reviews shorten intervals.
- **Replay order depends on how the conflict was resolved.** Events are
  replayed "in log order … Their times are not sorted"
  (`docs/scheduling.md`, "Replay"). After a merge, one device's reviews may
  come after the other's later ones. A review out of order counts as 0
  days elapsed, so the result depends on line order, not on what happened.

This is the main promise the plain-text log makes (history you can sync),
and it fails silently.

**Possible fixes**, from cheapest:

1. Read only files named exactly `YYYY-MM.jsonl`; report any other file in
   `.srl/log/` as an error in `srl check` and a warning elsewhere.
2. Have `srl init` write a `.gitattributes` with
   `.srl/log/*.jsonl merge=union`, and document it.
3. The real fix, which changes the log contract: one log file per device
   and month (for example `log/2026-10.DEVICE.jsonl`, with the device name
   kept outside the store), and replay sorted by `t`, then by file and
   line. Appends never touch another device's file, so neither git nor sync
   tools ever conflict. Exact duplicate events could also be dropped.

### H2. A broken note anywhere makes `srl review` fail, losing the rating

**Severity High · Difficulty Moderate**

`srl review` loads every note before it writes (`crates/srl/src/commands.rs`,
`fn review`, which calls `Notes::load`). By `docs/notes.md`, "Errors", any
error in any note then stops it. So:

1. A frontend shows a card and the person rates it.
2. Meanwhile the person has a note open in an editor, with an srl block
   half typed (an unclosed fence), and the editor autosaves.
3. `srl review` fails with exit 2. The rating is not recorded.

This is likely, not exotic: the frontend contract encourages "opening its
note … in an editor" (`docs/frontends.md`, "The loop"), and the whole point
is that questions are written in notes while studying them. The same holds
for `srl suspend`, and for a kind program that fails as a whole (for
example a missing Python package) while you review a `qa` card.

The rationale for strictness ("a question you think you are studying and
aren't", `docs/rationale.md`, "Why one error stops everything") is about
studying and listing. It does not apply to recording a rating for a card
that was valid a moment ago. The log only needs a valid card ID; the
`hash` is the only thing that needs the item.

**Possible fixes:** let the log-writing commands (`review`, `suspend`,
`unsuspend`, `reset`, `move`) read only the notes that hold the cards they
name, or accept any well-formed card ID that the store knew at its last
successful read (the index cache could keep that set). Keep the error
loud: print it as a warning, but record the rating. This needs a decision,
and fits the open question "Making strictness less annoying" in
`docs/overview.md`.

### H3. Kind programs make a store machine-specific, and a cloned store runs code

**Severity High · Difficulty Moderate**

`.srl/config.toml` is committed and shared (`docs/store.md`, "Version
control and syncing"), and `[kinds]` holds commands. But:

- **The command differs by platform.** `docs/cards.md` itself says Linux
  takes `["python3", "kinds/ja_words.py"]` and Windows
  `["py", "kinds/ja_words.py"]`. One shared file can't hold both, so a
  store with kind programs can't be synced between Linux and Windows,
  against the rule that `srl` "behaves the same on Linux, macOS and
  Windows".
- **A missing dependency stops everything.** A kind program that fails as a
  whole stops every command that reads items (`docs/cards.md`, "How srl
  calls them"). Clone a store with `kind:ja-words` on a laptop without
  MeCab and you can't study even your `qa` cards, or record a review (H2).
- **Any read command runs the store's programs.** `srl find`, `srl next`,
  `srl stats` all run `validate` and `groups` for items without kept
  answers, and a fresh clone has none. The docs say "read its `[kinds]`
  first", but unlike git hooks, nothing stops a copied store from running
  code the moment you list it. For a tool aimed at agents operating on
  stores, that is a real exposure.

**Possible fixes:** a per-machine overlay, such as an uncommitted
`.srl/config.local.toml` (or a user-level config) that can override
`[kinds]` commands; and a per-machine trust list kept outside the store
(`srl trust`, non-interactive), without which kind programs of a store are
not run and their items are reported as untrusted. Both change
configuration contracts.

### H4. Card IDs of kind programs can change silently, orphaning history

**Severity High · Difficulty Easy**

Card IDs are history. For kind programs they rest on two things nothing
enforces:

- The author must never change suffixes for the same fields (the "Rule for
  kind authors" in `docs/cards.md`). That is a promise from third-party
  code.
- `srl` keeps `groups` answers in the cache and does not notice a program
  changed in place (`docs/store.md`, "The index cache"). After an update,
  old suffixes stay in use until `srl check` or a cache rebuild (a new
  machine, a new `srl` version), then change at once.

When they change, the old cards become orphans, which `srl check` reports
as notices with exit code 0 (`docs/cli.md`, "Checking"). Nothing fails, and
the history quietly restarts. This is exactly the silent loss the "fail
loudly" principle is there to prevent.

**Possible fix, cheap and within today's design:** when `srl check` (or any
call that re-asks) gets a `groups` answer that differs from a kept answer
for the same input line, report it as an error naming the program and the
suffixes that changed. The cache already holds both. Also consider making
orphans created by such a change an error rather than a notice.

---

## Medium

### M1. LLM-generated cards contradict `srl`'s own case for a database

**Severity Medium · Difficulty Easy**

`docs/rationale.md` opens with why `srl` exists instead of "just prompting
an LLM": an LLM "can be wrong again each time; a checked question stays
correct", and "every session invents new questions". Drafts exist "so that
items written by agents … never enter your study queue without your
approval" (`docs/notes.md`, "Status").

`examples/kind-llm-generated`, presented in `docs/guides/ideas.md` and
explained at length in `docs/rationale.md` and `docs/cards.md`, does
exactly what the rationale argues against: one card ID whose question and
answer an LLM writes anew at every review, never seen or accepted before it
is studied, with what was asked not kept in the log. Its memory state then
measures "how well you did on whatever the LLM asked", not one piece of
knowledge.

It also shapes the core: the open question about strictness, and the
`srl next` failure mode, are argued with "an LLM API that is down"
(`docs/cli.md`, "Studying"). `srl` "does not contain, call or assume an
LLM", but its error design is being bent around one.

**Possible fixes:** decide explicitly. Either state in the rationale that
non-deterministic kinds are a supported, deliberate trade-off (and say what
the memory state means for them), or keep `make` deterministic in spirit
("may be slow", not "may differ") and move LLM-written questions back to
drafts written by agents, which the rest of the design already supports.

### M2. The authoring model drifted from "questions live in your notes"

**Severity Medium · Difficulty Hard**

The first goals in `docs/overview.md` are "Questions live in your notes …
in a few keystrokes" and "Notes stay notes". But `docs/rationale.md`, "Why
JSONL notes", now says "Most questions are not typed next to the knowledge
by hand: scripts and agents write them". Those are two different products:
a note-centric tool for people, and a data store for generators.

The second has added a whole item syntax (JSONL notes), `see`,
`context_format`, an item object shared by four places, and the open
"one context rule for both syntaxes" question, which is a symptom: the two
syntaxes really are two models of where a question lives.

**Possible fix:** decide which user is primary and say so in the Goals. If
machine-written items are primary, the goal should say "questions are
linked to your notes", and Markdown blocks become the convenience. If
people are primary, JSONL notes are an import format and their context
rules can stay simple. This is a vision decision, hence Hard, though the
doc edit is small once made.

### M3. A JSONL context gives away the answers of other items

**Severity Medium · Difficulty Easy**

`docs/notes.md`, "Context": a JSONL note named by `see` "is given as it
is … every line", in `prose` mode too. So a card whose `see` names a JSONL
note shows, after its answer, every question and answer in that note.
Those may be cards you study the same day, which defeats the sibling rule's
purpose (seeing one gives away another). It also breaks the symmetry with
Markdown notes, whose `prose` mode removes items for that reason.

**Possible fix:** in `prose` mode, a JSONL note gives no context (or its
items' fields stripped); `full` keeps it whole. This also answers the open
"one context rule" question: `prose` never shows items, in either syntax.

### M4. Kind programs are scope creep beyond "two things, and nothing else"

**Severity Medium · Difficulty Easy**

`docs/overview.md` says `srl` is "two things, and nothing else": a data
layer and one mapping to frontends; and a non-goal is that it "does not
launch other programs for you", with kind programs the one exception. In
practice the exception has become a third thing, a plugin host:

- a four-call protocol with batching, ordering and error-attribution rules;
- a kept-answer cache keyed by command, with its own invalidation story;
- a versioning and migration policy imposed on third-party authors;
- timeouts, per-call failure modes, and a `TODO (design)` for other run
  modes;
- in `docs/ideas.md`, about 100 lines designing an HTTP daemon per store
  (ports, lock files, tokens, DNS rebinding, process groups, `setsid`). A
  daemon contradicts the non-goals and the "CLI is the API" principle.

Kind programs are about a third of the specification's complexity for, so
far, two example programs. That is a lot of contract surface to freeze.

**Possible fixes:** move the daemon and session designs out of `ideas.md`
into one paragraph ("a long-running mode may be wanted; not designed"), and
state in the overview that `srl` is also a host for kind programs, with the
limits it keeps. Consider whether `describe`/`validate`/`groups` could be
one call returning everything, since `srl` already decides the order.

### M5. Invalid log lines are skipped, against "fail loudly"

**Severity Medium · Difficulty Easy**

Principle 6 says no error is swallowed and `srl` never substitutes a guess.
Notes follow it strictly. The log does not: an invalid line "is skipped and
reported on standard error … Every command carries on" (`docs/log.md`). For
a source of truth, a dropped review is a changed schedule. Combined with
H1, a corrupt line from a bad merge becomes a warning that frontends,
which only read standard output, never show.

There are good reasons (one bad line shouldn't stop studying, and a line
being written can be read half-way), but they aren't in
`docs/rationale.md`, so the two rules look inconsistent.

**Possible fixes:** add a rationale section for the difference; make
`srl check` exit 2 for invalid log lines (it already reports them); and
skip silently only a last line without its final newline, which is the
"being written" case.

### M6. "Every rule in one sentence" no longer describes the specification

**Severity Medium · Difficulty Moderate**

"Predictable behaviour. Every rule is simple enough to state in one
sentence" (`docs/overview.md`). The srl block syntax has 14 numbered rules;
kind programs have four calls, a batch protocol, cache keys and error
attribution; the cache's incremental replay is a paragraph of conditions;
`srl lint` has a page of heuristics. About 4,200 lines of documents
describe about 14,000 lines of Rust, much of it cross-references and
designs for hypothetical syntaxes (`docs/rust.md`, "What still assumes
lines"; `docs/ideas.md`, "Items without a line").

This matters because the docs are declared the source of truth and must be
updated with every change (`AGENTS.md`). Their size and redundancy make
drift likely (see L8), and the goal sets an expectation the product can't
meet.

**Possible fixes:** reword the goal to "each decision has one rule, stated
where it is used"; remove speculative sections whose only purpose is a
future syntax; and state each rule once, linking to it instead of
restating it (context rules, for example, are restated in `notes.md`,
`frontends.md`, `rationale.md` and `rust.md`).

---

## Low

### L1. The review `hash` is defined by a display format

**Severity Low · Difficulty Easy**

`hash` is the SHA-256 of "the item as written, as `srl show` prints it
without `--json`" (`docs/log.md`). So it changes when nothing about the
question did: moving an item between syntaxes, changing its fence length,
or `srl edit` rewriting a JSONL line with other key order or spacing. Its
one purpose, "did my retention change after I rewrote this item?", gets
false positives, and a future change to `srl show` output would change it
for all items.

**Fix:** hash a canonical form, such as the item object of its `kind` and
fields (not `id`, `see`, `draft`), serialized one fixed way. Old hashes
stay valid as what they were.

### L2. Exit codes are surprising for linters and write commands

**Severity Low · Difficulty Easy (but a breaking change)**

- `srl lint` exits 0 when it found something and 1 when not
  (`docs/cli.md`, "Checking"): the opposite of most linters, so it can't
  gate a commit hook or CI as written.
- `srl check` exits 0 when it found only orphans, which can be lost
  history (see H4).
- Write commands exit 1 when they changed nothing (`srl id`, `srl accept`,
  `srl reset`, `srl suspend`). The grep convention fits queries; for
  writes it breaks `set -e` scripts on the normal "already done" case.

**Fix:** decide once, document it in "Conventions", and change it before
more scripts depend on it.

### L3. The default store is used silently from any directory

**Severity Low · Difficulty Easy**

Without `--dir`, `SRL_DIR` or a `.srl` above the current directory, every
command uses the default store in the platform's data directory
(`docs/store.md`, "Finding the store"). Once that store exists, running
`srl add notes/x.md --stdin` in the wrong folder writes into it without a
word. A store is otherwise "your study folder", so a hidden one in
`~/.local/share` is an odd default. **Fix:** print the store used on
standard error when it is the default, or drop the default.

### L4. The card ID separator is described as changeable

**Severity Low · Difficulty Trivial**

`docs/cards.md`, "Card IDs", names the code constant `SUFFIX_SEPARATOR` and
discusses what changing it would do, "accepted while in development". The
separator is part of the log contract, and `srl` is already published
(version 0.1.1, `cargo install srl` in the README). **Fix:** declare it
fixed, and drop the paragraph about changing it.

### L5. The frontend contract is larger in practice than "one mapping"

**Severity Low · Difficulty Trivial**

The docs say frontends "understand this and nothing else"
(`docs/frontends.md`). The same page then lets them suspend cards and open
`path` in an editor, and `srl-web` also browses cards and adds items, using
`find`, `cards`, `show` and `add`. That is fine, but the contract a real
frontend depends on is several commands' JSON, not one. **Fix:** say that
`srl next --json` is the one mapping needed to study, and that every
`--json` output is a contract too (as "Small fixed contracts" implies).

### L6. `srl-core` is a second, public interface

**Severity Low · Difficulty Easy**

"The CLI is the API. There is no separate interface for scripts or agents."
`srl-core` is published on crates.io with about 130 public items. Its
README rightly says it is not stable, but anyone can depend on it, and each
release then breaks them. **Fix:** mark the API `#[doc(hidden)]` or say in
the crate description that it exists only for the `srl` binary; keep the
public surface to what `commands.rs` uses.

### L7. `srl-core` mixes pure rules with process, SQLite and file I/O

**Severity Low · Difficulty Moderate**

The workspace split is sound: `srl` is a thin CLI, `srl-core` holds the
rules and never prints or reads the environment. Module boundaries follow
the documents, and the `Syntax` trait cleanly isolates item syntaxes.

Two boundaries are weaker:

- `srl-core` holds both pure logic (parsing, cloze, selectors, FSRS
  replay, the sibling rule) and side effects (spawning kind programs in
  `kind/program.rs`, SQLite in `index.rs`, OS locks in `log.rs`, atomic
  writes in `write.rs`). `kind.rs` both defines `qa` and orchestrates the
  programs. A clearer internal layer (a pure model and scheduling layer,
  with I/O adapters around it) would make the pure parts testable without
  a store, and keep kind-program growth (M4) from spreading.
- The "one module per document" rule is already bent: `check.rs` holds
  lint and orphans, `review.rs` spans `log.md` and `scheduling.md`. That is
  fine, but the rule in `docs/rust.md` should say it maps areas, not
  documents.

None of this is urgent; the largest files (`blocks.rs`, `program.rs`,
`index.rs`, `write.rs`, each over 1,000 lines with tests) are where a split
would pay first.

### L8. Documentation drift: overview misses two examples

**Severity Low · Difficulty Trivial**

`docs/overview.md`, "Specification, guides and examples", lists four
examples. `examples/README.md` lists six: `kind-ja-words` and
`kind-llm-generated` are missing from the overview, though `cards.md` and
`rationale.md` refer to both. A small sign of M6: the same list is kept in
two places.

---

## What holds up well

Worth keeping as they are:

- Memory state computed only from the log, so the algorithm, retention
  target and weights can change for the whole history at once.
- The index cache as an optimization only, tested against a cache-free
  path.
- Identity by ID, never by position; `groups` seeing only the fields.
- Note writes that change only the lines of the items, verify by
  re-parsing, detect concurrent edits, and write atomically.
- No LLM in the core, and agents using the same commands as people.
- The rule for what goes in the specification, guides and examples.

## Suggested order

1. **H4** and **L8**, **L4**: cheap, and H4 protects history today.
2. **H1** fixes 1 and 2 (read only `YYYY-MM.jsonl`; union merge), then
   decide on per-device logs.
3. **H2**: let commands that write the log record despite unrelated note
   errors.
4. **M3**, **M5**, **L1**, **L2**: small rule changes, best made before
   more programs depend on current behaviour.
5. Decide **M1** and **M2** (vision), then trim **M4** and **M6** to match.
6. **H3**: per-machine config overlay and trust, once M4 settles how big
   kind programs are meant to be.
7. **L3**, **L5**, **L6**, **L7** when nearby.
