# A terminal frontend in Python

`srl-tui` is a study frontend for the terminal, written in Python with
[Rich](https://github.com/Textualize/rich): the web viewer's layout, made
for a terminal, as a starting point for a frontend that suits you.

It is meant for Linux, macOS and Windows, but is tried on Linux only. Rich,
uv and running `srl` work the same everywhere; the one part written per
platform is reading a key without Enter (`read_key`): `termios` on Linux
and macOS, `msvcrt` on Windows. On Windows that part has never run, so the
arrow keys may not scroll there (`j`/`k` do the same), and the alternate
screen needs a modern terminal such as Windows Terminal.

- One screen per card: a header with the selector, the card's ID and kind
  (in magenta when it isn't `qa`, such as a kind that asks an LLM) and the
  counters, the card in a panel, and the keys at the bottom.
- Card text and context are rendered as Markdown by Rich (CommonMark plus
  GFM tables and strikethrough), with `==highlights==` in bold yellow.
- Showing the answer turns the question into its revealed prompt, with a
  cloze's blanks filled in (or leaves it as it is when the card has none,
  `prompt_revealed` being `null`), and adds the answer below it when there
  is one.
- The context is shown under the answer, titled with its note (`see`, or
  else `path`). It asks for the `prose` context: the note without its
  items.
- The screen never scrolls: the card and its context each scroll inside
  their own panel. When both are too long for the screen, each gets half
  of it; when one is shorter than half, the other takes the rest. A panel
  that can't show everything says which lines it shows.
- One panel has the focus, with the brighter border: the card on the
  question side, the context once the answer is shown (the card when
  there is no context). Tab moves the focus to the other panel.
- When `srl` takes a while, as when `srl next` runs a kind program that
  asks an LLM, the screen shows a spinner, "Loading…", and the seconds
  taken; Ctrl-C stops the wait.
- Keys act at once, without Enter: space (or Enter) shows the answer,
  `1`–`4` rate, `j`/`k` or the arrows scroll the panel in focus,
  Ctrl-D/Ctrl-U by half of it, `g`/`G` to its top and bottom, Tab
  switches the focus, `s` suspends the card, `q` (or Ctrl-C) quits.
- It ends with a summary: the cards reviewed, the new ones among them, and
  the Again ratings. If `srl` fails, it prints what `srl` said and exits
  with 2.

It is a uv project of its own, kept apart from `srl` and from the other
examples: everything it needs is in this folder.

## What you need

- **`srl`**, on `PATH`: `srl --help` should work.
- **[uv](https://docs.astral.sh/uv/)**, which manages everything else: the
  Python version, a virtual environment and the dependencies (Rich).
  Install it with your package manager (`pacman -S uv`, `brew install uv`,
  `winget install astral-sh.uv`, …) or with its installer:

  ```sh
  curl -LsSf https://astral.sh/uv/install.sh | sh                                # Linux, macOS
  powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"  # Windows
  ```

You don't need to install Python yourself: if there is no Python 3.10 or
later on the system, uv downloads one the first time it runs the project.

## Running it

From this folder, without installing anything else:

```sh
uv run srl-tui in:grammar
```

The first run creates `.venv/` here and installs the versions locked in
`uv.lock`; later runs start at once.

To use it from anywhere, install it as a uv tool:

```sh
uv tool install --editable .    # puts srl-tui in ~/.local/bin
uv tool update-shell            # only if ~/.local/bin is not on PATH yet
srl-tui in:grammar
```

With `--editable`, your changes to the code take effect without
installing again. `uv tool uninstall srl-tui` removes it.

It takes a selector like `srl next` (none means every card), and nothing
else: an argument starting with `--` is refused, except `--help`, and the
terms reach `srl next` after `--`, so none of them is read as an option of
`srl`'s. `srl` finds the store as always: `SRL_DIR` if it is set,
otherwise the nearest store from the current directory up (see
[docs/store.md](../../docs/store.md)).

## Making it yours

It is one file, `src/srl_tui/__init__.py`, running the frontend loop of
[docs/frontends.md](../../docs/frontends.md): `srl next --json --context
CONTEXT` for a card, its state, kind and context, `srl review --stdin` for
the rating. Change what you like: the colors in `THEME` and `RATINGS`, the
context mode in `CONTEXT` (`prose`, `full` or `none`), the layout in
`Session.show`, the keys in `QUESTION_KEYS` and `ANSWER_KEYS` and what
they do in `Session.run`.

Highlights are paired in order, each `==` with the next one, as `srl-web`
does (`highlight`): plugins that treat `==` like `*` nest the marks
in text without spaces, such as `==[...]==を==[...]==`.
