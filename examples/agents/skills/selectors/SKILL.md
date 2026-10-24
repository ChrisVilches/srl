---
name: selectors
description: The complete srl selector language (in:, path:, id:, status:, state:, text search, comma, minus) and how to quote selectors in sh, bash, zsh, PowerShell and cmd. Load when building a selector that is more than one or two simple terms, when a selector gives an error, or when text search needs quotes.
---

# Selectors

A selector picks cards. `srl find`, `srl next`, `srl stats`, `srl log`,
`srl accept`, `srl suspend`, `srl unsuspend`, `srl reset` and `srl drafts`
all take one, and it means the same everywhere.

## Terms

| Term | Matches |
|---|---|
| `in:NAME` | Cards whose note has a folder named `NAME` anywhere in its path, or is itself named `NAME` plus an extension (`NAME.md`, `NAME.txt`, …). Ignores case. `in:grammar` matches `japanese/Grammar/wo.md` and `grammar.md`, not `grammar-notes.md`. |
| `path:GLOB` | The note's path, relative to the store. `*` stays within a folder, `**` crosses folders: `path:japanese/*` is notes directly in `japanese/`, `path:japanese/**` everything below it, `path:rust/ownership.md` one note. |
| `id:ID` | A card (`id:k3m9q1-2`), or every card of an item (`id:k3m9q1`). |
| `status:S` | `active`, `draft` or `suspended`. |
| `state:S` | `new` (never reviewed since reset), `due` (due today or earlier), `young` and `mature` (not due; stability under or over 21 days), `leech` (forgotten 8 times or more; overlaps the others). |
| `word` | Items containing the text in any field, ignoring case. |
| `"some words"` | The same, for text with spaces. Always text. |

## Combining

| Syntax | Meaning |
|---|---|
| `key:a,b` | or: `in:grammar,kanji` |
| `term term` | and: `in:rust state:due` |
| `-term` | not: `-in:n5`, `-"some words"` |

There is no grouping and no precedence. If something can't be said with
these, run two commands. An empty selector matches every card.

A term that starts with lowercase letters and a colon is a `key:value`
term, and an unknown key is an error (`ni:x` fails rather than searching
for text). To search for text containing a colon, quote it: `"std::move"`
(from the shell, `srl find '"std::move"'`).

## Defaults per command

`srl next` adds `status:active`: drafts and suspended cards are never
studied. `srl drafts` adds `status:draft`. Every other command
applies the selector as written, so `srl stats` counts drafts and suspended
cards unless told `status:active`.

## Quoting in the shell

Separate arguments are joined with spaces: `srl find in:a in:b` and
`srl find "in:a in:b"` are the same. The shell removes one level of quotes,
so a text term with spaces needs two:

| Shell | Write |
|---|---|
| sh, bash, zsh, PowerShell 7.3+ | `srl find '"some words"'` |
| Windows PowerShell 5.1 | `srl find '\"some words\"'` |
| cmd.exe | `srl find "\"some words\""` |

Without the inner quotes, `srl find "some words"` is two terms, `some` and
`words`, both required.

Quote globs too: zsh fails on an unquoted `*` that matches no file, and
other shells may replace it with file names. Write
`srl find 'path:japanese/**'`.

## Examples

```sh
srl find in:japanese -in:n5          # all Japanese except the n5 folder
srl find in:rust -state:new          # rust cards already studied
srl find state:leech                 # cards that keep being forgotten
srl find "ものともせず"               # text search
srl stats in:vocab status:active     # progress on studied vocabulary
srl accept 'path:generated/**'       # accept every draft under generated/
srl reset path:rust/ownership.md     # start one note over
srl suspend id:k3m9q1                # stop studying every card of an item
```

Commands that list cards exit with 0 if something matched, 1 if nothing
did, and 2 on an error such as a bad selector.
