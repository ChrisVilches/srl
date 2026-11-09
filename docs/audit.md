# Audit

Known problems with `srl`'s specification, structure and scope that are
not fixed yet: rules that contradict each other or the goals, designs that
are brittle, and places where the vision and the documents drift apart.
Each finding is rated, and says how it could be fixed.

This audit is ongoing. It is kept up to date like every other document
(see [overview.md](overview.md), "Open questions"):

- **A finding is removed once it is fixed**, in the same piece of work as
  the fix. Its ID is never reused, so references to it in commits stay
  unambiguous.
- **A finding that turns out to be wrong is removed**, or reworded to what
  survives of it.
- **New findings are added** whenever a review of the documents, the code
  or the way `srl` is used finds one, with the next free ID of its
  severity.
- **A finding that becomes a decision to take** moves to a
  **TODO (design):** note where the specification touches it, and is
  removed here.

Findings look at what `srl` is and does: its contracts, rules and
structure, and whether they serve the goals in
[overview.md](overview.md). They don't cover coding style, which
[rust.md](rust.md) does.

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
positions, careful collection writes, and the split between specification,
guides and examples.

The open findings cluster in three places:

1. **Multi-device use.** The docs promise that a store "works well with git
   or any file syncing tool". The log format, kind programs and source
   programs make that untrue in common cases (H1, H3).
2. **Strictness at the wrong moment.** "One error stops everything" also
   stops `srl review`, so a half-typed collection loses the rating of the
   card just studied (H2).
3. **Kind programs.** They are `srl`'s main extension mechanism: any new
   type of card is a program, an idea as useful without any AI as with it.
   The overview still presents them as an exception rather than a pillar
   (M3), and they carry the main threat to history (H4) and much of the
   specification's complexity.

| ID | Finding | Area | Severity | Difficulty |
|---|---|---|---|---|
| H1 | The log does not survive syncing as the docs claim | Brittleness | High | Moderate |
| H2 | A broken item anywhere makes `srl review` fail, losing the rating | Specification | High | Moderate |
| H3 | Kind programs and source programs make a store machine-specific, and a cloned store runs code on any command | Brittleness | High | Moderate |
| H4 | Card IDs of kind programs can change silently, orphaning history | Brittleness | High | Easy |
| M3 | Kind programs are the main extension mechanism, but the vision doesn't say so | Vision | Medium | Easy |
| M4 | Invalid log lines are skipped, against "fail loudly" | Specification | Medium | Easy |
| M5 | "Every rule in one sentence" no longer describes the specification | Vision | Medium | Moderate |
| L2 | Exit codes are surprising for `srl check` and write commands | Specification | Low | Easy |
| L3 | The default store is used silently from any directory | Specification | Low | Easy |
| L4 | The card ID separator is described as changeable | Specification | Low | Trivial |
| L5 | The frontend contract is larger in practice than "one mapping" | Vision | Low | Trivial |
| L6 | `srl-core` is a second, public interface | Structure | Low | Easy |
| L7 | The domain layer still leans on modules with I/O | Structure | Low | Moderate |
| L9 | What a card with varying content means is unspecified | Specification | Low | Easy |
| L10 | What symbolic links in a store mean is unspecified, and a loop multiplies collections | Specification | Low | Easy |
| L11 | A collection named as a Windows device can't exist on Windows | Specification | Low | Easy |
| L12 | A `[sources]` without sources makes a store that silently has no items | Specification | Low | Easy |

---

## High

### H1. The log does not survive syncing as the docs claim

**Severity High · Difficulty Moderate**

[store.md](store.md), "Version control and syncing", says the log "is
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
  (`files` in `crates/srl-core/src/log.rs`), so the events in both
  copies are replayed twice. Duplicated Agains double the lapses and can
  make a card a leech; duplicated reviews shorten intervals.
- **Replay order depends on how the conflict was resolved.** Events are
  replayed "in log order … Their times are not sorted"
  ([scheduling.md](scheduling.md), "Replay"). After a merge, one device's reviews may
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

### H2. A broken item anywhere makes `srl review` fail, losing the rating

**Severity High · Difficulty Moderate**

`srl review` reads every source before it writes (`crates/srl/src/commands.rs`,
`fn review`, which calls `Catalog::load`). By [items.md](items.md), "Errors", any
error in any source then stops it. So:

1. A frontend shows a card and the person rates it.
2. Meanwhile the person has an XML collection open in an editor, with an
   item half typed (an unclosed element), and the editor autosaves; or
   has just renamed a note that items name with `see`.
3. `srl review` fails with exit 2. The rating is not recorded.

This is likely, not exotic: the frontend contract encourages opening the
note an item names while studying ([frontends.md](frontends.md), "The
loop"), and items are edited and added while studying them. The same holds
for `srl suspend`, for a kind program that fails as a whole (for
example a missing Python package) while you review a `qa` card, and for a
source program that fails, such as one whose web service is down: its
failure "stops every command that reads items until it is fixed"
([sources.md](sources.md), "What srl checks"), even a review of a card
from another source.

The rationale for strictness ("a question you think you are studying and
aren't", [rationale.md](rationale.md), "Why one error stops everything") is about
studying and listing. It does not apply to recording a rating for a card
that was valid a moment ago. The log only needs a valid card ID; the
`hash` is the only thing that needs the item.

**Possible fixes:** let the log-writing commands (`review`, `suspend`,
`unsuspend`, `reset`, `move`) read only the collections that hold the cards they
name, or accept any well-formed card ID that the store knew at its last
successful read (the index cache could keep that set). Keep the error
loud: print it as a warning, but record the rating. This needs a decision,
and fits the open question "Making strictness less annoying" in
[overview.md](overview.md).

### H3. Kind programs and source programs make a store machine-specific, and a cloned store runs code

**Severity High · Difficulty Moderate**

`.srl/config.toml` is committed and shared ([store.md](store.md), "Version
control and syncing"), and both `[kinds]` and `[sources]` hold commands:
a kind program's, and a source program's `program`
([sources.md](sources.md), "Source programs"). But:

- **The command differs by platform.** [cards.md](cards.md) itself says Linux
  takes `["python3", "kinds/ja_words.py"]` and Windows
  `["py", "kinds/ja_words.py"]`, and a source program's `program` follows
  the same rules. One shared file can't hold both, so a store with kind
  or source programs can't be synced between Linux and Windows, against
  the rule that `srl` "behaves the same on Linux, macOS and Windows"
  ([cli.md](cli.md)).
- **A missing dependency stops everything.** A kind program that fails as a
  whole stops every command that reads items ([cards.md](cards.md), "How srl
  calls them"), and so does a source program that fails
  ([sources.md](sources.md), "What srl checks"). Clone a store with
  `kind:ja-words` on a laptop without MeCab, or with a source program
  whose Python package or database isn't there, and you can't study even
  your `qa` cards of other notes, or record a review (H2).
- **Any read command runs the store's programs.** `srl find`, `srl next`,
  `srl stats` all run `validate` and `groups` for items without kept
  answers, and a fresh clone has none. Source programs are worse: `srl`
  keeps nothing they answer, so every command that reads items runs
  `collections` and `read` of every one ([sources.md](sources.md), "How
  many calls"). The docs say "A store you copy from someone else runs
  their programs: read its `[kinds]` first" ([cards.md](cards.md)) and
  "read the `[sources]` of a store you copy from someone else"
  ([sources.md](sources.md)), but unlike git hooks, nothing stops a
  copied store from running code the moment you list it. For stores
  shared between people, that is a real exposure.

**Possible fixes:** a per-machine overlay, such as an uncommitted
`.srl/config.local.toml` (or a user-level config) that can override the
commands of `[kinds]` and `[sources]`; and a per-machine trust list kept
outside the store (`srl trust`, non-interactive), without which the
programs of a store are not run and their items are reported as
untrusted. Both change configuration contracts.

### H4. Card IDs of kind programs can change silently, orphaning history

**Severity High · Difficulty Easy**

Card IDs are history. For kind programs they rest on two things nothing
enforces:

- The author must never change suffixes for the same fields (the "Rule for
  kind authors" in [cards.md](cards.md)). That is a promise from third-party
  code.
- `srl` keeps `groups` answers in the cache and does not notice a program
  changed in place ([store.md](store.md), "The index cache"). After an update,
  old suffixes stay in use until `srl check` or a cache rebuild (a new
  machine, a new `srl` version), then change at once.

When they change, the old cards become orphans, which `srl check` reports
as notices with exit code 0 ([cli.md](cli.md), "Checking"). Nothing fails, and
the history quietly restarts. This is exactly the silent loss the "fail
loudly" principle is there to prevent.

**Possible fix, cheap and within today's design:** when `srl check` (or any
call that re-asks) gets a `groups` answer that differs from a kept answer
for the same input line, report it as an error naming the program and the
suffixes that changed. The cache already holds both. Also consider making
orphans created by such a change an error rather than a notice.

---

## Medium

### M3. Kind programs are the main extension mechanism, but the vision doesn't say so

**Severity Medium · Difficulty Easy**

Kind programs are how `srl` grows new types of cards without changing the
core: conjugation drills, the words of a sentence, problems drawn at
random. That was as useful before LLMs as it is now; a kind that asks an
LLM for its questions is one plugin among many, and the core knows nothing
about it.

The documents undersell this. [overview.md](overview.md) says `srl` is "two
things, and nothing else", a data layer and one mapping to frontends, and
lists kind programs, beside source programs, as the exceptions to "does
not launch other programs for you". So the mechanism that carries much of the
specification reads as an exception, not as a part of the design:

- a four-call protocol with batching, ordering and error-attribution rules;
- a kept-answer cache keyed by command, with its own invalidation story;
- a versioning and migration policy for third-party authors;
- timeouts, per-call failure modes, and a `TODO (design)` for other run
  modes.

Because it is the extension point, its contract is the one that most needs
to be small and stable before others build on it. Meanwhile
[ideas.md](ideas.md) spends about 100 lines on an HTTP daemon per store (ports,
lock files, tokens, DNS rebinding, process groups, `setsid`), well ahead
of any program that needs it.

**Possible fixes:** name kinds in the overview's "The idea" as the way
`srl` is extended (a data layer, one mapping to frontends, and one
mapping from items to cards that programs can provide). Shorten the
daemon and session designs in `ideas.md` to a paragraph until a program
needs them. Consider whether `describe`/`validate`/`groups` could be fewer
calls before more programs depend on four.

### M4. Invalid log lines are skipped, against "fail loudly"

**Severity Medium · Difficulty Easy**

Principle 6 says no error is swallowed and `srl` never substitutes a guess.
Notes follow it strictly. The log does not: an invalid line "is skipped and
reported on standard error … Every command carries on" ([log.md](log.md)). For
a source of truth, a dropped review is a changed schedule. Combined with
H1, a corrupt line from a bad merge becomes a warning that frontends,
which only read standard output, never show.

There are good reasons (one bad line shouldn't stop studying, and a line
being written can be read half-way), but they aren't in
[rationale.md](rationale.md), so the two rules look inconsistent.

**Possible fixes:** add a rationale section for the difference; make
`srl check` exit 2 for invalid log lines (it already reports them); and
skip silently only a last line without its final newline, which is the
"being written" case.

### M5. "Every rule in one sentence" no longer describes the specification

**Severity Medium · Difficulty Moderate**

"Predictable behaviour. Every rule is simple enough to state in one
sentence" ([overview.md](overview.md)). Kind programs have four calls, a
batch protocol, cache keys and error attribution; the cache's incremental
replay is a paragraph of conditions; the XML format has six numbered rules
and a layout rule for fields of many lines. The documents of the
specification are several thousand lines, much of them cross-references.

This matters because the docs are declared the source of truth and must be
updated with every change ([AGENTS.md](../AGENTS.md)). Their size and redundancy make
drift likely (the overview's list of examples has already fallen
behind `examples/README.md` once), and the goal sets an expectation the product can't
meet.

**Possible fixes:** reword the goal to "each decision has one rule, stated
where it is used"; remove speculative sections whose only purpose is a
future syntax; and state each rule once, linking to it instead of
restating it (the rules of context, for example, are restated in
`items.md`, `frontends.md`, `rationale.md` and `rust.md`).

---

## Low

### L2. Exit codes are surprising for `srl check` and write commands

**Severity Low · Difficulty Easy (but a breaking change)**

- `srl check` exits 0 when it found only orphans, which can be lost
  history (see H4), or only warnings, so neither can gate a commit hook
  or CI as written.
- Write commands exit 1 when they changed nothing (`srl id`, `srl accept`,
  `srl reset`, `srl suspend`). The grep convention fits queries; for
  writes it breaks `set -e` scripts on the normal "already done" case.

**Fix:** decide once, document it in "Conventions", and change it before
more scripts depend on it.

### L3. The default store is used silently from any directory

**Severity Low · Difficulty Easy**

Without `--dir`, `SRL_DIR` or a `.srl` above the current directory, every
command uses the default store in the platform's data directory
([store.md](store.md), "Finding the store"). Once that store exists, running
`srl add notes/x.md --stdin` in the wrong folder writes into it without a
word. A store is otherwise "your study folder", so a hidden one in
`~/.local/share` is an odd default. **Fix:** print the store used on
standard error when it is the default, or drop the default.

### L4. The card ID separator is described as changeable

**Severity Low · Difficulty Trivial**

[cards.md](cards.md), "Card IDs", names the code constant `SUFFIX_SEPARATOR` and
discusses what changing it would do, "accepted while in development". The
separator is part of the log contract, and `srl` is already published
(`cargo install srl` in the README). **Fix:** declare it
fixed, and drop the paragraph about changing it.

### L5. The frontend contract is larger in practice than "one mapping"

**Severity Low · Difficulty Trivial**

The docs say frontends "understand this and nothing else"
([frontends.md](frontends.md)). The same page then lets them suspend
cards and open the note `see` names, and `srl-web` also browses cards
and adds items, using `find`, `cards`, `show`, `collections`, `notes` and
`add`. That is fine, but the contract a real frontend depends on is
several commands' JSON, not one. **Fix:** say that
`srl next --json` is the one mapping needed to study, and that every
`--json` output is a contract too (as "Small fixed contracts" implies).

### L6. `srl-core` is a second, public interface

**Severity Low · Difficulty Easy**

"The CLI is the API. … There is no separate interface for scripts or other
programs." `srl-core` is published on crates.io with about 150 public
types and functions, besides their public fields. Its
README rightly says it is not stable, but anyone can depend on it, and each
release then breaks them. **Fix:** mark the API `#[doc(hidden)]` or say in
the crate description that it exists only for the `srl` binary; keep the
public surface to what `commands.rs` uses.

### L7. The domain layer still leans on modules with I/O

**Severity Low · Difficulty Moderate**

`srl-core` is in layers (see [rust.md](rust.md), "Layers"): a domain of
values, an application, and ports with their adapters, among them the
`Source` trait that isolates where items come from. Two boundaries are
still weaker than the layers suggest:

- The domain names types of modules that do I/O: `Kind` lives in
  `kind.rs`, which also runs kind programs, and replaying the log in
  `domain/schedule.rs` takes `Log`, from `log.rs`, which also reads and
  locks the files. `kind.rs` both defines `qa` and orchestrates the
  programs. Moving `Kind`, `qa` and the log's events into the domain
  would make the layers true by construction, and a crate of its own for
  the domain would let the compiler keep them so.
- The largest files (`kind/program.rs` and `index.rs`, each over 1,000
  lines with tests, then `source/filesystem/xml.rs`) are where a split
  would pay first.

None of this is urgent.

### L9. What a card with varying content means is unspecified

**Severity Low · Difficulty Easy**

`make` "may differ every time" ([cards.md](cards.md), "Calls"), so one card ID
can show different content at each review. That is a real use case, with
or without AI: an arithmetic drill that draws new numbers, a conjugation
card that picks a random verb from a list, a map quiz that shows a random
region. Two things about such cards are left unsaid:

1. **What their memory state means.** FSRS models remembering one fixed
   thing. For a varying card it measures a skill ("multiplying two-digit
   numbers"), and differences in difficulty between draws show up as
   noise in it. That may be exactly what the author wants, but the
   specification never says it is intended, or what to expect.
2. **The log doesn't record what was shown.** A review's `hash` covers the
   item, not the card. For a fixed card that is enough, since the item
   decides the card; for a varying one, later analysis can't tell what
   was asked. Neither `srl next --json` nor `srl review` offers a generic
   way to keep it.

**Possible fixes:** a paragraph in [rationale.md](rationale.md) saying that varying
cards are supported and their memory state is the skill's; and, if
analysis of such cards matters, an optional field in review events, such
as a hash of the card as shown, passed back by the frontend.

### L10. What symbolic links in a store mean is unspecified, and a loop multiplies collections

**Severity Low · Difficulty Easy**

[store.md](store.md), "Which files are read", chooses files by globs and
skips folders whose name starts with `.`, but says nothing of symbolic
links. The code follows them, to files and folders alike, so:

- **A link to a folder above it is followed until the system refuses**: a
  link `loop/up` to `..` makes every collection of the store appear again
  as `loop/up/a.jsonl`, `loop/up/loop/up/a.jsonl` and so on, about forty
  times on Linux, each copy an error for the IDs it repeats.
- **A link can make a file outside the store a collection or a note**,
  read and, for a collection, written, although the store is described as
  one folder that holds everything. A note `leak.md` linked to a file
  elsewhere is shown whole as the context of any item whose `see` names
  it, and a linked folder's collections are changed by `srl add` and
  `srl edit`.

Linking a folder of notes kept elsewhere into a store is a reasonable
thing to do, so the choice takes something away from someone.

**Possible fixes**, each said in [store.md](store.md), "Which files are
read", and tested:

1. **Never follow links to folders**; a link to a file is read as the
   file. The simplest rule, and what most tools that walk a tree do by
   default: loops can't happen, but a linked folder of notes is no longer
   read.
2. **Follow links, visiting each real folder once.** Loops end and linked
   folders keep working, but files outside the store are still read and
   written, so the store is no longer one folder.
3. **Follow only links that lead inside the store.** The safest, but it
   resolves paths, which differs between platforms (junctions on Windows,
   names compared without case on macOS and Windows), so it needs care
   and probably a **TODO (platform)** note.

### L11. A collection named as a Windows device can't exist on Windows

**Severity Low · Difficulty Easy**

Windows reserves the names `CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`
and `LPT1` to `LPT9` for devices, whatever their case and extension:
`con.jsonl` and `notes/Aux.md` name devices, not files. The checks of a
new collection's name ([cli.md](cli.md), `srl add`) refuse empty parts,
`.`, `..` and hidden parts, but not these, so:

- **`srl add con.jsonl` works on Linux and macOS, and fails on Windows**
  with the system's error, against "`srl` behaves the same on Linux,
  macOS and Windows".
- **A store made elsewhere can't be used on Windows** when it holds such
  a file: git and file syncing tools can't create it there, so the
  store's collection, or a note `see` names, is missing.

**Possible fixes:**

1. **Refuse these names everywhere**, in every part of a new
   collection's name, as hidden parts are refused, and say so in
   [sources.md](sources.md), "Collections". Behaves the same on every
   platform and keeps stores portable, at the cost of a rule almost no
   one meets. Existing files with such names could be refused too, as an
   error of their collection, or left alone.
2. **Accept the difference**: a **TODO (platform)** note in
   [formats.md](formats.md) saying such names fail on Windows, and a
   comment above the code that checks names.

### L12. A `[sources]` without sources makes a store that silently has no items

**Severity Low · Difficulty Easy**

`[sources]` replaces the default source rather than adding to it
([sources.md](sources.md), "Configuration"), so an empty `[sources]`
table, left behind while editing the configuration, gives a store with
no source at all. Its collections are still on disk, but nothing says
they are no longer read:

- `srl find` and `srl collections` exit 1, as for an empty store;
- `srl check` exits 0, reporting nothing;
- only `srl add` says why: "the store has no source".

**Possible fixes:**

1. **Make an empty `[sources]` an error of the configuration**
   ("`[sources]` names no source; remove it to use the default source").
   One rule, and the mistake can't go unnoticed. A store that really
   wants no items has no use for `srl`.
2. **Treat an empty `[sources]` as no `[sources]`**, giving the default
   source. Forgiving, but it is one more rule to explain, and a table
   that means its absence is surprising.
3. **Keep it, and have `srl check` warn** that the store has no source.
   Leaves `srl find` as silent as now.

---

## What holds up well

Worth keeping as they are:

- Memory state computed only from the log, so the algorithm, retention
  target and weights can change for the whole history at once.
- The index cache as an optimization only, tested against a cache-free
  path.
- Identity by ID, never by position; `groups` seeing only the fields.
- Collection writes that change only the items changed, verify by
  re-parsing, detect concurrent edits, and write atomically.
- A core that calls no service and judges no answer, with every other
  program, agents included, using the same commands as people.
- The rule for what goes in the specification, guides and examples.

## Suggested order

1. **H4** and **L4**: cheap, and H4 protects history today.
2. **H1** fixes 1 and 2 (read only `YYYY-MM.jsonl`; union merge), then
   decide on per-device logs.
3. **H2**: let commands that write the log record despite unrelated
   errors in any source.
4. **M4**, **L2**, **L9**, **L10**, **L11**, **L12**: small rule
   changes, best made before more programs depend on current behaviour.
5. **M3**: state kinds as the extension mechanism, and settle their
   contract, since every plugin builds on it.
6. **H3**: per-machine config overlay and trust for kind programs.
7. Trim the documents as **M5** says.
8. **L3**, **L5**, **L6**, **L7** when nearby.
