# Formats

The `filesystem` source (see [sources.md](sources.md)) reads collections
that are files of the store, each holding items and nothing else, in a
**format** named by its extension:

| Extension | Format | Made for |
|---|---|---|
| `.jsonl` | **JSONL**: one item object per line. | Programs: frontends, agents, importers and generators append and change lines. |
| `.json` | **JSON**: one array of item objects. | Programs and tools that write JSON files whole. |
| `.xml` | **XML**: an `<items>` element of `<item>` elements, one element per field. | People writing items by hand, with fields of many lines. |

Every format writes the same items: the same header entries, fields, IDs
and status (see [items.md](items.md)). Everything after reading the items
(kinds, cards, scheduling, the log, selectors) is the same for every
format and every source. A collection holds no context: the notes items
ask about are files of their own, which items name with `see` (see
[items.md](items.md), "Context").

```json
{"id": "x7k2m9", "see": "japanese/grammar/wo-mono-tomo-sezu.md", "Q": "What does 〜をものともせず mean?", "A": "Undeterred by (adversity), braving."}
{"id": "p4n8d2", "see": "japanese/grammar/wo-mono-tomo-sezu.md", "Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。"}
```

The same items in XML:

```xml
<items>
  <item id="x7k2m9" see="japanese/grammar/wo-mono-tomo-sezu.md">
    <Q>What does 〜をものともせず mean?</Q>
    <A>Undeterred by (adversity), braving.</A>
  </item>
  <item id="p4n8d2" see="japanese/grammar/wo-mono-tomo-sezu.md">
    <Q>嵐を{{c1::ものともせず}}、漁師たちは海へ出た。</Q>
  </item>
</items>
```

## Which files are collections

**A file is a collection of a `filesystem` source when a glob of the
source's `include` matches its path, no glob of its `exclude` does, and
it is not in a folder whose name starts with `.`** (`.srl/`, `.git/`,
`.obsidian/`, …). Each source sets both in `.srl/config.toml` (see
[sources.md](sources.md), "Configuration"). Without `[sources]`, a store
has one source, `items`, whose `include` is `["**/*.jsonl", "**/*.xml"]`:
JSON files are left out by default, since a store often holds JSON files
meant for other programs. `include = ["**/*.jsonl", "**/*.xml",
"items/**/*.json"]` reads JSON collections under `items/` too.

**Every file a source's `include` matches must be a valid collection**,
in a format `srl` knows by its extension; a glob that matches files meant
for other programs makes them errors. A file belongs to one source at
most (see [sources.md](sources.md), "Addresses").

A collection's **address** is its path, relative to the store's folder,
with `/` between folders. JSON outputs give the address of the
collection an item is in (`address`). `srl collections` lists every
collection, including those that hold no items (see [cli.md](cli.md)).

Files are UTF-8. Both `\n` and `\r\n` line endings are read, and a UTF-8
byte order mark is ignored.

## JSONL

A JSONL collection holds one item per line, and nothing else:

1. **Every line is an item object** (see [items.md](items.md), "The item
   object"): one JSON object of the item's header entries and its fields,
   such as `{"id": "x7k2m9", "Q": "…", "A": "…"}`. The object takes the
   whole line; JSON can't break it over several, since a line ending
   inside a text is written `\n`.
2. **Blank lines are allowed**, and ignored.
3. **Any other line is an error**, reported with its line: text that is
   not JSON, JSON that is not an object, or an object that is not a valid
   item object.

Field text is anything JSON can write: line endings, code blocks, quotes
and backslashes need nothing but JSON's escapes. Format the file with any
JSON tool, such as `jq -c`, if you want; `srl` reads it the same.

## JSON

A JSON collection holds one JSON array, whose every element is an item
object, laid out as you like:

```json
[
  {"id": "x7k2m9", "Q": "What does 〜をものともせず mean?", "A": "Undeterred by (adversity), braving."},
  {
    "id": "p4n8d2",
    "Q": "嵐を{{c1::ものともせず}}、漁師たちは海へ出た。"
  }
]
```

- **A file that is not a JSON array is an error** of the whole
  collection, as is an empty file: an empty collection is `[]`.
- **An element that is not a valid item object is an error** of that
  element, by its position from 1; the other elements are read as
  always.

## XML

An XML collection is made for writing items by hand: a field of many
lines is written as it reads, without escapes for line endings.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<items>
  <!-- Ownership in Rust. -->
  <item id="b19x0q" see="rust/ownership.md">
    <Q>
      Does this compile?

          let a = String::new();
          let b = a;
          println!("{a}");
    </Q>
    <A>
      No. `a` was moved into `b`, so it can't be used.

      Types that implement `Copy` are copied instead.
    </A>
  </item>
  <item kind="cloze" draft="true">
    <Q><![CDATA[Is `a < b` true? {{c1::yes}}]]></Q>
  </item>
</items>
```

1. **The root is one `<items>` element**, without attributes. An XML
   declaration and comments may come before and after it; nothing else
   may.
2. **`<items>` holds `<item>` elements**, and comments and blank space
   between them; nothing else.
3. **An `<item>`'s attributes are its header entries** (see
   [items.md](items.md), "Header entries"): `id`, `kind`, `see` and
   `draft`, which is `true` or `false`. Their values are read as an item
   object's: `kind="qa"` and `draft="false"` are the same as leaving them
   out. Any other attribute is an error.
4. **An `<item>` holds its fields**, at least one: each an element named
   by the field's key, such as `<Q>` or `<VERB_1>`, without attributes or
   elements of its own, and comments and blank space between them;
   nothing else. A field written twice is an error, as in every item.
5. **A field's text is its content**, with XML's escapes (`&lt;`, `&amp;`,
   `&#10;`, …) read, by one of two rules:
   - **Content that is CDATA sections only**, with nothing around them
     but blank space, is their text exactly as written:
     `<Q><![CDATA[a < b]]></Q>` is `a < b`.
   - **Any other content is laid out**: if it spans several lines, a
     first line and a last line that hold nothing but blank space are
     removed, and then the indentation all of its other lines have in
     common, spaces and tabs, is removed from each, so that a field reads
     as indented under its element. Lines that hold nothing but blank
     space don't count for the common indentation, and are left empty.
     Text on one line is taken as it is. `<A></A>` and `<A/>` are empty
     fields.

   So the `Q` above is `Does this compile?`, a blank line, and the code,
   still indented by four spaces, since the common indentation is that of
   `Does this compile?`.
6. **An error is reported with the line it is on**: XML that isn't
   well-formed, an element or attribute these rules don't allow, or an
   item that is not valid.

## How srl writes to collections

The commands that change items (see [items.md](items.md), "How srl
changes items") change collections by these rules:

| Command | Changes in a collection |
|---|---|
| `srl id` | Adds `id` to each item without one. |
| `srl accept SELECTOR` | Removes `draft` from the matching items. |
| `srl add ADDRESS` | Appends the new items to the collection at `ADDRESS`, or creates it. |
| `srl edit --stdin` | Replaces the items it is given with new ones. |

1. **Each format keeps the file as close to how it was as it can**; what
   it can't keep is said below. An item `srl edit` is given unchanged is
   left as it is, in every format.
   - **JSONL: only the lines of the items changed are written; every
     other byte of the file stays the same**, including line endings and
     a byte order mark. A changed item's line is written again as the
     item object of its entries and fields, in that order: only the
     entries that differ from their defaults, without spaces
     (`{"id":"x7k2m9","Q":"…"}`); its spacing, the order of its keys and
     its escapes are not kept. New items are appended, one line each.
   - **XML: only the `<item>` elements of the items changed are written;
     every other byte of the file stays the same**, including comments,
     line endings and a byte order mark. A changed item's element is
     written again in place, and new items are added before `</items>`,
     indented as the file's first item is, or by two spaces. `srl` writes
     an element with its entries as attributes, in the order `id`,
     `kind`, `see`, `draft`, and each field as an element of its own, on
     one line when its text is one line, and laid out over several,
     indented as its element is, when it has several: those that rule 5
     reads back as written. A text that holds `<` or `&`, or that the
     layout would change, such as one whose every line is indented, is
     written as CDATA. A text that holds a carriage return can't be
     written to XML, which reads it as a line ending; such a change is
     refused, and the item can be kept in a JSONL collection instead.
   - **JSON: the whole file is written again**: an array with one item
     object per element, two spaces of indentation, in the line ending of
     the file and with its byte order mark. An item not changed is
     written with the keys and values it was read with, in their order;
     a changed or new one as the item object of its entries and fields,
     as in JSONL. The file's layout is not kept.
2. **A collection that changed while `srl` was working is not written.**
   Each item to change is found in the collection as it is right before
   it is written: by its ID, or, for `srl id`, by its place among the
   collection's items. If an item is not there any more, or not as `srl`
   read it, or the collection has an error now, the command stops and
   says so. Right before writing, the file is read again and compared
   with what `srl` read. If it differs, for example because your editor
   saved it in between, the command stops and says so; files it wrote
   before stay written, and running it again finishes the work.
3. **A changed file is read again before it is written**, and must give
   exactly the items intended, or nothing is written: a mismatch is a bug
   in `srl`, reported as one.
4. **Writes are atomic.** The new content is written to a temporary file
   in `.srl/tmp/`, then renamed over the collection, so a file is never
   left half written. If the rename fails, the file is left as it was and
   the error is reported; it always fails for a file on another file
   system than `.srl/` (for example in a folder that is a mount point),
   so keep collections on the same file system as their store's `.srl/`.
   If `srl` is killed before the rename, the temporary file is left behind
   in `.srl/tmp/`, which git ignores (see [store.md](store.md), "Version
   control and syncing"); it can be deleted at any time no `srl` command
   is running.

A new collection starts empty, as each format writes one: an empty file
for JSONL, `[]` for JSON, and `<items>` and `</items>` on two lines for
XML.

> **TODO (platform):** not run on Windows yet. Windows refuses to rename
> over a file that another program holds open without sharing it, as some
> editors do; the command then fails for that file and says so.

## Errors

The errors of collection files, besides those of every item (see
[items.md](items.md), "Errors"), each at its collection:

- a file whose extension names no format `srl` knows;
- a line of a JSONL file that is not blank and not a valid item object,
  with its line;
- a JSON file that is not an array, and an element of it that is not a
  valid item object, with its position;
- an XML file that is not well-formed, or breaks a rule of "XML", with
  its line.
