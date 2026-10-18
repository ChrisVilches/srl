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
| `id` | yes | Stable identity. Six characters `[a-z0-9]`, assigned by `srs`. Must be unique in the store. |
| `cue` | yes | What the atom is about, as a short Markdown string. The default question. |
| `answer` | no | A short answer. If present, `reveal` shows it instead of the whole body. |
| `tags` | no | List of tags. A `/` makes them hierarchical: `japanese/grammar`. Tags are case-sensitive. |
| `status` | no | `active` (default), `draft` or `suspended`. |
| `source` | no | Where the knowledge came from, as an opaque string: a path, a URL, a book reference. `srs` stores it and shows it, but never interprets it. |
| *anything else* | no | A **facet**: data for the drill of the same name. |

### Facets

Every other top-level key is a facet. The facet's name is the name of the
drill that reads it, and the shape of its value is defined by that drill
(`srs-drill-<name> --help` documents it). An atom can have any number of
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
`srs check`.

### The body

The body is the knowledge itself, written for people. It is used:

- as the answer of `reveal` cards when the atom has no `answer`;
- as the explanation shown after answering any card, unless the drill provided
  its own;
- for text search (`srs find "…"`);
- as context for agents and LLM drills.

The body can be empty. An atom made only of a cue and facets is complete.

## Identity and editing

- **The ID is the identity.** Rename or move the file freely; nothing breaks.
- **Content can be changed at any time without consequences.** Fix a typo,
  rewrite the answer, add examples: the memory state stays as it is. Use your
  editor, `srs edit ID` (opens `$EDITOR`), or `srs edit ID --stdin` (replaces
  the whole file).
- **Starting over is explicit.** `srs reset ID` clears the memory state, so the
  atom becomes new again. It never happens automatically.

## Status

| Status | Studied? | Meaning |
|---|---|---|
| `active` | yes | Normal atom. |
| `draft` | no | Proposed, waiting for you. Listed by `srs inbox`. `srs accept SELECTOR` makes matching drafts active. |
| `suspended` | no | Kept but not studied. |

Drafts exist so that atoms written by agents or imports never enter your study
queue without your approval. `srs add --draft` creates drafts; accepting many
at once is a single command (`srs accept status:draft tag:jlpt/n1`).

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
srs new                                       # open $EDITOR with a template
srs add --cue "鮪" --answer "tuna (まぐろ)" --tag kanji/food
srs add path/to/atom.md                       # copy a file in, assigning an ID
some-producer | srs add --stdin               # JSONL: one atom per line
```

On `--stdin`, each line is a JSON object with the front-matter fields
(without `id`) and an optional `body` string:

```json
{"cue": "〜をものともせず", "tags": ["japanese/grammar", "jlpt/n1"], "source": "Tobira ch.3",
 "cloze": ["嵐を{{ものともせず}}、漁師たちは海へ出た。"], "body": "Undeterred by (adversity)…"}
```

Add `--draft` to any form to create drafts. `srs add` prints the new IDs.

New files are placed under `atoms/`, in a folder taken from the first tag, and
named after the cue (`atoms/japanese/grammar/wo-mono-tomo-sezu.md`). You can
move them afterwards.

## Validation

`srs check` reads every atom and reports:

- missing or duplicate IDs (`srs check --fix` assigns IDs to atoms without one);
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
