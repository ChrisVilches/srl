# Notes

A **note** is a file of the store that a source of one of the two
built-in drivers, `markdown` and `jsonl`, reads for items: by default, any
Markdown file (see "Which files are notes"). Everything else in a note is
yours: `srl` reads it only to show it as context, and never changes it.

The built-in drivers are sources like any other (see
[sources.md](sources.md)): they answer what every source answers, and the
rest of `srl` knows nothing of notes, files or how items are written in
them. They are built in so that a store of Markdown notes needs no program
and no configuration; a source program could do the same work. Items
themselves, whatever their source, are described in [items.md](items.md).
This document describes the notes of the built-in drivers: which files
they are, how their items are written in them, what they show as
context, and how `srl` writes to them.

Each driver writes items in its own **item syntax** (see "Item
syntaxes"):

- **srl blocks**, in the notes of a `markdown` source: fenced code blocks
  whose language is `srl`, inside your Markdown. A block holds one item,
  with the item's entries on its opening fence, or several, each starting
  with an `@` line that holds its entries.
- **JSONL**, in the notes of a `jsonl` source: one item per line, as a
  JSON object. These notes are data, written by programs such as
  importers and generators, and hold nothing but items.

````markdown
# 〜をものともせず

Undeterred by (adversity), braving. Admiring tone; not used about oneself.

```srl id:x7k2m9
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.
```

```srl
@ id:p4n8d2
Q: 嵐を{{c1::ものともせず}}、漁師たちは海へ出た。

@ id:r5t1w8 draft
Q: What does 〜をものともせず say about the speaker?
A: That they admire whoever is braving the adversity.
```
````

Rendered, each block is a code block that shows its items as written, set
apart from the prose. In an editor, it reads the same. The same items in a
JSONL note:

```json
{"id": "x7k2m9", "see": "japanese/grammar/wo-mono-tomo-sezu.md", "Q": "What does 〜をものともせず mean?", "A": "Undeterred by (adversity), braving."}
{"id": "p4n8d2", "see": "japanese/grammar/wo-mono-tomo-sezu.md", "Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。"}
```

## Which files are notes

**A file is a note of a source when a glob of the source's `include`
matches its path, no glob of its `exclude` does, and it is not in a
folder whose name starts with `.`** (`.srl/`, `.git/`, `.obsidian/`, …).
Each source sets both in `.srl/config.toml` (see [sources.md](sources.md),
"Configuration"). Without `[sources]`, a store has one source, `notes`,
with driver `markdown` and `include = ["**/*.md"]`: every Markdown file is
a note of srl blocks.

So JSONL notes are read only by a `jsonl` source, such as
`[sources.data]` with `driver = "jsonl"`, whose `include` is
`["**/*.jsonl"]` unless it says otherwise; `"items/**/*.jsonl"` keeps them
in one folder. Every file a source's `include` matches must be a valid
note, so a glob that matches files meant for other programs makes them
errors. A file belongs to one source at most (see [sources.md](sources.md),
"Addresses").

Each note is a **collection**, a group of items. Its name is its **path**,
relative to the store's folder, with `/` between folders, and its
**address**, the name it has everywhere else, is its source's prefix
followed by its path: for a note of a source without a prefix, as
`notes` has, its path. JSON outputs give the address of the note an item
is in (`address`), and of the note its context comes from
(`context_address`). `srl collections` lists every collection, including
the notes that hold no items (see [cli.md](cli.md)).

Files are UTF-8. Both `\n` and `\r\n` line endings are read, and a UTF-8
byte order mark is ignored.

## Item syntaxes

**A note is in the item syntax of its source's driver**: srl blocks for
`markdown`, JSONL for `jsonl`. Nothing else decides it, not even the
note's name or what it holds.

Every note of a `markdown` source is read the same way, whatever its
name: a `.txt` note has the same srl blocks, fenced code blocks and
context as a `.md` note, and its fields are formatted as in
[cards.md](cards.md), "Text formatting". The srl block syntax is
Markdown's, so it suits Markdown and the formats built on it, such as MDX
and Quarto; which other files make sense as notes is up to you.

Both syntaxes write the same items: the same header entries, fields, IDs
and status (see [items.md](items.md)). Everything after reading the items
(kinds, cards, scheduling, the log, selectors) is the same for both, and
for every source. They differ in what else a note holds, and so in its
context (see "Context"), and in how `srl` writes to it (see "How srl
writes to notes").

## The srl block syntax

An item is a **header** followed by **field** lines, inside an srl block.
The rules, in full:

1. **An srl block opens** with a line that starts, at column 0, with three
   or more backticks or tildes, then any number of spaces, `srl`, then a
   space or the end of the line. What follows `srl` is the fence's
   **entries** (see [items.md](items.md), "Header entries").
   ```` ```srl ````, ```` ``` srl id:x7k2m9 ```` and `~~~~srl` all open
   blocks.
2. **It closes** at the first line made of at least as many of the same
   character, after at most three spaces, with nothing after them but
   spaces: the closing fence of CommonMark. **A block that is never closed
   is an error**, since a renderer would show the rest of the note as
   code. So is a fence of backticks with a backtick after `srl`, because
   CommonMark doesn't read that line as a fence; open the block with
   tildes instead.
3. **The first line of a block decides what it holds.** It must be a field
   line or an `@` line:
   - **A field line: the block holds one item.** Its opening fence is the
     item's header, and the fence's entries are the item's.
   - **An `@` line: each item starts with one.** An `@` line is `@` at
     column 0, then a space and the item's entries, or nothing: `@`,
     `@ id:x7k2m9 draft`. It is the header of the item below it. The
     opening fence then has no entries.

   A block that is empty, or starts with anything else, is an error. So is
   a block that mixes the two: an `@` line in a block that holds one item,
   or entries on the fence of a block whose items start with `@` lines.
4. **Only srl blocks make items.** `Q:` lines outside a block are ordinary
   text, and are not reported (but see `srl lint` in [cli.md](cli.md)).
5. **A field line** starts, at column 0, with a key made of uppercase
   letters `A`–`Z`, digits and `_`, beginning with a letter, then a colon,
   then a space or the end of the line: `Q: text`, `A:`. The field's text
   starts after that space.
6. **The line right after a header must be a field line.** A header
   followed by anything else is an error.
7. **A continuation line** starts with exactly two spaces. It continues the
   field above it, on a new line. The two spaces are removed, and any
   further spaces are kept as part of the text, so indented code keeps its
   indentation.
8. **A line that starts with one space or a tab, inside an item, is an
   error**, because it can only be a mistyped continuation.
9. **Blank lines may appear inside an item**, between fields or inside a
   field, when the next non-blank line is a field line or a continuation
   line. A blank line inside a field is part of its text.
10. **An item ends** at the next `@` line or at the closing fence. Blank
    lines before them are not part of the item.
11. **Inside a block, every other line is an error**: a line that is not a
    header, a field line, a continuation line or blank is usually a
    continuation line without its two spaces.
12. **A field holding a fenced code block needs a block with a longer
    fence**, or one of tildes: the continuation line that ends the code
    block, two spaces and ```` ``` ````, closes a block opened with
    ```` ``` ````, in `srl` as in every CommonMark renderer. The rest of
    the item is then text of the note, and the block's own closing fence
    opens an ordinary code block (rule 13), which can hide the srl blocks
    after it. This is not always an error, since an item cut this way may
    still be valid, but `srl lint` reports every block closed by an
    indented fence (see [cli.md](cli.md), "Checking"). `srl add` picks a
    long enough fence by itself.
13. **Other fenced code blocks of the note are ignored**, so a note can
    show the item syntax in a code example. A fenced code block starts
    with a line of three or more backticks or tildes (after up to three
    spaces), and ends with a line of at least as many of the same
    character. An indented ```` ```srl ```` starts one of them, not an srl
    block.
14. **The YAML front-matter of a note**, if it starts with `---` on the
    first line, is skipped up to the next `---` line. `srl` never reads it.

A field's text is its first line (after `KEY: `), then each continuation
line on a new line, without trailing blank lines. A field written as `KEY:`
alone starts on the next line:

`````markdown
````srl id:b19x0q
Q: Does this compile?
  ```rust
  let a = String::new();
  let b = a;
  println!("{a}");
  ```

A:
  No. `a` was moved into `b`, so it can't be used.

  Types that implement `Copy` are copied instead.
````
`````

Here `Q` is `Does this compile?` and the code block, without its two
leading spaces; `A` is the two paragraphs. The blank line between the code
and `A:` is allowed by rule 9, and so is the blank line inside `A`. The
block is opened with four backticks so that the code block inside `Q`
doesn't close it (rule 12).

The header entries of the srl block syntax (see [items.md](items.md),
"Header entries"):

- An entry is `key:value`, or the flag `draft`: `id:x7k2m9`, `kind:qa`,
  `see:ADDRESS`, `draft`. A value runs to the next space, or is written in
  double quotes when it contains spaces:
  `see:"japanese/grammar/wo mono.md"`. A value can't be empty, and there
  are no escapes inside quotes.
- Each entry may appear at most once. Any other entry is an error, so a
  typo such as `darft` fails instead of being ignored.
- The order of entries doesn't matter.

A hand-written item needs no entries: a line ```` ```srl ````, the
fields, and a line ```` ``` ````. `srl id` adds the ID.

## The JSONL syntax

A JSONL note holds one item per line, and nothing else:

1. **Every line is an item object** (see [items.md](items.md), "The item
   object"): one JSON
   object of the item's header entries and its fields, such as
   `{"id": "x7k2m9", "Q": "…", "A": "…"}`. The object takes the whole
   line; JSON can't break it over several, since a line ending inside a
   text is written `\n`.
2. **Blank lines are allowed**, and ignored.
3. **Any other line is an error**: text that is not JSON, JSON that is not
   an object, or an object that is not a valid item object.

An item's line, in `ADDRESS:LINE`, is the line it is on.

Field text is anything JSON can write: line endings, code blocks, fences
of any length, quotes and backslashes need nothing but JSON's escapes.

A JSONL note is data, not prose: it is never a context of its own items
(see "Context"), and `srl` writes each item it changes as a new line,
without keeping its spacing or the order of its keys (see "How srl writes
to notes"). Format the file with any JSON tool, such as `jq -c`, if you
want; `srl` reads it the same.

## An item's line

An item's line, in `ADDRESS:LINE` and as `line` in JSON (see
[items.md](items.md), "Identity"), is the line of its header, counting
from 1: the opening fence of a block that holds one item, or the item's
`@` line; in a JSONL note, the line it is on.

An item keeps its identity when it moves within its note, to another note,
into another block or out of one, or between the two syntaxes, and when
its note is renamed: its ID is its identity, never its line or its note.

## Context

What a note shows as context (see [items.md](items.md), "Context"), in
each mode:

| Mode | A note of srl blocks | A JSONL note |
|---|---|---|
| `prose` | The note without its items: without its srl blocks. | Nothing, since it is all items. |
| `full` | The note with its items: the field lines of srl blocks, as written, and the blank lines between them, without the fences of the blocks and their `@` lines. | The note as it is. |
| `none` | Nothing. | Nothing. |

For a note of srl blocks, in every mode, the YAML front-matter is removed,
runs of blank lines become one, and blank lines at the start and the end
are removed. A block with an error is removed in every mode.

**A JSONL note is never the context of its own items**: an item of a
JSONL note without `see` has no context, in any mode. So every item of a
JSONL note that asks about a note needs `see`. **In mode `full`, a JSONL
note named by `see` is given as it is**: every line, with nothing
removed, as the file holds it, line endings included. Only a UTF-8 byte
order mark is left out, as it is from every note.

## How srl writes to notes

The commands that change items (see [items.md](items.md), "How srl
changes items") change notes by these rules:

| Command | Changes in a note |
|---|---|
| `srl id` | Adds `id:ID` to the header of each item without one. |
| `srl accept SELECTOR` | Removes `draft` from the headers of the matching items. |
| `srl add ADDRESS` | Appends the new items to the end of the note at `ADDRESS`, or creates it. |
| `srl edit --stdin` | Replaces the lines of the items it is given with new ones. |

1. **Only the lines of the items changed are written; every other byte of
   the file stays the same**, including line endings and a byte order
   mark. How an item's lines change depends on the syntax:
   - **srl blocks: a change of `id` or `draft` alone changes header lines
     only**, whether `srl id`, `srl accept` or `srl edit` makes it.
     `id:ID` is added right after the `srl` of an opening fence or the `@`
     of an `@` line, with one space before it. `draft` is removed together
     with the spaces before it. Any other change by `srl edit` writes the
     item as `srl add` would, in a block of its own, or, when it starts
     with an `@` line, as an `@` line and its field lines in the same
     block.
   - **JSONL: the item's line is written again**, as the item object of
     its entries and fields, in that order: only the entries that differ
     from their defaults, without spaces (`{"id":"x7k2m9","Q":"…"}`). Its
     spacing, the order of its keys and its escapes are not kept.

   In either syntax, an item `srl edit` is given unchanged is left as it
   is. How `srl add` writes new items in each syntax is in
   [cli.md](cli.md), "Adding items".
2. **A note that changed while `srl` was working is not written.** Each
   item to change is found in the note as it is right before it is
   written: by its ID, or, for `srl id`, by its place among the note's
   items. If an item is not there any more, or not as `srl` read it, or
   the note has an error now, the command stops and says so. Right before
   writing, the file is read again and compared with what `srl` read. If
   it differs, for example because your editor saved it in between, the
   command stops and says so; notes it wrote before stay written, and
   running it again finishes the work.
3. **Writes are atomic.** The new content is written to a temporary file
   in `.srl/tmp/`, then renamed over the note, so a note is never left half
   written. If the rename fails, the note is left as it was and the error
   is reported; it always fails for a note on another file system than
   `.srl/` (for example in a folder that is a mount point), so keep notes
   on the same file system as their store's `.srl/`. If `srl` is killed
   before the rename, the temporary file is left behind in `.srl/tmp/`,
   which git ignores (see [store.md](store.md), "Version control and
   syncing"); it can be deleted at any time no `srl` command is running.

> **TODO (platform):** not run on Windows yet. Windows refuses to rename
> over a file that another program holds open without sharing it, as some
> editors do; the command then fails for that note and says so.

## Errors

The errors of notes, besides those of every item (see
[items.md](items.md), "Errors"):

- an srl block that is never closed, or a fence of backticks with a
  backtick after `srl` (rule 2);
- an srl block that is empty, starts with neither a field line nor an `@`
  line, or mixes the two (rule 3);
- an `@` line whose `@` is followed by something other than a space;
- a header not followed by a field line (rule 6);
- a line inside an item that starts with one space or a tab (rule 8);
- a line inside a block that belongs to no item (rule 11);
- an unclosed quote in a header entry;
- a line of a JSONL note that is not blank and not a valid item object
  (see [items.md](items.md), "The item object").
