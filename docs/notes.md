# Notes and items

A **note** is a file in the store that `srl` reads for items: by default,
any Markdown file (see "Which files are notes"). An **item** is a question
written inside a note. Everything else in a note is yours: `srl` reads it
only to show it as context, and never changes it.

Items are written in one of two **item syntaxes**, chosen by the note's
file name (see "Item syntaxes"):

- **srl blocks**, in every note but JSONL notes: fenced code blocks whose
  language is `srl`, inside your Markdown. A block holds one item, with the
  item's entries on its opening fence, or several, each starting with an
  `@` line that holds its entries.
- **JSONL**, in notes whose name ends with `.jsonl`: one item per line, as
  a JSON object. These notes are data, written by programs such as
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

**A file is a note when a glob in `include` matches its path, no glob in
`exclude` does, and it is not in a folder whose name starts with `.`**
(`.srl/`, `.git/`, `.obsidian/`, …). Both are set in `.srl/config.toml`
(see [store.md](store.md), "Configuration"). By default `include` is
`["**/*.md"]` and `exclude` is empty: every Markdown file is a note.
`srl notes` lists them (see [cli.md](cli.md)), including the notes that
hold no items.

So JSONL notes are read only when `include` matches them, for example with
`include = ["**/*.md", "**/*.jsonl"]`, or `"items/**/*.jsonl"` to keep them
in one folder. Every file that `include` matches must be a valid note, so a
glob that matches JSON files meant for other programs makes them errors.

A note's **path** is relative to the store's folder, with `/` between
folders. Files are UTF-8. Both `\n` and `\r\n` line endings are read, and a
UTF-8 byte order mark is ignored.

## Item syntaxes

**A note whose file name ends with `.jsonl` is in the JSONL syntax; every
other note is in the srl block syntax.** The extension is matched exactly,
in lowercase. Nothing else decides it, not even what the file holds.

Every note but a JSONL note is read the same way, whatever its name: a
`.txt` note has the same srl blocks, fenced code blocks and context as a
`.md` note, and its fields are formatted as in [cards.md](cards.md), "Text
formatting". The srl block syntax is Markdown's, so it suits Markdown and
the formats built on it, such as MDX and Quarto; which other files make
sense as notes is up to you.

Both syntaxes write the same items: the same header entries (see "Header
entries"), fields, IDs and status. Everything after reading the items
(kinds, cards, scheduling, the log, selectors) is the same for both. They
differ in what else a note holds, and so in its context (see "Context"),
and in how `srl` writes to it (see "How srl writes to notes").

Choosing the syntax by the extension is a convention, picked because it is
one rule that needs no setting: a Markdown note full of JSON lines, or a
`.jsonl` file of srl blocks, is rare (see [rationale.md](rationale.md),
"Why JSONL notes, and why the extension decides").

## The srl block syntax

An item is a **header** followed by **field** lines, inside an srl block.
The rules, in full:

1. **An srl block opens** with a line that starts, at column 0, with three
   or more backticks or tildes, then any number of spaces, `srl`, then a
   space or the end of the line. What follows `srl` is the fence's
   **entries** (see "Header entries"). ```` ```srl ````, ```` ``` srl
   id:x7k2m9 ```` and `~~~~srl` all open blocks.
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

The header entries of the srl block syntax (see "Header entries"):

- An entry is `key:value`, or the flag `draft`: `id:x7k2m9`, `kind:qa`,
  `see:PATH`, `draft`. A value runs to the next space, or is written in
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

1. **Every line is an item object** (see "The item object"): one JSON
   object of the item's header entries and its fields, such as
   `{"id": "x7k2m9", "Q": "…", "A": "…"}`. The object takes the whole
   line; JSON can't break it over several, since a line ending inside a
   text is written `\n`.
2. **Blank lines are allowed**, and ignored.
3. **Any other line is an error**: text that is not JSON, JSON that is not
   an object, or an object that is not a valid item object.

An item's line, in `PATH:LINE`, is the line it is on. Field text is anything JSON can write: line endings, code blocks, fences
of any length, quotes and backslashes need nothing but JSON's escapes.

A JSONL note is data, not prose: it is never a context of its own items
(see "Context"), and `srl` writes each item it changes as a new line,
without keeping its spacing or the order of its keys (see "How srl writes
to notes"). Format the file with any JSON tool, such as `jq -c`, if you
want; `srl` reads it the same.

## Header entries

| Entry | Meaning | Default |
|---|---|---|
| `id` | The item's ID (see "Identity"). | none: an item without an ID is an error until `srl id` gives it one |
| `kind` | How the fields become cards (see [cards.md](cards.md), "Kinds"). | `qa` |
| `see` | The note shown as context instead of the one the item is in (see "Context"). | the item's own note |
| `draft` | The item is a draft (see "Status"). | not a draft |

How they are written depends on the syntax: `id:x7k2m9` on a fence or an
`@` line (see "The srl block syntax"), `"id": "x7k2m9"` in an item object
(see "The item object"). In both, each entry may be given once, and a
misspelled one is an error.

## Identity

- **An item ID** is made of words of lowercase letters `a`–`z` and
  digits, joined by single `-`, 1 to 32 characters: `x7k2m9`, or
  `jp-grammar-desu` when you name it yourself. `-` is never first, last
  or doubled, and `.` is never part of an ID: it separates a card's
  suffix. `srl id` and `srl add` make random ones of 6 characters
  (`SRL_SEED` makes them repeatable; see [cli.md](cli.md),
  "Conventions"); you may also give one yourself, in a note or to `srl
  add`.
- **IDs are unique in the store.** Two items with the same ID are an error,
  which happens when an item is copied: delete the ID from the copy and run
  `srl id`.
- **The ID is the identity.** Edit the item, move it within the note or to
  another note, into another block or out of one, between the two
  syntaxes, rename the note: the item keeps its cards and their history.
- An item gives one or more cards, and each card has its own ID, made from
  the item's: `x7k2m9`, or `p4n8d2.1` and `p4n8d2.2` (see
  [cards.md](cards.md), "Card IDs"). Because an item ID never contains `.`,
  a card ID always names its item.
- **An item's line is for people only.** `PATH:LINE` in listings and
  messages, and `line` in JSON, say where the item starts, as near as its
  syntax can say: the line of its header, or its line in a JSONL note.
  `srl` never finds an item by its line: commands that change an item
  find it by its ID in the note as it is (see "How srl writes to notes").
- **Editing an item never resets its cards.** `srl reset` is the way to
  start over. When an edit makes a card disappear, its history is kept in
  the log, and `srl move` can give it to another card (see
  [scheduling.md](scheduling.md), "Moving history").

## Status

Every card has one status, usable in selectors as `status:NAME`:

| Status | Studied? | Comes from |
|---|---|---|
| `draft` | no | The item has `draft`. |
| `suspended` | no | The card was suspended with `srl suspend`, and not unsuspended since (see [log.md](log.md)). |
| `active` | yes | Neither. |

A card that is both a draft and suspended is `draft`.

Drafts exist so that items written by a program, such as a generator or
an import, never enter your study queue without your approval. `srl accept SELECTOR` removes `draft`
from the matching items. Suspending is about studying, not
about the note, so it is recorded in the log and never changes the note.

## Context

An item's **context** is the note named by `see` if the item has one, and
otherwise the note the item is in, **unless that is a JSONL note: an item
of a JSONL note without `see` has no context.** Frontends show it after the
answer of each of the item's cards, taking it from `srl next --json`; `srl
show` prints it too.

- `see` is a path relative to the store's folder, to a note of the store,
  in either syntax. A `see` that names anything else is an error.
- `see` is meant for items that ask about another note, such as items a
  script imported or generated into a file of their own. Every item of a
  JSONL note that asks about a note needs it.

The context is given in one of three **modes**. How much of the note to
show is a choice of whoever shows it, so the mode is not a setting of the
store: each frontend chooses it with `--context MODE` on `srl next`, and
`srl show` takes the same flag. Without it, the mode is `prose`.

| Mode | Shows |
|---|---|
| `prose` | The note without its srl blocks. |
| `full` | The note with the field lines of its items, as written, and the blank lines between them, without the fences of the srl blocks and their `@` lines. |
| `none` | Nothing. |

For a note of srl blocks, in every mode, the YAML front-matter is removed,
runs of blank lines become one, and blank lines at the start and the end
are removed. A block with an error is removed in every mode.

**A JSONL note named by `see` is given as it is**, in modes `prose` and
`full` alike: every line, with nothing removed, as the file holds it,
line endings included. Only a UTF-8 byte order mark is left out, as it is
from every note.

**A context with nothing but blank space left is no context**, exactly as
in mode `none`: frontends show nothing, and `context` is `null` in JSON. So
a note that holds only items has no context in `prose` mode.

**A context has a format**, given with it as `context_format` in JSON,
so that whoever shows it need not guess how to read it. The note it comes
from decides it, by its syntax: `markdown` for a note of srl blocks,
whatever its extension, and `jsonl` for a JSONL note. It is `null` when
`context` is `null`. More formats may be added later, so whoever reads
`context_format` must accept values it doesn't know (see
[frontends.md](frontends.md), "Rendering").

> **TODO (design):** the two syntaxes give their notes as context by
> different rules: srl blocks are removed from a note in `prose` mode, but
> a JSONL note named by `see` is given whole, its items included, and is
> never the context of its own items. Each rule is simple, but together
> they are not one rule; a single rule for both is wanted.

## How srl writes to notes

Only four commands ever change a note:

| Command | Changes |
|---|---|
| `srl id` | Gives an ID to the items that have none. |
| `srl accept SELECTOR` | Removes `draft` from the matching items. |
| `srl add NOTE` | Appends new items to the end of `NOTE`, or creates it. |
| `srl edit --stdin` | Replaces the lines of the items it is given with new ones, for scripts and other programs. |

Every other command only reads notes. The rules for writing:

1. **Only the lines of the items changed are written; every other byte of
   the file stays the same**, including line endings and a byte order
   mark. How an item's lines change depends on the syntax:
   - **srl blocks: `srl id` and `srl accept` change header lines only.**
     `id:ID` is added right after the `srl` of an opening fence or the `@`
     of an `@` line, with one space before it. `draft` is removed together
     with the spaces before it. `srl edit` writes the item as `srl add`
     would, in a block of its own, or, when it starts with an `@` line,
     as an `@` line and its field lines in the same block.
   - **JSONL: the item's line is written again**, as the item object of
     its entries and fields, in that order: only the entries that differ
     from their defaults, without spaces (`{"id":"x7k2m9","Q":"…"}`). Its
     spacing, the order of its keys and its escapes are not kept.
2. **Every note is checked before the first one is written.** If any note
   has an error, the command reports every error and changes nothing. For
   `srl id`, an item without an ID is not an error, since that is what it
   fixes.
3. **A note that changed while `srl` was working is not written.** Each
   item to change is found in the note as it is when the command writes
   it: by its ID, or, for `srl id`, by having none. If an item is not there
   any more, or the note has an error now, the command stops and says so.
   Right before writing, the file is read again and compared with what
   `srl` read. If it differs, for example because your editor saved it in
   between, the command stops and says so; notes it wrote before stay
   written, and running it again finishes the work.
4. **Writes are atomic.** The new content is written to a temporary file
   in `.srl/tmp/`, then renamed over the note, so a note is never left half
   written. If the rename fails, the note is left as it was and the error
   is reported; it always fails for a note on another file system than
   `.srl/` (for example in a folder that is a mount point), so keep notes
   on the same file system as their store's `.srl/`. If `srl` is killed
   before the rename, the temporary file is left behind in `.srl/tmp/`,
   which git ignores (see [store.md](store.md), "Version control and
   syncing"); it can be deleted at any time no `srl` command is running.
5. **Every change is reported.** `srl id` prints one line per item it gave
   an ID: the ID and `PATH:LINE`, separated by two spaces. `srl accept`,
   `srl add` and `srl edit` print the IDs of the items they changed or
   added (see [cli.md](cli.md)). With `--dry-run` they write nothing, and
   print the same (`srl add`: the items it would write).

`srl add`, `srl edit` and their formats are described in [cli.md](cli.md).

> **TODO (platform):** not run on Windows yet. Windows refuses to rename
> over a file that another program holds open without sharing it, as some
> editors do; the command then fails for that note and says so.

## Errors

Every command that reads items reads every note, and **any error in any
note stops it**: it reports every error it found, as `PATH:LINE: message`
on standard error, one per line, in the order of the notes and their
lines, and exits with status 2. The errors are:

- an srl block that is never closed, or a fence of backticks with a
  backtick after `srl` (rule 2);
- an srl block that is empty, starts with neither a field line nor an `@`
  line, or mixes the two (rule 3);
- an `@` line whose `@` is followed by something other than a space;
- a header not followed by a field line (rule 6);
- a line inside an item that starts with one space or a tab (rule 8);
- a line inside a block that belongs to no item (rule 11);
- a line of a JSONL note that is not blank and not a valid item object
  (see "The item object");
- an unknown, repeated or empty header entry, or an unclosed quote;
- an item without an ID (`srl id` fixes it), an invalid ID, or an ID that
  another item already has;
- a field key written twice in one item, whatever its kind;
- an unknown kind, or fields that the kind doesn't accept (see
  [cards.md](cards.md));
- a kind program that fails as a whole, reported once, without a line
  (see [cards.md](cards.md), "How srl calls them");
- a `see` that doesn't name a note of the store.

`srl id` is the exception for items without an ID: that is the error it
fixes. `srl check` reports the same errors and more (see [cli.md](cli.md)).

> **TODO (design):** stopping at every error can get in the way. Ways to
> make it less annoying are wanted, among them turning some errors into
> warnings and skipping the broken items; none is decided (see
> [overview.md](overview.md), "Open questions").

## The item object

The **item object** is an item as one JSON object: its header entries and
its fields side by side. It is the one JSON form of an item everywhere:

- a line of a JSONL note (see "The JSONL syntax");
- a line of `srl add --stdin` and of `srl edit --stdin` (see
  [cli.md](cli.md));
- what `srl show --json` prints, with every entry and the keys only `srl`
  gives.

So what `srl show --json` prints can be changed and given back to `srl
edit --stdin` as it is.

```json
{"id": "p4n8d2", "kind": "qa", "see": null, "draft": false,
 "Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。",
 "path": "japanese/grammar/wo-mono-tomo-sezu.md", "line": 11,
 "cards": ["p4n8d2.1"],
 "context": "# 〜をものともせず\n\nUndeterred by (adversity), braving. …\n",
 "context_format": "markdown"}
```

Its keys:

| Key | Value |
|---|---|
| `id` | The item's ID, as text. |
| `kind` | The kind's name, as text. `"qa"` is the same as leaving it out. |
| `see` | The path of the note shown as context, as text. `null` is the same as leaving it out. |
| `draft` | `true` or `false`. `false` is the same as leaving it out. |
| a field name: uppercase letters `A`–`Z`, digits and `_`, starting with a letter | The field's text. |
| `path`, `line`, `cards`, `context`, `context_format` | Given by `srl` only (see below), and ignored wherever an item object is read. |

- Header entries are lowercase and field names uppercase, so they never
  clash. Any other key is an error, so a typo such as `drfat` fails
  instead of being ignored. So is a key given twice, which JSON allows.
- The fields are in the order of their keys. An item object has at least
  one field.
- Text values can't be empty, except a field's. Values of the wrong type
  are errors: an ID, a kind or a path written as a number, a draft
  written as `"yes"`.

`srl show --json` prints every entry, defaults included, its fields, then
the keys only `srl` gives:

- `path` is the note the item is in, and `line` the line of its header,
  counting from 1: the opening fence of a block that holds one item, the
  item's `@` line, or its line in a JSONL note.
- `cards` lists the IDs of the item's cards, in order.
- `context` is the context in the mode chosen, or `null` when there is
  none (mode `none`, an item of a JSONL note without `see`, or nothing but
  blank space left).
- `context_format` is the context's format, `markdown` or `jsonl`, or
  `null` when `context` is (see "Context").
