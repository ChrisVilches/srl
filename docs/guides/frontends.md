# Writing a frontend

This is a guide, not part of the specification (see
[overview.md](../overview.md), "Specification, guides and examples"). A
frontend is any program that runs the loop of
[frontends.md](../frontends.md). This guide shows how small one can be, and
where to find fuller ones to copy.

## A frontend in a few lines of shell

```sh
while card=$(srl next --json --context prose -- "$1"); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r '.prompt_revealed // .prompt, .answer // empty, "---", .context // empty' <<<"$card"; read -r -p 'rating 1-4: ' r
  jq -n --argjson card "$card" --argjson rating "$r" '{card: $card.id, rating: $rating}' | srl review --stdin
done
```

It runs the whole loop: get a card, show the prompt, show the revealed
prompt, the answer and the context, get a rating, record it. A terminal
can't replace the prompt it printed, so it prints the revealed prompt below
it. The loop ends when `srl next`
exits with 1, because nothing is left, or with 2 on an error, such as a
card its kind couldn't make; `srl` prints the reason on standard error. `--context` is this frontend's
choice; `full` would show the items of the note too, and `none` nothing.

## A question in the shell prompt

```sh
# in .zshrc: show one due card before each prompt, if any
precmd() { srl next --json 2>/dev/null | jq -r '"srl: " + .prompt' }
```

This shows the prompt only and records nothing. Rate the card later, with
`srl review ID --rating N`, after looking up the answer with `srl cards ID`.

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
- **`srl-tui`**, in `examples/python-frontend/`: a terminal frontend in
  Python with Rich, with keys that act without Enter, and the card, the
  answer and the context rendered as Markdown on one screen.

Both keep everything about appearance to themselves: colors, keys, layout,
how `==highlights==` look (bold yellow in `srl-tui`, CSS in `srl-web`), and
the context mode they ask for. Each pairs highlights in order, each `==`
with the next one: Markdown plugins that treat `==` like `*` nest the marks
in text without spaces, such as `==[...]==を==[...]==`.
