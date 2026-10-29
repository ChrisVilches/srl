# Selectors

A **selector** picks a set of cards. Every command that works on several
cards takes one, and it means the same thing everywhere: for a person
typing, a script, or an agent.

```sh
srl next in:grammar,kanji
srl find in:rust -state:new
srl accept status:draft in:generated
```

## Syntax

A selector is a list of **terms** separated by spaces. Command-line arguments
are joined with spaces, so `srl find in:a in:b` and `srl find "in:a in:b"`
mean the same. Every argument that is not one of the command's options is a
term, even when it starts with `-` (`-in:n5`), and every argument after a
`--` is a term, even when it looks like an option. A program that passes a
selector it didn't write puts it after `--`.

| Term | Matches |
|---|---|
| `in:NAME` | Cards whose note has a folder named `NAME` anywhere in its path, or is itself named `NAME` (without its extension, the part after the last `.`). Names are compared ignoring case, in Unicode NFC: `in:grammar` matches `japanese/Grammar/wo.md`, `grammar.md` and `grammar.txt`, but not `grammar-notes.md`. |
| `path:GLOB` | Cards whose note's path, relative to the store's folder, matches the glob. `*` matches within one folder, `**` across folders: `path:japanese/*` is the notes directly in `japanese/`, `path:japanese/**` everything below it. Paths and globs are compared in Unicode NFC (see [store.md](store.md), "File names"). |
| `id:ID` | The card with that ID, or every card of the item with that ID: `id:k3m9q1` matches `k3m9q1.1` and `k3m9q1.2`. |
| `kind:K` | Cards whose item is of kind `K`: `qa`, which includes items with no `kind` entry, or a kind named in `[kinds]` of `.srl/config.toml` (see [cards.md](cards.md), "Kinds"). A selector without `kind:` matches items of every kind. |
| `status:S` | Cards with status `active`, `draft` or `suspended` (see [notes.md](notes.md), "Status") |
| `state:S` | Cards in a scheduling state: `new`, `due`, `young`, `mature`, `leech` (see [scheduling.md](scheduling.md)). States come from replaying the log, so a selector with `state:` needs the configuration and the local time zone. |
| `word` or `"some words"` | Cards whose item contains the text, case-insensitively, in any field |

`in:` is a quick way to name a topic, taken from how you already arrange
your notes: there are no tags to write.

Keys and text are told apart by one rule: a term that starts with lowercase
letters `a`–`z` and a colon is a `key:value` term, and anything else is text.
A key that is not in the table above is an error, so a typo such as `ni:x`
fails instead of silently searching for the text `ni:x`. To search for text
like that, put it in double quotes: `"std::move"`.

Double quotes make a term of everything up to the next `"`, spaces included,
and it is always text. `-` before the opening quote negates it. Quotes must be
closed, and followed by a space or the end. Your shell removes one level of
quotes, so from the command line write `srl find '"some words"'`. Without the
inner quotes, `srl find "some words"` is two terms, `some` and `words`. That
example is for POSIX shells (sh, bash, zsh) and PowerShell 7.3 or later.
In Windows PowerShell 5.1, write `srl find '\"some words\"'`; in `cmd.exe`,
write `srl find "\"some words\""`.

A value can't be empty (`in:` or `in:a,,b`), and `id:`, `kind:`, `status:`
and `state:` values must be valid IDs, kinds, statuses and states. So a
typo such as `kind:ja-word` fails instead of matching nothing.

Three combining rules, and nothing else:

| Syntax | Meaning |
|---|---|
| `key:a,b` | Comma: **or**. `in:a,b` is cards in `a` or in `b`. |
| `term term` | Space: **and**. Both must match. |
| `-term` | Minus: **not**. `-in:a` excludes cards in `a`. |

There is no grouping and no operator precedence. If you need something these
rules can't express, run two commands.

An empty selector matches every card.

## Defaults per command

One command adds a term of its own:

| Command | Adds |
|---|---|
| `srl next` | `status:active` (drafts and suspended cards are never studied) |

Every other command applies the selector exactly as written.

## Examples

```sh
srl next in:grammar,kanji                 # grammar or kanji notes
srl find in:japanese -in:n5               # all Japanese except the n5 folder
srl find state:leech --json               # every leech
srl find "ものともせず"                    # text search
srl reset path:rust/ownership.md          # start a note over
srl find in:vocab state:mature            # what you know well
srl next kind:ja-words                    # study one kind of item
srl find -kind:qa                         # every item of a kind program
```

Commands that print cards follow `grep` exit codes: 0 if anything matched, 1
if nothing did, 2 on error.
