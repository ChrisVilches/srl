# Atoms

An **atom** is one thing you can know or not know: a grammar point, a word, a
concept, a fact. It is the unit that gets scheduled, and the only thing with a
memory state. Questions about an atom (cards) are produced from it on demand;
see [drills.md](drills.md).

## The file

Each atom is one Markdown file under `atoms/`, in any subfolder and with any
filename ending in `.md`. The file has two parts:

- **Front-matter is data.** YAML between `---` lines. Programs read it.
- **The body is prose.** Free Markdown after the front-matter. It is shown to
  people and given to LLMs as context, but no program parses it.

Files are UTF-8. Both `\n` and `\r\n` line endings are read, and a UTF-8 byte
order mark is ignored, so files edited on any platform work.

The smallest useful atom:

```markdown
---
id: a7f3k2
title: 〜をものともせず
tags: [japanese/grammar, jlpt/n1]
cards:
  - q: What does 〜をものともせず mean?
    a: Undeterred by (adversity), braving.
---
Admiring tone; not used about oneself.
```

Its questions come from its facets: here the `cards:` facet gives one card.
You see the question, then the answer and the body, and rate yourself.

## Front-matter fields

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Stable identity. Six characters `[a-z0-9]`, assigned by `srl`. Must be unique in the store. |
| `title` | yes | What the atom is about, as a short Markdown string. A label for listings, file names and search; never a question. |
| `tags` | no | List of tags. A `/` makes them hierarchical: `japanese/grammar`. Tags are case-sensitive. |
| `status` | no | `active` (default), `draft` or `suspended`. |
| `source` | no | Where the knowledge came from, as an opaque string: a path, a URL, a book reference. `srl` stores it and shows it, but never interprets it. |
| *anything else* | no | A **facet**: data for the drill of the same name. `body` and `path` are reserved and cannot be facets (see "Atoms as JSON"). |

### Facets

Every other top-level key is a facet. The facet's name is the name of the
drill that reads it, and the shape of its value is defined by that drill
(`srl-drill-<name> --help` documents it). An atom can have any number of
facets, and each one can produce several cards.

```yaml
cloze:                               # read by the `cloze` drill
  - 嵐を{{ものともせず}}、漁師たちは海へ出た。
  - 周囲の反対を{{ものともせず}}、彼女は起業した。
cards:                               # read by the `cards` drill
  - q: Which particle precedes ものともせず?
    a: を
```

**Cards come only from facets.** An atom without a facet has no cards: it is
kept, listed and found by search, but never studied, and `srl check` reports
it. A facet with no installed drill gives no cards either, and is reported by
`srl check` too.

### The body

The body is the knowledge itself, written for people. No drill reads it. It
is used:

- as the explanation shown after the answer of every card of the atom,
  whichever facet the card came from;
- for text search (`srl find "…"`);
- as context for agents and LLM drills.

The facets and the body describe the same knowledge, so one body explains
every card. The body can be empty. An atom made only of a title and facets is
complete.

## Identity and editing

- **The ID is the identity.** Rename or move the file freely; nothing breaks.
- **Content can be changed at any time without consequences.** Fix a typo,
  rewrite the answer, add examples: the memory state stays as it is. Use your
  editor, `srl edit ID` (opens `$EDITOR`), or `srl edit ID --stdin` (replaces
  the whole file, which stays where it is; see [cli.md](cli.md)).
- **Starting over is explicit.** `srl reset ID` clears the memory state, so the
  atom becomes new again. It never happens automatically.

## Status

| Status | Studied? | Meaning |
|---|---|---|
| `active` | yes | Normal atom. |
| `draft` | no | Proposed, waiting for you. Listed by `srl drafts`. `srl accept SELECTOR` makes matching drafts active. |
| `suspended` | no | Kept but not studied. `srl suspend SELECTOR` and `srl unsuspend SELECTOR` move atoms between `active` and `suspended`. |

Drafts exist so that atoms written by agents or imports never enter your study
queue without your approval. `srl add --draft` creates drafts; accepting many
at once is a single command (`srl accept status:draft tag:jlpt/n1`).

## One atom per thing you can know

An atom should hold exactly one thing that you could know or not know on its
own. Cards produced from different facets of an atom share one memory state,
so the facets must be different views of the same knowledge.

- Conjugations of 食べる and cloze sentences using 食べる: one atom.
- Reading 鮪 and writing 鮪 by hand: two atoms, because you can do one without
  the other.

## Creating atoms

Everything that creates atoms ends in one of these commands, whatever the
original source was (your notes, an article, a PDF, an LLM's own knowledge, an
export from another app):

```sh
srl new                                       # open the editor with a template
srl add --title "鮪" --tag kanji/food          # no facets yet: add them with srl edit
some-producer | srl add --stdin               # JSONL: one atom per line
```

An atom file you wrote yourself needs no command: put it anywhere under
`atoms/` (with `cp`, your editor, …). If it has no `id`, `srl check --fix`
assigns one. Until then, commands skip it and say so.

On `--stdin`, each line is a JSON object with the front-matter fields
(without `id`) and an optional `body` string:

```json
{"title": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"], "source": "Tobira ch.3",
 "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred by (adversity)…"}
```

Fields that are absent or `null` get their defaults. A `body` that does not
end with a newline gets one. Lines are checked before anything is written: if
any line is invalid (not a JSON object, no `title`, an `id`, a reserved name,
`tags` that is not a list of text, an unknown `status`), `srl add` reports the
first such line and adds nothing. Blank lines are ignored.

Add `--draft` to any form to create drafts. `srl add` prints the new IDs, one
per line. With `--dry-run` it prints each ID with the file it would write, and
writes nothing.

New files are placed under `atoms/`, in a folder taken from the first tag, and
named after the title (`atoms/japanese/grammar/をものともせず.md`). You can move
them afterwards. The names follow one rule:

- A **name** is made from text by lowercasing it, keeping letters and digits
  of any script, turning every other run of characters into one `-`, and
  keeping at most 60 characters. `Move semantics in Rust` becomes
  `move-semantics-in-rust`; `〜をものともせず` becomes `をものともせず`. A
  name that Windows reserves for a device (`con`, `prn`, `aux`, `nul`,
  `com0`–`com9`, `lpt0`–`lpt9`) gets a `_` (`con_`) on every platform, so a
  store can be copied between platforms. Names never contain characters that
  any platform forbids, and are lowercase, so they don't clash on file
  systems that ignore case.
- The **folder** is the first tag, with each part between `/` turned into a
  name. Without tags, the file goes directly in `atoms/`.
- The **file** is the title's name (in Unicode NFC) plus `.md`. If the title
  gives an empty name, the file is `ID.md`; if the file already exists, it is
  `NAME-ID.md`.
- **The full path of a new file is at most 259 characters**, counted in
  UTF-16 units as Windows counts them (its limit is 260 including a final
  NUL, unless long paths are enabled). The name is cut short to fit, on
  every platform, so that the rule is the same everywhere. If not even
  `ID.md` fits next to the folder of the first tag, the atom isn't added
  and the error says to move the store to a shorter path or shorten the
  tag.

New files are written with the fields in the order of the table above and
`tags` on one line (`tags: [a, b]`), then the facets, then the body. An
existing file is never overwritten. They are UTF-8 with `\n` line endings on
every platform.

> **TODO (platform):** not run on Windows yet. `srl` itself can read and
> write longer paths there (Rust's standard library uses Windows' long path
> form), so the limit is for other programs: Explorer, many editors, git
> without `core.longpaths`. `srl check` reports existing files over the
> limit (see "Validation"); nothing stops you from making such files by
> hand.

## Atoms as JSON

`srl show --json` and `srl find --json` print one JSON object per atom, in the
same shape as a line of `srl add --stdin`, plus `id` and `path`:

```json
{"id": "a7f3k2", "path": "japanese/grammar/をものともせず.md",
 "title": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"],
 "status": "active", "source": "Tobira ch.3",
 "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred by (adversity)…\n"}
```

- `path` is relative to `atoms/`, with `/` between folders.
- Every front-matter field is present, with its default when the file leaves
  it out: `source` is `null`, `tags` is `[]`, `status` is
  `active`.
- Each facet is a key of its own, between `source` and `body`, in file order.
- Removing `id` and `path` gives a valid line for `srl add --stdin`.

Because of this format, `body` and `path` are **reserved names**: they cannot
be facets.

## Reading atoms

Every command that reads atoms reads every file under `atoms/`. A file that
is not a valid atom (no front-matter, invalid YAML, no `id`, no `title`, a field
of the wrong type) is skipped, and reported on standard error with the reason.
So is a file whose `id` an earlier file (by path) already has. The command
carries on with the other atoms.

## Validation

`srl check` reads every atom, runs every drill that applies to it, and
reports:

- atom files whose full path is longer than new files may be (see "Creating
  atoms"), since some programs on Windows can't open them;
- missing or duplicate IDs (`srl check --fix` assigns IDs to atoms without one);
- missing `title`;
- invalid YAML or unknown `status` values;
- atoms without a facet, which have no cards;
- facets with no installed drill, or whose name can't be a drill name;
- drills that fail, write invalid cards, or write no cards at all, for any
  drill that applies (see [drills.md](drills.md) and [cards.md](cards.md)).

Each problem is reported on standard error, one per line. The exit code is
0 if there are none, and 2 if there are any.

`--fix` gives each file without an `id` a new one, by adding an `id` line
right after the opening `---` and changing nothing else (as `srl accept`
changes `status`). It prints one line per atom fixed: the new ID and the
file, separated by two spaces. It then checks the store as it is after the
fix. With `--dry-run` it prints the same lines, writes nothing, and checks
the store as it is.

## Examples

A vocabulary atom with no body:

```markdown
---
id: k3m9q1
title: 鮪
tags: [kanji/food]
cards:
  - q: 鮪
    a: まぐろ — tuna
---
```

A concept with hand-written cards:

```markdown
---
id: r5t2w8
title: Move semantics in Rust
tags: [rust/ownership]
cards:
  - q: What happens when a non-Copy value is assigned to another variable?
    a: Ownership moves; the source becomes invalid.
  - q: |
      ```rust
      let a = String::new(); let b = a;
      println!("{a}");
      ```
      Does this compile?
    a: No — `a` was moved into `b`.
---
Ownership moves on assignment, on passing by value and on returning. Types that
implement `Copy` (integers, `bool`, `char`, …) are copied instead…
```
