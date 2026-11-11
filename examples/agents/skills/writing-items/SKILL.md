---
name: writing-items
description: How srl items are written (item objects, JSONL, JSON and XML collections, cloze deletions, see and notes), how to write good questions, and how to add, replace or move items with srl add, srl edit --stdin and srl move. Load before writing items for the person, when an item gives an error, or when they ask how to write a particular kind of question.
---

# Writing items

Items live in **collections**: structured files of the store read by the
built-in `filesystem` source, JSONL (`.jsonl`), JSON (`.json`) or XML
(`.xml`), or whatever a source program keeps, such as a database table.
`srl collections` lists them and `srl sources` describes each source
and its settings (a `filesystem` source's globs say which files it reads). **Notes are not collections**: the person's Markdown and
other study material hold no items. An item names the note it asks about
with `see`, and that note is shown as its context. `srl notes` lists the
files an item may name.

Whatever the collection, an item is added and changed with `srl add` and
`srl edit --stdin`, as an **item object**, one JSON object per line.

## The item object

```json
{"id": "x7k2m9", "see": "japanese/grammar/wo.md", "Q": "What does 〜をものともせず mean?", "A": "Undeterred by (adversity), braving."}
{"see": "japanese/grammar/wo.md", "draft": true, "Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。"}
```

- **Lowercase keys are header entries**, each at most once:

  | Entry | Meaning |
  |---|---|
  | `id` | The item's ID. `srl add` and `srl id` make one; lowercase letters and digits, words joined by single `-` (`jp-grammar-desu`), 1 to 32 characters, never `.`. |
  | `draft` | `true`: not studied until `srl accept`. |
  | `see` | The note shown as the item's context: a path relative to the store's folder that `srl notes` lists, such as `japanese/grammar/wo.md`. Without it, the item has no context. |
  | `kind` | The kind. `qa` is the default; any other is a kind program named in `[kinds]` of `.srl/config.toml`, whose fields are its own. `srl kinds` describes every kind and its fields. |

  `"kind": "qa"`, `"see": null` and `"draft": false` are the same as
  leaving them out.
- **Uppercase keys are fields**, and their values the fields' text. A
  newline in a field is `\n`; code needs no special treatment.
- Any other key is an error, except `collection`, `source`, `cards` and
  `context`, which `srl show --json` adds and every reader ignores. A key
  written twice is an error.

An item without an ID is an error for every command except `srl id`, which
fixes it. Two items with the same ID (usually a copied item) is an error:
delete the ID from the copy and run `srl id`.

The ID is the item's identity: editing it, moving it within or between
collections, or renaming its collection never changes its schedule.

## Collection formats

- **JSONL** (`.jsonl`): one item object per line; blank lines are allowed.
  The format for items written by programs and agents. `srl` writes a
  changed item's line again, compact; the other lines stay as they are.
- **JSON** (`.json`): one array of item objects. `srl` writes the whole
  array again when it changes it. Not read by default: a source reads it
  only if its `include` says so.
- **XML** (`.xml`): made for writing items by hand, with fields of many
  lines written as they read. Entries are attributes of `<item>`, fields
  are elements:

  ```xml
  <items>
    <item id="b19x0q" see="rust/ownership.md">
      <Q>
        Does this compile?

            let a = String::new();
            let b = a;
            println!("{a}");
      </Q>
      <A>No. `a` was moved into `b`, so it can't be used.</A>
    </item>
    <item draft="true" see="rust/borrowing.md">
      <Q><![CDATA[A `&mut T` reference is {{c1::exclusive}}.]]></Q>
    </item>
  </items>
  ```

  A field over several lines loses every blank line it starts and ends
  with and the indentation its lines share. Text with `<` or `&` goes in CDATA, which is
  taken exactly. `srl` rewrites only the `<item>` elements it changes.

Errors name the line (JSONL, XML) or the element (JSON) they are on.

## Fields of the `qa` kind

| Field | Required |
|---|---|
| `Q` | always |
| `A` | unless `Q` has cloze deletions |

No other field is allowed, and each appears once.

## Cloze deletions

Written in `Q`, as in Anki:

- `{{c1::text}}` hides `text` in card 1; `{{c1::text::hint}}` shows `hint`
  in its place.
- Each number gives one card, with the number as suffix: item `k3m9q1` with
  `c1` and `c2` gives cards `k3m9q1.1` and `k3m9q1.2`.
- Deletions with the same number are hidden together in one card.
- The prompt shows the hidden parts as `==[...]==` (or `==[hint]==`), the
  revealed prompt shows them as `==text==`, and the answer is `A`, or
  `null` if there is none.
- Errors: a deletion without `}}`, empty text, a deletion inside another,
  `c0` or leading zeros (`c01`), and deletions in `A`.
- Any other `{{` (as in code) is ordinary text.
- `srl check` warns of likely mistakes in `Q`: `{{C1::…}}` or `{{c1:…}}`,
  which are shown as written, and a hint that gives the answer away.

```json
{"Q": "{{c1::周囲の反対}}を{{c1::ものともせず}}、{{c2::彼女::who}}は起業した。", "A": "Despite everyone's opposition, she started a company."}
```

Cloze numbers are card identities. Renumbering them, or adding the first
cloze to an item that had none (its card `x7k2m9` becomes `x7k2m9.1`),
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
- **Give an item a `see`** when a note covers what it asks about. The
  note is shown after the answer, so the explanation does not need
  repeating in `A`. An item without `see` has no context, which is fine
  when there is no note to name; `A` can then carry a short explanation.
- `==text==` in a field is a highlight in the card too.

## Adding items with `srl add`

```sh
srl add COLLECTION [--dry-run] --stdin
```

One item object per line on standard input. `COLLECTION` names the
collection: for a file, its path relative to the store's folder, such as
`generated/rust.jsonl`. A collection that doesn't exist is created where
exactly one source of the store accepts it, in the format its extension
names (`srl sources` shows each `filesystem` source's globs; by default
`**/*.jsonl` and `**/*.xml`). Each source adds the items its own way: a
line each in JSONL, an element each before `</items>` in XML. A note is
never a collection: `srl add` never writes to the person's notes. The new
IDs are printed. Every line is checked first, by the same rules as
reading items, kind programs included: one bad line and nothing is added.
So `--dry-run` is the way to know whether an item is valid; `srl kinds`
only describes the fields.

```sh
srl add generated/rust.jsonl --stdin <<'JSONL'
{"draft": true, "see": "rust/ownership.md", "Q": "Does this compile?\n```rust\nlet a = String::new();\nlet b = a;\nprintln!(\"{a}\");\n```", "A": "No. `a` was moved into `b`."}
{"draft": true, "see": "rust/borrowing.md", "Q": "A `&mut T` reference is {{c1::exclusive}}: no other reference to the value may exist."}
JSONL
```

As an agent, always give every item `"draft": true`, write into a
collection of your own (for example `generated/rust.jsonl`). `see` is
optional, and never a reason not to write items. Unless the person named
a note, look through `srl notes`, read the likely ones, and give an item
a `"see"` naming the note it is about when one covers it. If the person
asked you to write notes, write them first (Markdown files of their own)
and name them with `see`. If they didn't and no note fits, write the items
without `see`. Items about different notes, or about none, can go in one
call. Use `--dry-run` to show the person the items before writing them.

The person accepts drafts with `srl accept SELECTOR`, for example
`srl accept in:generated in:rust.jsonl` or `srl accept id:x7k2m9`.

## Replacing items with `srl edit --stdin`

Only when the person asks you to change their items. Standard input holds
item objects, one per line, as for `srl add`, each with the `"id"` of the
item it replaces, in any collection; the rest of each collection is
untouched. `srl show --json ID` prints the item in that form, with
`collection`, `source`, `cards` and `context`, which `srl edit` ignores:
change what you need and give the line back. Each object is the whole new
item: a field or entry left out is gone, not kept.

```sh
srl show --json --no-context x7k2m9   # the item object, to change
srl edit --stdin <<'JSONL'
{"id": "x7k2m9", "Q": "What does 〜をものともせず mean, and who is it used about?", "A": "Undeterred by (adversity). Used admiringly about others, not oneself."}
JSONL
```

Quote the heredoc's delimiter (`<<'JSONL'`, not `<<JSONL`): unquoted, the
shell runs backticks as commands and `srl` gets something else. Each
item's source writes it its own way: a JSONL item as its line again, an
XML item as its `<item>` element again, in place, a JSON collection whole.
Every line is checked before anything is written, and the IDs are
printed.

`--dry-run` checks the input and prints the IDs, and writes nothing.
Editing never changes the card's schedule. To change it themselves
instead, the person opens the collection in their editor: `srl show
x7k2m9` prints its collection, which is the file's path for a collection of
the `filesystem` source. An item of a source program, such as in a
database, has no file: change it with `srl edit --stdin`.

## Moving items with `srl move`

Only when the person asks. `srl move ID... COLLECTION` moves items, from
any source, to a collection that already exists, named as
`srl collections` lists it; it never creates one. The items keep their
IDs, so their cards keep their schedule.

```sh
srl move x7k2m9 k3m9q1 japanese/grammar/archive.jsonl --dry-run
srl move x7k2m9 k3m9q1 japanese/grammar/archive.jsonl
```

It prints the IDs of the items moved; an item already in the collection
is left where it is.
