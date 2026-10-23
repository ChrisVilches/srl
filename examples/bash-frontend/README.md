# A frontend of your own, in Bash

`srl study` is plain on purpose: it prints card text as it is, reads a line
at a time, and works the same everywhere. `srl-quiz` is the same loop with
opinions, as a starting point for a frontend that suits you. It is a Bash
script for Linux, and needs `srl` and `jq`.

- Keys act at once, without Enter: space shows the answer, `1`–`4` rate,
  `e` edits the note, `s` suspends the card, `q` quits.
- The prompt and the answer are in color, under `Q` and `A` badges, with
  highlights in bold yellow. With `NO_COLOR` set, or when standard output
  isn't a terminal, it prints plain text.
- The context is not printed with the answer. It is written to
  `/tmp/srl-context.md` (the constant `CONTEXT_FILE` at the top of the
  script) when the answer is shown, and emptied when the next prompt is
  shown. Only the note's path is printed. Whatever you want to do with the
  context, such as showing it rendered in another pane, is up to you; the
  script only writes the file.

## Using it

Put the script on your `PATH`, and `srl` runs it as a command of its own:

```sh
cp srl-quiz ~/.local/bin/
srl quiz in:grammar
```

It takes a selector like `srl study`, and ends with the same summary.

## Making it yours

The script is one file of plain Bash, running the frontend loop of
[docs/frontends.md](../../docs/frontends.md): `srl next --json` for a card,
`srl show --json` for its context, `srl review --stdin` for the rating.
Copy it under another name (`srl-NAME` runs as `srl NAME`) and change what
you like: the colors at the top, the layout of a card, the keys, where the
context goes.
