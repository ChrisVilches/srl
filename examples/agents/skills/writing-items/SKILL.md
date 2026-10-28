---
name: writing-items
description: The full syntax of srl items (srl blocks, headers, fields, continuation lines, code blocks, cloze deletions), how to write good questions, and how to add or replace items with srl add and srl edit --stdin. Load before writing items for the person, when an item gives an error, or when they ask how to write a particular kind of question.
---

# Writing items

## The syntax, in full

Items live in **srl blocks**: fenced code blocks whose language is `srl`.
A block holds one item, with its entries on the opening fence:

````markdown
```srl id:x7k2m9
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.
```
````

or several, each starting with an `@` line that holds its entries:

````markdown
```srl
@ id:x7k2m9
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.

@ id:p4n8d2 draft
Q: 嵐を{{c1::ものともせず}}、漁師たちは海へ出た。
```
````

1. **A block opens** with a line starting at column 0 with three or more
   backticks or tildes, any spaces, `srl`, then a space or the end of the
   line; the rest of that line is its entries. It closes at the first line
   of at least as many of the same character. A block never closed is an
   error. A fence of backticks can't have a backtick after `srl`; use
   tildes.
2. **The first line of a block decides what it holds**: a field line means
   one item, whose header is the opening fence; an `@` line (`@`, or `@`, a
   space and entries) means every item starts with one, and the fence has
   no entries. An empty block, one starting with anything else, or one
   mixing the two is an error.
3. **Only srl blocks make items.** `Q:` lines outside a block are ordinary
   text.
4. **A field line** starts at column 0 with a key of uppercase letters,
   digits and `_`, a colon, and a space or the end of the line: `Q: text`,
   `A:`. The line right after a header must be a field line.
5. **A continuation line** starts with exactly two spaces and continues the
   field above it. Further spaces are kept, so indented code keeps its
   indentation. A line starting with one space or a tab inside an item is
   an error.
6. **Blank lines** may appear inside an item when more field or
   continuation lines follow. Inside a field they are part of its text.
7. **An item ends** at the next `@` line or the closing fence. Any other
   line in a block that is not part of an item is an error.
8. Lines inside other fenced code blocks of the note are ignored, so a
   note can show the item syntax as an example. YAML front-matter is
   skipped.

Header entries, in any order, each at most once:

| Entry | Meaning |
|---|---|
| `id:ID` | The item's ID. `srl id` adds it; lowercase letters and digits. |
| `draft` | Not studied until `srl accept`. |
| `see:PATH` | The note shown as context, instead of the item's own note. Must be a note of the store. Quote paths with spaces: `see:"my notes/a b.md"`. |
| `kind:NAME` | The kind. `qa` is the default; any other is a kind program named in `[kinds]` of `.srl/config.toml`, whose fields are its own. `srl kinds` describes every kind and its fields. |

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

A field written as `KEY:` alone starts on the next line.

**When a field holds a fenced code block, open the srl block with `~~~srl`
and close it with `~~~`.** The code's closing ```` ``` ```` would close a
block opened with ```` ``` ````. A tilde fence is never closed by
backticks, so there is nothing to count:

````markdown
~~~srl
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
````

Getting this wrong is not always an error. The block closes at the code's
fence, the rest of the item becomes text of the note, and the leftover
closing fence opens a code block that can hide the items after it. `srl
check` reports it only when the cut item is invalid, such as a `qa` item
that lost its `A:`; `srl lint` reports it every time, at the indented
fence that closed the block. So after writing such an item by hand, run
`srl check` **and** `srl lint`, and look at it with `srl show ID`. `srl
add` never makes this mistake: it picks the fence itself.

## Cloze deletions

Written in `Q`, as in Anki:

- `{{c1::text}}` hides `text` in card 1; `{{c1::text::hint}}` shows `hint`
  in its place.
- Each number gives one card, with the number as suffix: item `k3m9q1` with
  `c1` and `c2` gives cards `k3m9q1-1` and `k3m9q1-2`.
- Deletions with the same number are hidden together in one card.
- The prompt shows the hidden parts as `==[...]==` (or `==[hint]==`), the
  revealed prompt shows them as `==text==`, and the answer is `A`, or
  `null` if there is none.
- Errors: a deletion without `}}`, empty text, a deletion inside another,
  `c0` or leading zeros (`c01`), and deletions in `A`.
- Any other `{{` (as in code) is ordinary text.

````markdown
```srl
Q: {{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。
A: Despite everyone's opposition, she started a company.
```
````

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
srl add NOTE [--dry-run] --stdin
```

One JSON object per line on standard input. The lowercase keys are the
item's header entries: `"kind"` (text, default `qa`), `"see"` (text, the
note shown as its context) and `"draft"` (`true` or `false`). Every other
key is a field, and its value the field's text. A newline in a value (`\n`
in JSON) becomes a continuation line, and each item gets an srl block of
its own, with a fence long enough for the code blocks in its fields.
`NOTE` is relative to the store's folder, must be a note (by default, end
in `.md`; see `include` in `.srl/config.toml`), and is created
with its folders if it doesn't exist. `srl notes` lists the notes, also
those without items, for `NOTE` and `"see"`. Items are appended at the
end, each gets a new ID, and the IDs are printed. Every line is checked
first, by the same rules as reading notes, kind programs included: one
bad line and nothing is added. So `--dry-run` is the way to know whether
an item is valid; `srl kinds` only describes the fields.

```sh
srl add generated/rust.md --stdin <<'JSONL'
{"draft": true, "see": "rust/ownership.md", "Q": "Does this compile?\n```rust\nlet a = String::new();\nlet b = a;\nprintln!(\"{a}\");\n```", "A": "No. `a` was moved into `b`."}
{"draft": true, "see": "rust/borrowing.md", "Q": "A `&mut T` reference is {{c1::exclusive}}: no other reference to the value may exist."}
JSONL
```

As an agent, always give every item `"draft": true`, write into a note of
your own (for example under `generated/`), and give each item a `"see"`
when it is about one of the person's notes. Items about different notes
can go in one call. Use `--dry-run` to show the person the items before
writing them. A note of your own may also hold prose: write the note with
its explanation first, then add the items to it.

The person accepts drafts with `srl accept SELECTOR`, for example
`srl accept path:generated/ownership.md` or `srl accept id:x7k2m9`.

## Replacing an item with `srl edit ID --stdin`

Only when the person asks you to change one of their items. Standard input
must be exactly one srl block holding exactly one item, with the same ID;
the rest of the note is untouched. `srl show ID` prints the item in that
form, after its `PATH:LINE`. An item that shares its block with others
keeps its place there: `srl` writes it back as an `@` line and its fields.

````sh
srl show x7k2m9          # see the item as a block of its own
srl edit x7k2m9 --stdin <<'ITEM'
```srl id:x7k2m9
Q: What does 〜をものともせず mean, and who is it used about?
A: Undeterred by (adversity). Used admiringly about others, not oneself.
```
ITEM
````

Quote the heredoc's delimiter (`<<'ITEM'`, not `<<ITEM`): unquoted, the
shell runs the backticks as commands and `srl` gets something else. When a
field holds a code block, give the item as `~~~srl` … `~~~`, as above. An
item that keeps its place in a block shared with others must not close
that block: `srl edit` refuses a field line that would, and says so.

`--dry-run` only checks the input. Editing never changes the card's
schedule. To change it themselves instead, the person opens the note in
their editor: `srl show x7k2m9` prints its `PATH:LINE`, relative to the
store's folder.
