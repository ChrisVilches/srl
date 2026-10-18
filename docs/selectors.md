# Selectors

A **selector** picks a set of atoms. Every command that works on several atoms
takes one, and it means the same thing everywhere: for a person typing, a
script, or an agent.

```sh
srs study tag:jlpt/n1,kanji/food
srs find tag:rust -state:new
srs accept status:draft tag:jlpt/n1
```

## Syntax

A selector is a list of **terms** separated by spaces. Command-line arguments
are joined with spaces, so `srs find tag:a tag:b` and `srs find "tag:a tag:b"`
mean the same.

| Term | Matches |
|---|---|
| `tag:a` | Atoms with tag `a` or any tag under it (`tag:japanese` matches `japanese/grammar`) |
| `path:GLOB` | Atoms whose file path, relative to `atoms/`, matches the glob (`path:japanese/*`) |
| `id:ID` | The atom with that ID |
| `status:S` | Atoms with status `active`, `draft` or `suspended` |
| `state:S` | Atoms in a scheduling state: `new`, `due`, `young`, `mature`, `leech` (see [scheduling.md](scheduling.md)) |
| `word` or `"some words"` | Atoms whose file contains the text, case-insensitively, anywhere: front-matter or body |

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
| `srs next`, `srs study` | `status:active` (drafts and suspended atoms are never studied) |
| `srs inbox` | `status:draft` |

Every other command applies the selector exactly as written.

## Examples

```sh
srs next tag:jlpt/n1,kanji/food           # N1 grammar or food kanji
srs find tag:japanese -tag:jlpt/n5        # all Japanese except N5
srs find state:leech --json               # every leech
srs find "ものともせず"                    # text search
srs reset path:rust/ownership/*           # start a folder over
srs find tag:vocab state:mature           # what you know well
```

Commands that print atoms follow `grep` exit codes: 0 if anything matched, 1 if
nothing did, 2 on error.
