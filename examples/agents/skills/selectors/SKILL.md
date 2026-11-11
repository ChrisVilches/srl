---
name: selectors
description: The complete srl selector language (in:, source:, id:, kind:, is:, text search, comma, minus) and how to quote selectors in sh, bash, zsh, PowerShell and cmd. Load when building a selector that is more than one or two simple terms, when a selector gives an error, or when text search needs quotes.
---

# Selectors

A selector picks cards. `srl find`, `srl next`, `srl stats`, `srl log`,
`srl accept`, `srl suspend`, `srl unsuspend` and `srl reset` all take one, and it means the same everywhere.

## Terms

| Term | Matches |
|---|---|
| `in:NAME` | Cards whose item's collection has `NAME` among the `in` names its source gives it. A file has each of its folders, its file name, and its file name without its extension: `in:grammar` matches `japanese/Grammar/wo.jsonl`, `grammar.jsonl` and `grammar.xml`, not `grammar-notes.jsonl`; `in:grammar.xml` only the last. A source program gives the names it chooses, or the collection's whole name: the table `v1.2` matches `in:v1.2`, not `in:v1`. `srl collections --json` shows each collection's `in` names. Ignores case. Notes are not collections, so a note named `grammar.md` alone matches nothing. |
| `source:NAME` | Cards whose item comes from the source `NAME`: `source:items` for a store without `[sources]` (`srl sources` lists them). |
| `id:ID` | A card (`id:k3m9q1.2`), or every card of an item (`id:k3m9q1`). |
| `kind:K` | Cards whose item is of kind `K`: `qa`, which includes items with no `kind` entry, or a kind named in `[kinds]` of `.srl/config.toml`. An unknown kind is an error. Without `kind:`, items of every kind match. |
| `is:S` | A status: `active`, `draft` or `suspended`. Or a place in the schedule: `new` (never reviewed since reset), `due` (due today or earlier), `young` and `mature` (not due; stability under or over 21 days). Or `leech`: forgotten 8 times or more, whatever its place. `is:draft,leech` mixes them freely. |
| `word` | Items containing the text in any field, ignoring case. |
| `"some words"` | The same, for text with spaces. Always text. |

## Combining

| Syntax | Meaning |
|---|---|
| `key:a,b` | or: `in:grammar,kanji` |
| `term term` | and: `in:rust is:due` |
| `-term` | not: `-in:n5`, `-"some words"` |

There is no grouping and no precedence. If something can't be said with
these, run two commands. An empty selector matches every card.

A term that starts with lowercase letters and a colon is a `key:value`
term, and an unknown key is an error (`ni:x` fails rather than searching
for text). To search for text containing a colon, quote it: `"std::move"`
(from the shell, `srl find '"std::move"'`).

## Defaults per command

`srl next` adds `is:active`: drafts and suspended cards are never
studied. Every other command
applies the selector as written, so `srl stats` counts drafts and suspended
cards unless told `is:active`.

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

Every argument that is not one of the command's options is a term, even
one starting with `-` (`srl find -in:n5`). When you pass a selector you
didn't write, such as one the person typed, put it after `--`, so nothing
in it is read as an option: `srl find -- "$SEL"`.

A value can't be empty (`in:`, `in:a,,b`), and the values of `source:`,
`id:`, `kind:` and `is:` must be valid (a source of the store, an ID, a
kind, a value listed for `is:`), so a typo such as `source:itmes`,
`kind:ja-word` or `is:leeches` fails instead of matching nothing.

## Examples

```sh
srl find in:japanese -in:n5          # all Japanese except the n5 folder
srl find in:rust -is:new             # rust cards already studied
srl find is:leech                    # cards that keep being forgotten
srl find "ものともせず"               # text search
srl stats in:vocab is:active         # progress on studied vocabulary
srl accept in:generated              # accept every draft under generated/
srl reset in:ownership.xml           # start one collection over
srl suspend id:k3m9q1                # stop studying every card of an item
srl next kind:ja-words               # study only items of one kind
```

Commands that list cards exit with 0 if something matched, 1 if nothing
did, and 2 on an error such as a bad selector.
