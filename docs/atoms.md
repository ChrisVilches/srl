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
cue: 〜をものともせず
tags: [japanese/grammar, jlpt/n1]
---
Undeterred by (adversity), braving. Admiring tone; not used about oneself.
```

That atom can already be studied without any LLM: the built-in `reveal` drill
shows the cue, then the body, and you rate yourself.

## Front-matter fields

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Stable identity. Six characters `[a-z0-9]`, assigned by `srl`. Must be unique in the store. |
| `cue` | yes | What the atom is about, as a short Markdown string. The default question. |
| `answer` | no | A short answer. If present, `reveal` shows it instead of the whole body. |
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
confusable: [b19x0q, c88p1d]         # read by the `confusable` drill
cards:                               # read by the `cards` drill
  - q: Which particle precedes ものともせず?
    a: を
    accept: [を]
```

A facet with no installed drill is ignored when studying, and reported by
`srl check`.

### The body

The body is the knowledge itself, written for people. It is used:

- as the answer of `reveal` cards when the atom has no `answer`;
- as the explanation shown after answering any card, unless the drill provided
  its own;
- for text search (`srl find "…"`);
- as context for agents and LLM drills.

The body can be empty. An atom made only of a cue and facets is complete.

## Identity and editing

- **The ID is the identity.** Rename or move the file freely; nothing breaks.
- **Content can be changed at any time without consequences.** Fix a typo,
  rewrite the answer, add examples: the memory state stays as it is. Use your
  editor, `srl edit ID` (opens `$EDITOR`), or `srl edit ID --stdin` (replaces
  the whole file).
- **Starting over is explicit.** `srl reset ID` clears the memory state, so the
  atom becomes new again. It never happens automatically.

## Status

| Status | Studied? | Meaning |
|---|---|---|
| `active` | yes | Normal atom. |
| `draft` | no | Proposed, waiting for you. Listed by `srl inbox`. `srl accept SELECTOR` makes matching drafts active. |
| `suspended` | no | Kept but not studied. |

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
srl new                                       # open $EDITOR with a template
srl add --cue "鮪" --answer "tuna (まぐろ)" --tag kanji/food
srl add path/to/atom.md                       # copy a file in, assigning an ID
some-producer | srl add --stdin               # JSONL: one atom per line
```

On `--stdin`, each line is a JSON object with the front-matter fields
(without `id`) and an optional `body` string:

```json
{"cue": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"], "source": "Tobira ch.3",
 "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred by (adversity)…"}
```

Fields that are absent or `null` get their defaults. A `body` that does not
end with a newline gets one. Lines are checked before anything is written: if
any line is invalid (not a JSON object, no `cue`, an `id`, a reserved name,
`tags` that is not a list of text, an unknown `status`), `srl add` reports the
first such line and adds nothing. Blank lines are ignored.

Add `--draft` to any form to create drafts. `srl add` prints the new IDs, one
per line. With `--dry-run` it prints each ID with the file it would write, and
writes nothing.

New files are placed under `atoms/`, in a folder taken from the first tag, and
named after the cue (`atoms/japanese/grammar/をものともせず.md`). You can move
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
- The **file** is the cue's name plus `.md`. If the cue gives an empty name,
  the file is `ID.md`; if the file already exists, it is `NAME-ID.md`.

New files are written with the fields in the order of the table above and
`tags` on one line (`tags: [a, b]`), then the facets, then the body. An
existing file is never overwritten. They are UTF-8 with `\n` line endings on
every platform.

> **TODO (platform):** Windows limits paths to 260 characters unless long
> paths are enabled. A store deep in the file system with a long first tag
> can exceed it, and then `srl add` fails. Decide whether to cap the whole
> path.

## Atoms as JSON

`srl show --json` and `srl find --json` print one JSON object per atom, in the
same shape as a line of `srl add --stdin`, plus `id` and `path`:

```json
{"id": "a7f3k2", "path": "japanese/grammar/をものともせず.md",
 "cue": "〜をものともせず", "answer": null, "tags": ["japanese/grammar", "jlpt/n1"],
 "status": "active", "source": "Tobira ch.3",
 "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred by (adversity)…\n"}
```

- `path` is relative to `atoms/`, with `/` between folders.
- Every front-matter field is present, with its default when the file leaves
  it out: `answer` and `source` are `null`, `tags` is `[]`, `status` is
  `active`.
- Each facet is a key of its own, between `source` and `body`, in file order.
- Removing `id` and `path` gives a valid line for `srl add --stdin`.

Because of this format, `body` and `path` are **reserved names**: they cannot
be facets.

## Reading atoms

Every command that reads atoms reads every file under `atoms/`. A file that
is not a valid atom (no front-matter, invalid YAML, no `id`, no `cue`, a field
of the wrong type) is skipped, and reported on standard error with the reason.
So is a file whose `id` an earlier file (by path) already has. The command
carries on with the other atoms.

## Validation

`srl check` reads every atom and reports:

- missing or duplicate IDs (`srl check --fix` assigns IDs to atoms without one);
- missing `cue`;
- invalid YAML or unknown `status` values;
- facets with no installed drill;
- facets whose drill produces no cards or fails (see [drills.md](drills.md)).

## Examples

A vocabulary atom with no body:

```markdown
---
id: k3m9q1
cue: 鮪
answer: まぐろ — tuna
tags: [kanji/food]
reverse: true
---
```

A concept with hand-written cards:

```markdown
---
id: r5t2w8
cue: Move semantics in Rust
answer: Assigning a non-Copy value moves ownership; the source becomes invalid.
tags: [rust/ownership]
confusable: [r5t2w9]
cards:
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
