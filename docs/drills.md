# Drills

A **drill** is a program that turns one facet of an atom into cards. Drills
are how `srl` supports different kinds of questions without the core knowing
anything about them: a new kind of question is a new drill, and the core,
the frontends and the log stay unchanged.

```
facet in the atom (any shape) ──► drill (any program) ──► cards (fixed format)
conj: {verb: 食べる, …}            srl-drill-conj           {"prompt": "て-form of 食べる?", …}
```

## Rules

1. **A drill is named after its facet.** The drill `conj` reads the facet
   `conj:`.
2. **A drill runs for an atom only if the atom has its facet.** One built-in
   exception: `reveal` runs for every atom.
3. **All drills that apply to an atom run, and their cards go into one pool.**
   `srl next` picks one card from that pool (see
   [scheduling.md](scheduling.md)).
4. **Cards are not stored.** They are produced when needed and thrown away.
   The review event records which card was used.
5. **Drills should be deterministic.** The same atom must give the same cards.
   A drill that needs randomness (numbers in a formula, the order of choices)
   seeds it from the atom ID and the current study day, so a card stays the
   same for a whole day and changes the next.
6. **A drill that fails is skipped.** Studying continues with the other drills'
   cards; the error is reported on standard error and by `srl check`.

## Finding drills

A drill named `NAME` is the first of:

1. an executable `drills/srl-drill-NAME` in the store;
2. an executable `srl-drill-NAME` on `PATH`;
3. a built-in drill.

So a store or a user can replace a built-in drill by providing a program with
the same name.

A **drill name** is made of lowercase letters `a`–`z`, digits, `-` and `_`.
A facet whose name is anything else has no drill, and `srl check` says so.
This keeps facet names from ever being read as paths.

"Executable" means what the platform's shell means by it, for `drills/` and
`PATH` alike:

- On Unix, `srl-drill-NAME` is a file with the execute permission, and a
  script runs through its `#!` line (like the Python example below). A file
  with an extension, such as `srl-drill-NAME.py`, is not the drill `NAME`.
- On Windows there is no execute permission and `#!` lines are ignored, so
  the drill is `srl-drill-NAME` plus an extension listed in `PATHEXT`
  (`.exe`, `.cmd`, `.bat`, …). A Python drill needs a `.cmd` wrapper.

`srl drills` lists every drill found, by name, one line each: the name,
where it was found (`built-in` or the program's path) and the first line of
its help, separated by two spaces. For a program, that is the first
non-empty line of `srl-drill-NAME --help`, without a leading Markdown `#`.
With `--json`, each line is `{"name": N, "path": P, "summary": S}`, where
`path` is `null` for a built-in drill. A drill whose `--help` fails is listed
without a summary (`null` in JSON) and reported, and the exit code is 2.

## The drill protocol

A drill is any executable. `srl` runs it with no arguments, writes one atom as
JSON on its standard input, and reads cards as JSONL from its standard output.

Input:

```json
{"id": "v81k3z",
 "path": "atoms/japanese/verbs/taberu.md",
 "frontmatter": {"id": "v81k3z", "cue": "食べる", "conj": {"verb": "食べる", "class": "ichidan", "forms": ["te", "past"]}},
 "body": "…",
 "day": "2026-10-26"}
```

`day` is the current study day, for seeding (rule 5).

Output: zero or more cards, one JSON object per line, in the card format of
[cards.md](cards.md), with these simplifications:

- `drill` may be left out; `srl` fills it in.
- `ref` only needs to be unique within this drill's output for this atom;
  `srl` prefixes it with the drill name (`te` becomes `conj:te`).
- `atoms` may be left out; it defaults to `[id]`.
- `explanation` may be left out; it defaults to the atom's body. Set it to an
  empty string to show no explanation.

Exit status 0 means success, even with no cards. Any other status, output
that is not UTF-8, or taking longer than `[drills] timeout_seconds`, means
the drill failed. A drill that takes too long is killed. What a drill writes
to standard error reaches the person running `srl` directly.

Each line of output is one card. Blank lines are ignored. A line that is not
a valid card (see [cards.md](cards.md), rule 4), or whose `ref` an earlier
card already has, is dropped and reported; the other cards are kept.

### Help

`srl-drill-NAME --help` must print, in Markdown:

1. a one-line summary;
2. the shape of the facet, with every field explained;
3. one complete example atom.

This text is how people and agents learn to write the facet. It is the only
documentation a facet has.

### Example: a conjugation drill

```python
#!/usr/bin/env python3
"""srl-drill-conj: conjugation cards for Japanese ichidan verbs."""
import json, sys

if sys.argv[1:] == ["--help"]:
    print(__doc__)
    print("Facet: conj: {verb: dictionary form, class: ichidan, forms: [te, past, negative]}")
    sys.exit(0)

atom = json.load(sys.stdin)
c = atom["frontmatter"]["conj"]
stem = c["verb"][:-1]
forms = {"te": stem + "て", "past": stem + "た", "negative": stem + "ない"}
for f in c["forms"]:
    print(json.dumps({
        "ref": f,
        "prompt": f"{f} form of {c['verb']}?",
        "answer": forms[f],
        "grading": "exact",
        "accept": [[forms[f]]],
        "explanation": "Ichidan verbs: drop る and add the ending.",
    }, ensure_ascii=False))
```

## Built-in drills

A built-in drill fails, like any drill, when its facet doesn't have the shape
described here. The error says what is wrong.

### `reveal`

Runs for every atom. One card:

- prompt: the `cue`;
- answer: `answer` if present, otherwise the body (and then no explanation);
- grading: `self`.

### `reverse`

Facet: `reverse: true`. One card, the other way round:

- prompt: `answer` if present, otherwise the body (and then no
  explanation);
- answer: the `cue`;
- grading: `self`.

`reverse: false` gives no card. An atom with neither an `answer` nor a body
has nothing to ask about, so the drill fails.

### `cloze`

Facet: a list of Markdown strings with marked spans.

```yaml
cloze:
  - 嵐を{{ものともせず}}、漁師たちは海へ出た。
  - {{1::周囲の反対}}を{{1::ものともせず}}、{{2::彼女}}は起業した。
```

- A span is `{{text}}` or `{{N::text}}`, where `N` is a group number made of
  digits. Anything else before `::` is part of the text: `{{std::move}}` is
  one span with the text `std::move`.
- Spans with the same number are blanked together in one card. A span without
  a number gets its own group: the spans without a number get the numbers
  after the highest number in the sentence, in order (`{{a}} {{2::b}} {{c}}`
  are groups 3, 2 and 4).
- Every sentence needs at least one span. A span can't be empty, and every
  `{{` needs a closing `}}`.
- **One card per group per sentence.** Spans of other groups appear filled in,
  as plain text.
- Blanks in the prompt are written `==[1]==`, `==[2]==`, … in order. The answer
  is the full sentence with the blanked spans highlighted.
- grading: `exact`, with one `accept` entry per blank.
- ref: `S:G`, the sentence index and the group number (`cloze:1:1`).

```json
{"ref": "cloze:1:1",
 "prompt": "==[1]==を==[2]==、彼女は起業した。",
 "answer": "==周囲の反対==を==ものともせず==、彼女は起業した。",
 "grading": "exact", "accept": [["周囲の反対"], ["ものともせず"]]}
```

### `cards`

Facet: a list of hand-written questions.

```yaml
cards:
  - q: Which particle precedes ものともせず?
    a: を
    accept: [を]              # optional: makes the card `exact`
    explanation: …            # optional
```

- One card per item. `q` becomes the prompt and `a` the answer. Both are
  required, and an item has no other keys than the four above.
- grading: `exact` if `accept` is present (one blank, any of the listed
  strings), otherwise `self`.
- ref: the item's index (`cards:0`).

These cards are also useful guidance when an LLM generates more questions for
the atom.

### `confusable`

Facet: a list of IDs of atoms that are easily confused with this one.

```yaml
confusable: [b19x0q, c88p1d]
```

One multiple-choice card:

- prompt: `Which one matches?`, a blank line, and this atom's `answer` (or
  body, and then no explanation);
- choices: the `cue` of this atom and of the first three atoms listed, in an
  order seeded by atom ID and day;
- grading: `choice`, with `accept: [[this atom's cue]]`;
- ref: `confusable`.

An empty list gives no card. An ID that no atom has, the atom's own ID, or an
atom with neither an `answer` nor a body makes the drill fail.

## Drills that use an LLM

The core has no LLM drill, but there are two ways to bring LLM-made questions
into the system:

- **The LLM writes facet data.** An agent adds more sentences to `cloze:`, more
  items to `cards:`, more `confusable:` links. The ordinary drills then use
  that data as-is, offline and for free. This is just editing the atom.
- **An `llm` drill writes cards.** A drill program named `srl-drill-llm` reads
  the whole atom as context and asks an LLM for cards. Like any drill, it only
  runs on atoms with its facet, for example
  `llm: {focus: "application questions using food vocabulary"}`. It can call
  the model live or keep its own cache of earlier answers; any caching belongs
  to the drill program, not to `srl`.

Either way, facets are never templates for the LLM. It either writes data in
a facet's shape, or reads facets as knowledge and writes cards.
