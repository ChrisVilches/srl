# A terminal frontend in Python

`srl-tui` is a study frontend for the terminal, written in Python with
[Rich](https://github.com/Textualize/rich): the web viewer's layout, made
for a terminal, as a starting point for a frontend that suits you. It needs
`srl`, Python 3.10 or later and [uv](https://docs.astral.sh/uv/), and should
work on Linux, macOS and Windows (it is tried on Linux only).

- One screen per card: a header with the selector, the card's ID and the
  counters, the card in a panel, and the keys at the bottom.
- Card text and context are rendered as Markdown, with `==highlights==` in
  bold yellow. The context is shown under the answer, titled with its note.
- Keys act at once, without Enter: space (or Enter) shows the answer,
  `1`–`4` rate, `e` edits the note, `s` suspends the card, `q` quits.
- It ends with a summary: the cards reviewed, the new ones among them, and
  the Again ratings.

It is a uv project of its own, kept apart from `srl` and from the other
examples: everything it needs is in this folder.

## Using it

```sh
uv tool install --editable .    # puts srl-tui on PATH
srl tui in:grammar
```

It takes a selector like `srl next`, and `srl` finds the store as always
(from the current directory, or `SRL_DIR`). Without installing it,
`uv run srl-tui in:grammar` runs it from this folder.

## Making it yours

It is one file, `src/srl_tui/__init__.py`, running the frontend loop of
[docs/frontends.md](../../docs/frontends.md): `srl next --json` for a card,
its state and its context, `srl review --stdin` for the rating. Change what
you like: the colors in `THEME` and `RATINGS` at the top, the layout in
`Session.show`, the keys in `Session.run`.

Highlights are paired in order, each `==` with the next one, as the other
frontends do (`highlight`): plugins that treat `==` like `*` nest the marks
in text without spaces, such as `==[...]==を==[...]==`.
