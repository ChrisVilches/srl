# A frontend of your own, in Bash

`srl` has no built-in frontend: you study with a program of your choice.
`srl-quiz` is one for the terminal, with opinions, as a starting point for
a frontend that suits you. It is a Bash script for Linux, and needs `srl`
and `jq`.

- Keys act at once, without Enter: space shows the answer, `1`–`4` rate,
  `e` edits the note, `s` suspends the card, `q` quits.
- The prompt and the answer are in color, under `Q` and `A` badges, with
  highlights in bold yellow. With `NO_COLOR` set, or when standard output
  isn't a terminal, it prints plain text.
- The context is not printed with the answer. It is written to
  `/tmp/srl-quiz-context.md` (the constant `CONTEXT_FILE` at the top of the
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

It takes a selector like `srl next`, and ends with a summary: the cards
reviewed, the new ones among them, and the Again ratings.

## Making it yours

The script is one file of plain Bash, running the frontend loop of
[docs/frontends.md](../../docs/frontends.md): `srl next --json` for a card,
its state and its context, `srl review --stdin` for the rating.
Copy it under another name (`srl-NAME` runs as `srl NAME`) and change what
you like: the colors at the top, the layout of a card, the keys, where the
context goes.
