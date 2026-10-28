# Screenshots of the frontends

`images/` holds screenshots of the two example frontends, `srl-tui`
([`../python-frontend/`](../python-frontend/)) and `srl-web`
([`../web-frontend/`](../web-frontend/)), studying a small demo store.
This folder has everything needed to take them again: the store, and the
scripts that take them. A person or an agent can follow the steps below.

Tried on Linux only. It needs tmux, which Windows doesn't have, and the
path to Firefox differs between platforms (see `FIREFOX` below).

## What is here

| Path | What it is |
|---|---|
| `images/` | The screenshots: `tui-*.png` and `web-*-dark.png`. |
| `store/` | The demo store: notes about geography, software, Japanese, Chinese and electrical engineering, and its `.srl/config.toml`. Its items already have IDs. |
| `capture-tui.sh` | Runs `srl-tui` in tmux on each shot's selector and saves the screen, with its colors, as a `.ansi` file. |
| `tui.html` | A terminal in a web page (xterm.js) that draws a `.ansi` file with the fonts and colors of the shots. |
| `shoot.mjs` | Takes the screenshots in headless Firefox: the `.ansi` files drawn in `tui.html`, and `srl-web`'s pages. |

## How the screenshots are made

**The TUI shots are not screenshots of a real terminal.** `srl-tui` runs in
a detached tmux session; `tmux capture-pane -e` saves its screen as text
with Rich's color codes; `tui.html` draws that text with xterm.js, the
terminal VS Code uses; and Firefox takes a picture of it. The text, colors
and layout are exactly what `srl-tui` printed; the font and the color
theme are chosen in `tui.html`, and are a real terminal's own.

**The web shots are screenshots of `srl-web`** in headless Firefox, driven
by Puppeteer over WebDriver BiDi, so the system's Firefox is used and no
browser is downloaded.

No shot rates a card: they only show cards and turn them, so the demo
store's log stays empty and the same cards come up every time. Taking the
shots again on the same machine, with nothing changed, gives the same
images, byte for byte.

## Taking them again

You need `srl` on `PATH`, tmux, uv (for `srl-tui`), Node.js 20 or later,
Firefox, the fonts Fira Code and Noto Sans Mono CJK JP, and MeCab with its
IPA dictionary (for the `ja-words` cards; see
[`../kind-ja-words/`](../kind-ja-words/)).

1. **Install** this folder's dependencies, and build `srl-web`:

   ```sh
   npm install                                  # here
   (cd ../web-frontend && npm install && npm run build)
   ```

2. **Copy the store** to a new folder outside the repository, with the
   `ja-words` kind program its configuration names, so that nothing
   written while studying ends up in the repository:

   ```sh
   mkdir /tmp/srl-screenshots-store
   cp -r store/. /tmp/srl-screenshots-store/
   mkdir /tmp/srl-screenshots-store/kinds
   cp ../kind-ja-words/ja_words.py /tmp/srl-screenshots-store/kinds/
   SRL_DIR=/tmp/srl-screenshots-store srl check
   ```

   Always pass the store explicitly, as `SRL_DIR` or `--dir`: a `SRL_DIR`
   already set in your shell would point every command at your own store.

3. **Take the TUI shots:**

   ```sh
   ./capture-tui.sh /tmp/srl-screenshots-store /tmp/srl-shots/ansi
   node shoot.mjs tui /tmp/srl-shots/ansi /tmp/srl-shots
   ```

4. **Take the web shots**, with `srl-web` running on port 4417 over the
   same store:

   ```sh
   (cd ../web-frontend && node bin/srl-web.js --dir /tmp/srl-screenshots-store --no-open --port 4417) &
   node shoot.mjs web /tmp/srl-shots
   kill %1
   ```

   Set `FIREFOX` to the Firefox program if it isn't `/usr/bin/firefox`.

5. **Look at them in `/tmp/srl-shots`**, and copy the ones you keep to
   `images/`.

Which cards are shown is chosen by the selectors in `capture-tui.sh` and
`shoot.mjs`: change them, or the notes in `store/`, to show other cards.
After adding items to `store/`, give them IDs with
`SRL_DIR=store srl id`, so that every copy of the store has the same
cards.

## Current preferences

The shots in `images/` were taken with the choices below. They are the
current preferences of the project's author, not requirements: new shots
may change any of them.

- **Shots are reviewed before they're kept.** They're taken into a folder
  in `/tmp` first, looked at, and only then copied to `images/`.
- **Dark theme only.** The web shots are dark (`SCHEMES` in `shoot.mjs`);
  `srl-web` follows the system's setting, and also has a light theme.
- **Web width:** 980 CSS pixels for Study and Add, so the page has little
  empty space at its sides, and 1280 for Browse, which stacks its list
  above the card when narrower (`WIDTH` and `BROWSE_WIDTH` in
  `shoot.mjs`). Pages are taken at twice their size, and in full, past
  the bottom of the window.
- **Terminal size:** 108 columns (`cols` in `capture-tui.sh`); the rows are
  as many as the screen uses, so there is no empty space under the keys.
- **Terminal font:** Fira Code for Latin text, at 30 px with a line height
  of 1.12, so the rating labels sit centered in their colored boxes. CJK
  text is Noto Sans Mono CJK JP scaled to 120% (`size-adjust` in
  `tui.html`), so that each character fills exactly two Fira Code cells:
  without it, CJK text has gaps between characters. Noto alone puts Latin
  text low in its cells.
- **Terminal colors:** a Catppuccin Mocha-like theme, with black darkened
  to `#181825` and bold text not drawn in bright colors, so the labels on
  the rating keys (`bold black on red` and so on) are dark.
- **The demo store's notes stay plain Markdown that terminals can show.**
  Sub- and superscripts are Unicode characters (R₁, I²), which work in
  terminals; LaTeX, which neither frontend renders, is avoided.

## Notes

- xterm.js's canvas renderer draws everything twice too big when the page
  is zoomed (`deviceScaleFactor`), so the TUI is drawn at scale 1 with a
  font twice as big instead, which gives the same resolution.
- Firefox over WebDriver BiDi can't emulate `prefers-color-scheme`;
  `shoot.mjs` sets Firefox's own preference for it instead, with a
  browser per scheme.
- Firefox's scrollbars stay light in the dark scheme, so `shoot.mjs` hides
  them.
