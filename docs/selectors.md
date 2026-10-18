# Selectors

A **selector** picks a set of atoms. Every command that works on several atoms
takes one, and it means the same thing everywhere: for a person typing, a
script, or an agent.

```sh
srl study tag:jlpt/n1,kanji/food
srl find tag:rust -state:new
srl accept status:draft tag:jlpt/n1
```

## Syntax

A selector is a list of **terms** separated by spaces. Command-line arguments
are joined with spaces, so `srl find tag:a tag:b` and `srl find "tag:a tag:b"`
mean the same.

| Term | Matches |
|---|---|
| `tag:a` | Atoms with tag `a` or any tag under it (`tag:japanese` matches `japanese/grammar`) |
| `path:GLOB` | Atoms whose file path, relative to `atoms/`, matches the glob. `*` matches within one folder, `**` across folders: `path:japanese/*` is the files directly in `japanese/`, `path:japanese/**` everything below it. |
| `id:ID` | The atom with that ID |
| `status:S` | Atoms with status `active`, `draft` or `suspended` |
| `state:S` | Atoms in a scheduling state: `new`, `due`, `young`, `mature`, `leech` (see [scheduling.md](scheduling.md)) |
| `word` or `"some words"` | Atoms whose file contains the text, case-insensitively, anywhere: front-matter or body |

Keys and text are told apart by one rule: a term that starts with lowercase
letters `a`–`z` and a colon is a `key:value` term, and anything else is text.
A key that is not in the table above is an error, so a typo such as `tga:x`
fails instead of silently searching for the text `tga:x`. To search for text
like that, put it in double quotes: `"std::move"`.

Double quotes make a term of everything up to the next `"`, spaces included,
and it is always text. `-` before the opening quote negates it. Quotes must be
closed, and followed by a space or the end. Your shell removes one level of
quotes, so from the command line write `srl find '"some words"'`. Without the
inner quotes, `srl find "some words"` is two terms, `some` and `words`. That
example is for POSIX shells (sh, bash, zsh) and PowerShell 7.3 or later.
In Windows PowerShell 5.1, write `srl find '\"some words\"'`; in `cmd.exe`,
write `srl find "\"some words\""`.

A value can't be empty (`tag:` or `tag:a,,b`), and `id:` and `status:` values
must be valid IDs and statuses.

Three combining rules, and nothing else:

| Syntax | Meaning |
|---|---|
| `key:a,b` | Comma: **or**. `tag:a,b` is tag `a` or tag `b`. |
| `term term` | Space: **and**. Both must match. |
| `-term` | Minus: **not**. `-tag:a` excludes tag `a`. |

There is no grouping and no operator precedence. If you need something these
rules can't express, run two commands.

An empty selector matches every atom.

## Defaults per command

Some commands add a term of their own:

| Command | Adds |
|---|---|
| `srl next`, `srl study` | `status:active` (drafts and suspended atoms are never studied) |
| `srl drafts` | `status:draft` |

Every other command applies the selector exactly as written.

## Examples

```sh
srl next tag:jlpt/n1,kanji/food           # N1 grammar or food kanji
srl find tag:japanese -tag:jlpt/n5        # all Japanese except N5
srl find state:leech --json               # every leech
srl find "ものともせず"                    # text search
srl reset path:rust/ownership/*           # start a folder over
srl find tag:vocab state:mature           # what you know well
```

Commands that print atoms follow `grep` exit codes: 0 if anything matched, 1 if
nothing did, 2 on error.
