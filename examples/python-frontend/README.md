# A terminal frontend in Python

`srl-tui` is a study frontend for the terminal, written in Python with
[Rich](https://github.com/Textualize/rich): the web viewer's layout, made
for a terminal, as a starting point for a frontend that suits you. It should
work on Linux, macOS and Windows (it is tried on Linux only).

- One screen per card: a header with the selector, the card's ID and the
  counters, the card in a panel, and the keys at the bottom.
- Card text and context are rendered as Markdown, with `==highlights==` in
  bold yellow. The context is shown under the answer, titled with its note.
- The screen never scrolls: the question and the answer always stay in
  view. A context too long for the space left under them scrolls on its
  own, inside its panel.
- Keys act at once, without Enter: space (or Enter) shows the answer,
  `1`–`4` rate, `j`/`k` or the arrows scroll the context, `e` edits the
  note, `s` suspends the card, `q` quits.
- It ends with a summary: the cards reviewed, the new ones among them, and
  the Again ratings.

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

To use it from anywhere, as `srl tui`, install it as a uv tool:

```sh
uv tool install --editable .    # puts srl-tui in ~/.local/bin
uv tool update-shell            # only if ~/.local/bin is not on PATH yet
srl tui in:grammar
```

With `--editable`, your changes to the code take effect without
installing again. `uv tool uninstall srl-tui` removes it.

It takes a selector like `srl next` (none means every card), and `srl`
finds the store as always: `SRL_DIR` if it is set, otherwise the nearest
store from the current directory up (see
[docs/store.md](../../docs/store.md)).

## Making it yours

It is one file, `src/srl_tui/__init__.py`, running the frontend loop of
[docs/frontends.md](../../docs/frontends.md): `srl next --json` for a card,
its state and its context, `srl review --stdin` for the rating. Change what
you like: the colors in `THEME` and `RATINGS` at the top, the layout in
`Session.show`, the keys in `Session.run`.

Highlights are paired in order, each `==` with the next one, as the other
frontends do (`highlight`): plugins that treat `==` like `*` nest the marks
in text without spaces, such as `==[...]==を==[...]==`.
