# Selectors

A **selector** picks a set of cards. Every command that works on several
cards takes one, and it means the same thing everywhere: for a person
typing or a program.

```sh
srl next in:grammar,kanji
srl find in:rust -is:new
srl accept is:draft in:generated
```

## Syntax

A selector is a list of **terms** separated by spaces. Command-line arguments
are joined with spaces, so `srl find in:a in:b` and `srl find "in:a in:b"`
mean the same. Every argument that is not one of the command's options is a
term, even when it starts with `-` (`-in:n5`), except one that starts with
`--`: that is an option the command doesn't have, such as the typo
`--dryrun`, and an error. Every argument after a `--` is a term, even when
it looks like an option. A program that passes a selector it didn't write
puts it after `--`.

| Term | Matches |
|---|---|
| `in:NAME` | Cards whose item's collection has `NAME` among its **`in` names**, which its source gives (see [sources.md](sources.md), "Collections"). A collection of a `filesystem` source has each of its folders, its file name, and its file name without its extension (see [formats.md](formats.md), "Which files are collections"): `in:grammar` matches `japanese/Grammar/wo.jsonl`, `grammar.jsonl` and `grammar.xml`, but not `grammar-notes.jsonl`, and `in:grammar.xml` only the last of those. A source program gives the names it chooses, or, by default, the collection's name alone: its table `v1.2` matches `in:v1.2`, not `in:v1`. Names are compared ignoring case, in Unicode NFC. Only collections are matched: a note named `grammar.md` that items name with `see` doesn't make them match. |
| `path:GLOB` | Cards whose item's collection has a name the glob matches, whatever its source. `*` matches within one part between `/`, `**` across parts: `path:japanese/*` is the collections directly in `japanese/`, `path:japanese/**` everything below it. Collections' names and globs are compared in Unicode NFC (see [store.md](store.md), "File names"). |
| `source:NAME` | Cards whose item comes from the source `NAME` of `.srl/config.toml` (see [sources.md](sources.md)): `source:items` is the cards of a store without `[sources]`, whose one source is `items`. |
| `id:ID` | The card with that ID, or every card of the item with that ID: `id:k3m9q1` matches `k3m9q1.1` and `k3m9q1.2`. |
| `kind:K` | Cards whose item is of kind `K`: `qa`, which includes items with no `kind` entry, or a kind named in `[kinds]` of `.srl/config.toml` (see [cards.md](cards.md), "Kinds"). A selector without `kind:` matches items of every kind. |
| `is:S` | Cards with status `S`, one of `active`, `draft` and `suspended` (see [items.md](items.md), "Status"); cards at `S` in their schedule, one of `new`, `due`, `young` and `mature` (see [scheduling.md](scheduling.md), "Schedule"); or, for `is:leech`, the leeches (see [scheduling.md](scheduling.md), "Leeches"). See "Status, schedule and leeches" below. These come from replaying the log, so a selector with `is:` needs the configuration and the local time zone. |
| `word` or `"some words"` | Cards whose item contains the text, case-insensitively, in any field |

`in:` is a quick way to name a topic, taken from how you already arrange
your collections, such as their folders and names: there are no tags
to write. `srl collections --json` gives each collection's `in` names
(see [cli.md](cli.md)). `path:` is for a collection's whole name.

Keys and text are told apart by one rule: a term that starts with lowercase
letters `a`–`z` and a colon is a `key:value` term, and anything else is text.
A key that is not in the table above is an error, so a typo such as `ni:x`
fails instead of silently searching for the text `ni:x`. To search for text
like that, put it in double quotes: `"std::move"`.

Double quotes make a term of everything up to the next `"`, spaces included,
and it is always text. `-` before the opening quote negates it. So text
that starts with `-`, such as `--verbose`, is searched for in quotes
(`"--verbose"`): unquoted, its first `-` would negate it. Quotes must be
closed, and followed by a space or the end. Your shell removes one level of
quotes, so from the command line write `srl find '"some words"'`. Without the
inner quotes, `srl find "some words"` is two terms, `some` and `words`. That
example is for POSIX shells (sh, bash, zsh) and PowerShell 7.3 or later.
In Windows PowerShell 5.1, write `srl find '\"some words\"'`; in `cmd.exe`,
write `srl find "\"some words\""`.

A value can't be empty (`in:` or `in:a,,b`), and `source:`, `id:`, `kind:`
and `is:` values must be sources of the store, valid IDs, kinds, and the
values listed for `is:`. So a typo such as `kind:ja-word` fails instead of
matching nothing.

Three combining rules, and nothing else:

| Syntax | Meaning |
|---|---|
| `key:a,b` | Comma: **or**. `in:a,b` is cards in `a` or in `b`. |
| `term term` | Space: **and**. Both must match. |
| `-term` | Minus: **not**. `-in:a` excludes cards in `a`. |

There is no grouping and no operator precedence. If you need something these
rules can't express, run two commands.

An empty selector matches every card.

## Status, schedule and leeches

A card has three things that `is:` selects, each separate from the
others, and given apart in the JSON of `srl find` (see [cli.md](cli.md)):

| JSON field | Says | Values |
|---|---|---|
| `status` | whether the card is studied | `active`, `draft`, `suspended` |
| `schedule` | where the card is in its schedule | `new`, `due`, `young`, `mature` |
| `leech` | whether the card keeps being forgotten | `true`, `false` |

One key selects all three, since no value of one is a value of another:
`is:draft` is the drafts, `is:due` the due cards and `is:leech` the
leeches. `is:draft,due` is the drafts and the due cards, and `is:new
is:active` the new cards that are studied. See [rationale.md](rationale.md),
"Why one `is:` for status, schedule and leeches".

## Defaults per command

One command adds a term of its own:

| Command | Adds |
|---|---|
| `srl next` | `is:active` (drafts and suspended cards are never studied) |

Every other command applies the selector exactly as written.

## Examples

```sh
srl next in:grammar,kanji                 # grammar or kanji collections
srl find in:japanese -in:n5               # all Japanese except the n5 folder
srl find is:leech --json                  # every leech
srl find "ものともせず"                    # text search
srl reset path:rust/ownership.xml         # start a collection over
srl find in:vocab is:mature               # what you know well
srl next kind:ja-words                    # study one kind of item
srl find -kind:qa                         # every item of a kind program
```

Commands that print cards follow `grep` exit codes: 0 if anything matched, 1
if nothing did, 2 on error.
