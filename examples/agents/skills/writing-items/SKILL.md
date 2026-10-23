---
name: writing-items
description: The full syntax of srl items (headers, fields, continuation lines, code blocks, cloze deletions), how to write good questions, and how to add or replace items with srl add and srl edit --stdin. Load before writing items for the person, when an item gives an error, or when they ask how to write a particular kind of question.
---

# Writing items

## The syntax, in full

```markdown
<!-- srl id:x7k2m9 -->
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.
```

1. **A header** is a line starting at column 0 with `<!--`, then `srl`, and
   ending with `-->`, all on one line, with nothing after it. Spaces inside
   are free: `<!--srl-->` and `<!-- srl id:x7k2m9 -->` are both headers.
   A line that starts like a header but isn't one (no `-->`, or text after
   it) is an error.
2. **Only headers make items.** `Q:` lines without a header above them are
   ordinary text.
3. **A field line** starts at column 0 with a key of uppercase letters,
   digits and `_`, a colon, and a space or the end of the line: `Q: text`,
   `A:`. The line right after a header must be a field line.
4. **A continuation line** starts with exactly two spaces and continues the
   field above it. Further spaces are kept, so indented code keeps its
   indentation. A line starting with one space or a tab inside an item is
   an error.
5. **Blank lines** may appear inside an item when more field or
   continuation lines follow. Inside a field they are part of its text.
6. **An item ends** at the first non-blank line that is neither a field nor
   a continuation line, or at the end of the file.
7. Lines inside fenced code blocks of the note (outside items) are ignored,
   so a note can show the item syntax as an example. YAML front-matter is
   skipped.

Header entries, in any order, each at most once:

| Entry | Meaning |
|---|---|
| `id:ID` | The item's ID. `srl id` adds it; lowercase letters and digits. |
| `draft` | Not studied until `srl accept`. |
| `see:PATH` | The note shown as context, instead of the item's own note. Must be a note of the store. Quote paths with spaces: `see:"my notes/a b.md"`. |
| `kind:qa` | The kind. `qa` is the default and the only one. |

An item without an ID is an error for every command except `srl id`, which
fixes it. Two items with the same ID (usually a copied item) is an error:
delete the ID from the copy and run `srl id`.

The ID is the item's identity: editing it, moving it within or between
notes, or renaming the note never changes its schedule.

## Fields of the `qa` kind

| Field | Required |
|---|---|
| `Q` | always |
| `A` | unless `Q` has cloze deletions |

No other field is allowed, and each appears once.

## Multi-line fields and code

A field written as `KEY:` alone starts on the next line:

~~~markdown
<!-- srl -->
Q: Does this compile?
  ```rust
  let a = String::new();
  let b = a;
  println!("{a}");
  ```

A:
  No. `a` was moved into `b`, so it can't be used.

  Types that implement `Copy` are copied instead.
~~~

Many Markdown viewers join consecutive lines into one paragraph, so put a
blank line between `Q` and `A` if the person's viewer shows them on one
line. `srl` reads both the same.

## Cloze deletions

Written in `Q`, as in Anki:

- `{{c1::text}}` hides `text` in card 1; `{{c1::text::hint}}` shows `hint`
  in its place.
- Each number gives one card, with the number as suffix: item `k3m9q1` with
  `c1` and `c2` gives cards `k3m9q1-1` and `k3m9q1-2`.
- Deletions with the same number are hidden together in one card.
- The prompt shows the hidden parts as `==[...]==` (or `==[hint]==`), the
  answer shows them as `==text==`, followed by `A` if there is one.
- Errors: a deletion without `}}`, empty text, a deletion inside another,
  `c0` or leading zeros (`c01`), and deletions in `A`.
- Any other `{{` (as in code) is ordinary text.

```markdown
<!-- srl -->
Q: {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。
A: Despite everyone's opposition, she started a company.
```

Cloze numbers are card identities. Renumbering them, or adding the first
cloze to an item that had none (its card `x7k2m9` becomes `x7k2m9-1`),
leaves the old card's history behind; see the `fixing-problems` skill.

## Writing good questions

- **One fact per question**, with one answer the person can check
  themselves in a second. Split lists and long answers into several items.
- **Ask for understanding as well as recall**: "Why ...", "What happens
  if ...", "Does this compile?".
- **Give enough context in the question** that it has only one right
  answer, but never the answer itself.
- **Cloze for sentences and fixed phrases**: vocabulary in context,
  grammar patterns, formulas. Hide the part worth remembering, not filler.
- **Write the item next to what it asks about.** The note around it is
  shown after the answer, so the explanation does not need repeating in `A`.
- `==text==` in a field is a highlight in the card too.

## Adding items with `srl add`

```sh
srl add NOTE [--see PATH] [--draft] [--dry-run] --stdin
```

One JSON object per line on standard input; each key is a field, each value
its text. A newline in a value (`\n` in JSON) becomes a continuation line.
`NOTE` is relative to the store's folder, ends in `.md`, and is created
with its folders if it doesn't exist. Items are appended at the end, each
gets a new ID, and the IDs are printed. Every line is checked first: one
bad line and nothing is added.

```sh
srl add generated/ownership.md --see rust/ownership.md --draft --stdin <<'JSONL'
{"Q": "Does this compile?\n```rust\nlet a = String::new();\nlet b = a;\nprintln!(\"{a}\");\n```", "A": "No. `a` was moved into `b`."}
{"Q": "A `&mut T` reference is {{c1::exclusive}}: no other reference to the value may exist."}
JSONL
```

As an agent, always add `--draft`, write into a note of your own (for
example under `generated/`), and add `--see` when the items are about one
of the person's notes. Use `--dry-run` to show the person the items before
writing them. A note of your own may also hold prose: write the note with
its explanation first, then add the items to it.

The person accepts drafts with `srl accept SELECTOR`, for example
`srl accept path:generated/ownership.md` or `srl accept id:x7k2m9`.

## Replacing an item with `srl edit ID --stdin`

Only when the person asks you to change one of their items. Standard input
must be exactly one item, header included, with the same ID; the rest of
the note is untouched.

```sh
srl show x7k2m9          # see the item as written
srl edit x7k2m9 --stdin <<'ITEM'
<!-- srl id:x7k2m9 -->
Q: What does 〜をものともせず mean, and who is it used about?
A: Undeterred by (adversity). Used admiringly about others, not oneself.
ITEM
```

`--dry-run` only checks the input. Editing never changes the card's
schedule. To open the note in the person's editor instead, they run
`srl edit x7k2m9`.
