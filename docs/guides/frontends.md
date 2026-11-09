# Writing a frontend

This is a guide, not part of the specification (see
[overview.md](../overview.md), "Specification, guides and examples"). A
frontend is any program that runs the loop of
[frontends.md](../frontends.md). This guide shows how small one can be, and
where to find fuller ones to copy.

## A frontend in a few lines of shell

```sh
while card=$(srl next --json -- "$1"); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r '.prompt_revealed // .prompt, .answer // empty, "---", .context // empty' <<<"$card"; read -r -p 'rating 1-4: ' r
  srl review "$(jq -r .id <<<"$card")" --rating "$r"
done
```

It runs the whole loop: get a card, show the prompt, show the revealed
prompt, the answer and the context, get a rating, record it. A terminal
can't replace the prompt it printed, so it prints the revealed prompt below
it. The loop ends when `srl next`
exits with 1, because nothing is left, or with 2 on an error, such as a
card its kind couldn't make; `srl` prints the reason on standard error.
Showing the context is this frontend's choice; `--no-context` would leave
it out.

## Where items come from

A frontend needs to know nothing of the store's sources (see
[sources.md](../sources.md)): every card comes the same way, whatever
source its item is in. What it may use is in the JSON it gets: `collection`
names the item's collection, for people and for `srl add`, and `see` the
note the item asks about, relative to the store's folder, for a frontend
that opens it. An item is changed through `srl show ID --json` and `srl
edit --stdin`, whatever its source. A form that adds items picks a
collection from `srl collections`, or offers a new one where `srl sources`
says a source can create it, and a note for `see` from `srl notes`: the
two are separate lists, since a collection is never a context.

## The examples

The repository's `examples/` has two fuller frontends (see
[overview.md](../overview.md), "Specification, guides and examples"). They
are made to be copied and changed. Both are meant for Linux, macOS and
Windows, but are tried on Linux only; their READMEs say how to install
them, what they need and what may differ elsewhere.

- **`srl-web`**, in `examples/web-frontend/`: a web viewer. A small
  Node.js server on localhost runs `srl next`, `srl review` and
  `srl suspend` for a React page, which renders card text and context as
  Markdown. Its card browser lists cards with `srl find` and makes any of
  them with `srl cards`, to see how a kind's cards look without studying.
  Its Add tab writes items: the fields of a kind from `srl kinds`, a collection of any source
  from `srl collections`, or a new one where `srl sources` says a source
  can create it, and a note as context from `srl notes`, checked with
  `srl add --dry-run` and added with `srl add`, so it knows no rule of
  any kind.
- **`srl-tui`**, in `examples/python-frontend/`: a terminal frontend in
  Python with Rich, with keys that act without Enter, and the card, the
  answer and the context rendered as Markdown on one screen.

Screenshots of both are in `examples/screenshots/images/`; how to take new
ones is in `examples/screenshots/SKILL.md`.

Both keep everything about appearance to themselves: colors, keys, layout,
how `==highlights==` look (bold yellow in `srl-tui`, CSS in `srl-web`), and
how they show the context. Each pairs highlights in order, each `==`
with the next one: Markdown plugins that treat `==` like `*` nest the marks
in text without spaces, such as `==[...]==を==[...]==`.

Both render a context by the extension of its note, `see` (see
[frontends.md](../frontends.md), "Rendering"): `.md`, `.markdown`, `.mdx`
and `.qmd` as Markdown, and every other context as a code block of plain
text, so that the lines of a text note don't run together into one
paragraph and its backticks aren't taken as code spans. The fence is
longer than any run of backticks in the context. A note `srl` gives no
text for, such as a PDF, is named by its path alone, and an item without
`see` shows no context.
