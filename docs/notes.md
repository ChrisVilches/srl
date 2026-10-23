# Notes and items

A **note** is any Markdown file in the store. An **item** is a question
written inside a note. Everything else in a note is yours: `srl` reads it
only to show it as context, and never changes it.

```markdown
# 〜をものともせず

Undeterred by (adversity), braving. Admiring tone; not used about oneself.

<!-- srl id:x7k2m9 -->
Q: What does 〜をものともせず mean?
A: Undeterred by (adversity), braving.

<!-- srl id:p4n8d2 -->
Q: 嵐を{{c1::ものともせず}}、漁師たちは海へ出た。
```

Rendered, the header lines disappear (they are HTML comments) and the
items read as plain questions and answers. Unrendered, in an editor, they
read the same, with a short comment above each.

## Which files are notes

Every file whose name ends in `.md`, in the store's folder or any folder
below it, is a note, except:

- files in folders whose name starts with `.` (`.srl/`, `.git/`,
  `.obsidian/`, …);
- files matched by a glob in `exclude` in `.srl/config.toml` (see
  [store.md](store.md), "Configuration").

A note's **path** is relative to the store's folder, with `/` between
folders. Files are UTF-8. Both `\n` and `\r\n` line endings are read, and a
UTF-8 byte order mark is ignored.

## The item syntax

An item is a **header** line followed by **field** lines. The rules, in
full:

1. **A header** is a line that starts, at column 0, with `<!--`, any number
   of spaces, and `srl`, followed by a space or by `-->`. It ends with any
   number of spaces and `-->`, and nothing after it but spaces. Between
   them are **entries**, separated by one or more spaces. `<!--srl-->`,
   `<!-- srl -->` and `<!--  srl id:x7k2m9  -->` are all headers.
2. **A line that starts like a header but isn't one is an error**: one that
   doesn't end with `-->` on the same line, or has anything after it.
   Otherwise a renderer would hide the rest of the note.
3. **Only headers make items.** A line is never part of an item unless a
   header comes before it. `Q:` lines without a header are ordinary text,
   and are not reported (but see `srl lint` in [cli.md](cli.md)).
4. **A field line** starts, at column 0, with a key made of uppercase
   letters `A`–`Z`, digits and `_`, beginning with a letter, then a colon,
   then a space or the end of the line: `Q: text`, `A:`. The field's text
   starts after that space.
5. **The line right after a header must be a field line.** A header
   followed by anything else is an error.
6. **A continuation line** starts with exactly two spaces. It continues the
   field above it, on a new line. The two spaces are removed, and any
   further spaces are kept as part of the text, so indented code keeps its
   indentation.
7. **A line that starts with one space or a tab, inside an item, is an
   error**, because it can only be a mistyped continuation.
8. **Blank lines may appear inside an item**, between fields or inside a
   field, when the next non-blank line is a field line or a continuation
   line. A blank line inside a field is part of its text.
9. **An item ends** at the first non-blank line that is neither a field line
   nor a continuation line (another header, a heading, a paragraph), or at
   the end of the file. Blank lines before that line are not part of the
   item.
10. **Lines inside fenced code blocks of the note are ignored**, so a note
    can show the item syntax in a code example. A fenced code block starts
    with a line of three or more backticks or tildes (after up to three
    spaces), and ends with a line of at least as many of the same
    character. Fences inside an item, in continuation lines, are part of the
    field's text.
11. **The YAML front-matter of a note**, if it starts with `---` on the
    first line, is skipped up to the next `---` line. `srl` never reads it.

A field's text is its first line (after `KEY: `), then each continuation
line on a new line, without trailing blank lines. A field written as `KEY:`
alone starts on the next line:

~~~markdown
<!-- srl id:b19x0q -->
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

Here `Q` is `Does this compile?` and the code block, without its two
leading spaces; `A` is the two paragraphs. The blank line between the code
and `A:` is allowed by rule 8, and so is the blank line inside `A`.

Most renderers join lines that aren't separated by a blank line into one
paragraph, so a `Q:` and an `A:` on consecutive lines may render on one
line. Put a blank line between them if your viewer does this; `srl` reads
both the same.

### Header entries

| Entry | Meaning | Default |
|---|---|---|
| `id:ID` | The item's ID (see "Identity"). | none: an item without an ID is an error until `srl id` gives it one |
| `kind:NAME` | How the fields become cards (see [cards.md](cards.md), "Kinds"). | `qa` |
| `see:PATH` | The note shown as context instead of the one the item is in (see "Context"). | the item's own note |
| `draft` | The item is a draft (see "Status"). | not a draft |

- An entry is `key:value`, or the flag `draft`. A value runs to the next
  space, or is written in double quotes when it contains spaces:
  `see:"japanese/grammar/wo mono.md"`. A value can't be empty, and there
  are no escapes inside quotes.
- Each entry may appear at most once. Any other entry is an error, so a
  typo such as `darft` fails instead of being ignored.
- The order of entries doesn't matter.

A hand-written item needs nothing in its header: `<!-- srl -->`, then the
fields. `srl id` adds the ID.

## Identity

- **An item ID** is made of lowercase letters `a`–`z` and digits, 1 to 32
  characters. `srl id` and `srl add` make random ones of 6 characters; you
  may also type one yourself.
- **IDs are unique in the store.** Two items with the same ID are an error,
  which happens when an item is copied: delete the ID from the copy and run
  `srl id`.
- **The ID is the identity.** Edit the item, move it within the note or to
  another note, rename the note: the item keeps its cards and their
  history.
- An item gives one or more cards, and each card has its own ID, made from
  the item's: `x7k2m9`, or `p4n8d2-1` and `p4n8d2-2` (see
  [cards.md](cards.md), "Card IDs"). Because an item ID never contains `-`,
  a card ID always names its item.
- **Editing an item never resets its cards.** `srl reset` is the way to
  start over. When an edit makes a card disappear, its history is kept in
  the log, and `srl move` can give it to another card (see
  [scheduling.md](scheduling.md), "Moving history").

## Status

Every card has one status, usable in selectors as `status:NAME`:

| Status | Studied? | Comes from |
|---|---|---|
| `draft` | no | The item's header has `draft`. |
| `suspended` | no | The card was suspended with `srl suspend`, and not unsuspended since (see [log.md](log.md)). |
| `active` | yes | Neither. |

A card that is both a draft and suspended is `draft`.

Drafts exist so that items written by agents or imports never enter your
study queue without your approval. `srl accept SELECTOR` removes `draft`
from the headers of the matching items. Suspending is about studying, not
about the note, so it is recorded in the log and never changes the note.

## Context

An item's **context** is the note named by `see:` if the header has one,
and otherwise the note the item is in. Frontends show it after the answer
of each of the item's cards; `srl show` prints it.

- `see:` is a path relative to the store's folder, to a note of the store.
  A `see:` that names anything else is an error.
- `see:` is meant for items that ask about another note, such as items an
  agent wrote in a file of their own (see [agents.md](agents.md)).

The context is shown in one of three **modes**, set by `context` in
`.srl/config.toml` (default `prose`) or by `--context` on `srl show`:

| Mode | Shows |
|---|---|
| `prose` | The note without its items: every item's header, fields and continuation lines are removed. |
| `full` | The note with its items, as written, without their header lines. |
| `none` | Nothing. |

In every mode, the YAML front-matter is removed, runs of blank lines become
one, and blank lines at the start and the end are removed.

**A context with nothing but blank space left is no context**, exactly as
in mode `none`: frontends show nothing, and `context` is `null` in JSON. So
a note that holds only items, studied in `prose` mode, needs no setting to
have no context.

## How srl writes to notes

Only four commands ever change a note:

| Command | Changes |
|---|---|
| `srl id` | Adds `id:ID` to headers that have no ID. |
| `srl accept SELECTOR` | Removes `draft` from the headers of the matching items. |
| `srl add NOTE` | Appends new items to the end of `NOTE`, or creates it. |
| `srl edit ID --stdin` | Replaces the lines of one item with new ones, for scripts and agents. |

Every other command only reads notes (`srl edit ID` opens the note in your
editor, which is you changing it). The rules for writing:

1. **`srl id` and `srl accept` change header lines only.** `id:ID` is added
   right after `srl`, with one space before it. `draft` is removed together
   with the spaces before it. Every other byte of the file stays the same,
   including line endings and a byte order mark.
2. **Every note is checked before the first one is written.** If any note
   has an error, the command reports every error and changes nothing. For
   `srl id`, an item without an ID is not an error, since that is what it
   fixes.
3. **A note that changed while `srl` was working is not written.** Right
   before writing, the file is read again and compared with what `srl`
   read. If it differs, for example because your editor saved it in
   between, the command stops and says so; notes it wrote before stay
   written, and running it again finishes the work.
4. **Writes are atomic.** The new content is written to a temporary file
   next to the note, named `.srl-*.tmp`, then renamed over it, so a note is
   never left half written. If the rename fails, the note is left as it was
   and the error is reported. If `srl` is killed before the rename, the
   temporary file may be left behind; delete it (see
   [store.md](store.md), "Version control and syncing").
5. **Every change is reported.** `srl id` prints one line per item it gave
   an ID: the ID and `PATH:LINE`, separated by two spaces. `srl accept` and
   `srl add` print the IDs of the items they changed or added (see
   [cli.md](cli.md)). With `--dry-run` they write nothing, and print the
   same (`srl add`: the items it would write). `srl edit ID --stdin`
   changes only the item it was given, so it prints nothing; with
   `--dry-run` it only checks its input.

`srl add` and its format are described in [cli.md](cli.md).

> **TODO (platform):** not run on Windows yet. Windows refuses to rename
> over a file that another program holds open without sharing it, as some
> editors do; the command then fails for that note and says so.

## Errors

Every command that reads items reads every note, and **any error in any
note stops it**: it reports every error it found, as `PATH:LINE: message`
on standard error, one per line, and exits with status 2. The errors are:

- a line that starts like a header but isn't one (rule 2);
- a header not followed by a field line (rule 5);
- a line inside an item that starts with one space or a tab (rule 7);
- an unknown, repeated or empty header entry, or an unclosed quote;
- an item without an ID (`srl id` fixes it), an invalid ID, or an ID that
  another item already has;
- an unknown kind, or fields that the kind doesn't accept (see
  [cards.md](cards.md));
- a `see:` that doesn't name a note of the store.

`srl id` is the exception for items without an ID: that is the error it
fixes. `srl check` reports the same errors and more (see [cli.md](cli.md)).

> **TODO (design):** a `strict = false` setting could report these errors
> as warnings and skip the broken items instead of stopping (see
> [overview.md](overview.md), "Open questions").

## Items as JSON

`srl show --json` prints the item of each card or item ID it is given, one
JSON object per line:

```json
{"id": "p4n8d2", "path": "japanese/grammar/wo-mono-tomo-sezu.md", "line": 9,
 "kind": "qa", "see": null, "draft": false,
 "fields": {"Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。"},
 "cards": ["p4n8d2-1"],
 "context": "# 〜をものともせず\n\nUndeterred by (adversity), braving. …\n"}
```

- `line` is the line of the header, counting from 1.
- `see` is the path given by `see:`, or `null`. The context comes from the
  note at `see`, or at `path` when `see` is `null`.
- `fields` holds the fields in the order they are written.
- `cards` lists the IDs of the item's cards, in order.
- `context` is the context in the mode chosen, or `null` when there is
  none (mode `none`, or nothing but blank space left).
