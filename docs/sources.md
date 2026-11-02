# Sources

A **source** is a place items come from and are written back to. `srl`
reads every item of every source on every command that reads items, and
everything after reading them (kinds, cards, scheduling, the log,
selectors) is the same whatever their source. Where items are kept, and
how they are written, is each source's business, and no other part of
`srl` knows it.

A store has the sources its configuration names. Without any, it has one,
`notes`, which reads every Markdown file of the store as a note of srl
blocks: a store of Markdown notes needs no configuration, and the words
of this document only show once it has more than notes.

## Terms

| Term | Meaning |
|---|---|
| **Source** | A named place items are read from and written to, configured as `[sources.NAME]`: `notes`, `data`, `vocab`. |
| **Driver** | What a source is: `markdown`, notes of srl blocks, or `jsonl`, notes of JSON lines (see [notes.md](notes.md)). A source is a driver with its settings; two sources may use the same driver with different settings. |
| **Collection** | A group of items inside a source, named by its source: for both drivers, a note, named by its path. Collections are written like paths, with `/` between parts. |
| **Prefix** | Where a source is mounted in the store's one tree of addresses: `""` by default, or folders ending in `/`, such as `"vocab/"`. |
| **Address** | The name of a collection everywhere outside its source: its source's prefix followed by the collection's name. `japanese/grammar/wo.md` for a note of a source without a prefix. |
| **File** | The file a collection is kept in, relative to the store's folder, when it is a file of its own, as a note is. Only for people and editors. |

## Configuration

```toml
[sources.notes]
driver = "markdown"
include = ["**/*.md"]
exclude = ["templates/**"]

[sources.data]
driver = "jsonl"
include = ["generated/**/*.jsonl"]
```

Each table of `[sources]` is a source, and its key is the source's name:
words of lowercase letters `a`–`z` and digits, joined by single `-`, as an
item ID is. A source's keys:

| Key | Value | Default |
|---|---|---|
| `driver` | `markdown` or `jsonl`. | none: every source has one |
| `include` | Globs of the notes the source reads, relative to the store's folder: `*` matches within one folder, `**` across folders. | `["**/*.md"]` for `markdown`, `["**/*.jsonl"]` for `jsonl` |
| `exclude` | Globs of notes to leave out; they win over `include`. | `[]` |
| `prefix` | The source's place among the store's addresses (see "Addresses"). | `""` |

- **Without `[sources]`, a store has one source**, `notes`, with driver
  `markdown` and the defaults above: every Markdown file is a note.
  `[sources]` replaces it rather than adding to it, so a store with notes
  and JSONL notes names both:
  `[sources.notes]` with `driver = "markdown"`, and `[sources.data]` with
  `driver = "jsonl"`.
- **Which files are notes** is said in [notes.md](notes.md), "Which files
  are notes": those `include` matches and `exclude` doesn't, outside
  folders whose name starts with `.`.
- A key that is not listed is an error, so a typo fails instead of being
  ignored, and so is a glob that isn't valid, a name or prefix that isn't
  made as above, and a source without a `driver`.
- `[notes]`, which named the notes before there were sources, is an
  error, saying how to write it as sources.

## Addresses

**A store is one tree of addresses, and each source is mounted in it at
its prefix**, the way Unix mounts file systems in one tree of folders.
Every collection is reached by an address, whatever its driver, and
`srl collections` lists the tree. The default prefix is `""`, so a store
of notes is mounted at the root and its addresses are the notes' paths.

The rules, the same for every source:

1. **A prefix is `""` or folders ending in `/`**: `"vocab/"`,
   `"db/jp/"`. A prefix that ends inside a name, such as `"voc"`, is an
   error, so a prefix always names whole folders of the tree.
2. **An address belongs to the sources mounted deepest above it**: those
   whose prefix is the longest one the address starts with. **A source
   that gives an address under another source's deeper prefix is an
   error**, naming both: with `notes` at `""` and a source at
   `"vocab/"`, a note `vocab/readme.md` of `notes` is one. Unix would
   quietly hide the folder under the mount point; `srl` says so instead,
   and you leave the note out of `notes` with `exclude`, or mount the
   other source elsewhere.
3. **Sources may share a prefix**, as two sources of notes at `""` do;
   they share that part of the tree, and **no address may be given by two
   sources**. So a file that two sources' globs both match is an error.
4. **No two collections may be the same file.** It catches two sources
   reading one note under different prefixes.
5. **A prefix needs no folder on disk.** A folder with its name may
   exist, and only matters when a source reads notes in it, which rule 2
   reports.
6. **An address never holds a line.** Every item of a collection has the
   same address, and items are told apart by their IDs. Where `srl`
   prints `ADDRESS:LINE` for people, the `:LINE` is added for display;
   `see`, `srl add` and `path:` take addresses alone.

A collection that breaks one of these rules is an error in the store
(see [notes.md](notes.md), "Errors"): it stops every command that reads
items, and `srl check` reports it, as `ADDRESS: message`.

**A new collection**, for `srl add`, is created in the source mounted
deepest above its address that accepts it: for both drivers, a note whose
path, after the prefix, `include` matches and `exclude` doesn't. Exactly
one source must accept it; none or several is an error.

## What a source does

Every source does the same few things, each its own way:

- **It lists its collections**, including those without items: for both
  drivers, its notes.
- **It reads its items**, each with its collection, the line where it
  starts if it can say, and the item as written, and the problems it
  finds (see [notes.md](notes.md), "Errors").
- **It shows a collection as context**, to the collection's own items and
  to the items that name it with `see`, wherever they come from: the
  context of an item is always asked of the source that owns the
  collection shown, found by its address, so each collection is shown by
  its own source's rules (see [notes.md](notes.md), "Context").
- **It makes changes**: it adds items to a collection, creating it if
  needed, and replaces items. It is given each item to replace as it was
  read, and refuses to change an item that is no longer as it was, since
  someone changed it in between. Every change to a source is made and
  checked before anything is written (see [notes.md](notes.md), "How srl
  writes to notes").
- **It looks for likely mistakes**, for `srl lint`.
- **It describes itself**, for `srl sources`: what it is, and which
  collections `srl add` may create in it.

**IDs are unique in the store**, across its sources, and not across
stores: each store has its own log, which knows card IDs only.

`srl sources` describes the store's sources and `srl collections` lists
their collections (see [cli.md](cli.md)).
