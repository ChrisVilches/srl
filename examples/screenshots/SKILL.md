---
name: srl-screenshots
description: Take or update the screenshots of srl-tui and srl-web in examples/screenshots/images/, from the demo store in examples/screenshots/store/.
---

# Taking screenshots of the frontends

Instructions for an agent asked to take new screenshots of the example
frontends, `srl-tui` (`examples/python-frontend/`) and `srl-web`
(`examples/web-frontend/`), or to change the ones in `images/`. Read all
of it before starting.

Tried on Linux only: it needs tmux, which Windows doesn't have.

## The files

Every path below is relative to `examples/screenshots/`, and **every
command runs from there**.

| Path | What it is |
|---|---|
| `images/` | The accepted screenshots: `tui-*.png` and `web-*-dark.png`. |
| `store/` | The demo store: notes about geography, software, Japanese, Chinese and electrical engineering, and `.srl/config.toml`. Its items already have IDs. |
| `store/kinds/` | Not in git: the `ja-words` kind program, copied in by step 2 below. |
| `capture-tui.sh` | Runs `srl-tui` in tmux on each shot's selector and saves the screen, with its colors, as a `.ansi` file. |
| `tui.html` | A terminal in a web page (xterm.js) that draws a `.ansi` file with the fonts and colors of the shots. |
| `shoot.mjs` | Takes the screenshots in headless Firefox: the `.ansi` files drawn in `tui.html`, and `srl-web`'s pages. |

## How the screenshots are made

**The TUI shots are not screenshots of a real terminal.** `srl-tui` runs in
a detached tmux session; `tmux capture-pane -e` saves its screen as text
with Rich's color codes; `tui.html` draws that text with xterm.js; and
Firefox takes a picture of it. The text, colors and layout are what
`srl-tui` printed; the font and the color theme are chosen in `tui.html`.
Tell the user so if they ask how the shots were made.

**The web shots are screenshots of `srl-web`** in headless Firefox, driven
by Puppeteer over WebDriver BiDi, so the system's Firefox is used and no
browser is downloaded.

No shot rates a card: shots only show cards and turn them, so the store's
log stays empty and the same cards come up every time. Taking the shots
again on the same machine, with nothing changed, gives the same images,
byte for byte.

## Rules

- **Never touch the user's own store.** `SRL_DIR` is often set in the
  user's shell and points at their real notes. Pass the demo store
  explicitly to every `srl` command, script and server, as `SRL_DIR=…` or
  `--dir …`.
- **Take the shots from a copy of the store**, in a new folder in `/tmp`,
  so nothing written while studying (the log, the cache, an item added
  in `srl-web`) reaches the repository.
- **Use new, literal folder names** for the copy and the output, such as
  `/tmp/srl-screenshots-store` and `/tmp/srl-shots`. If one exists, pick
  another name; don't delete it, and never run `rm` on a path held in a
  variable.
- **Show the user the shots before keeping them.** Take them into `/tmp`,
  look at every image yourself, tell the user where they are and what
  each shows, and copy them to `images/` only once the user accepts them.
- **Don't rate cards** while taking shots, by key or by command.
- **Keep references to the shots working.** Adding, removing and renaming
  shots is allowed, but other files link to images in `images/` by name,
  such as the README at the repository's root. Before keeping the shots,
  search the whole repository (outside `node_modules/`) for the name of
  every shot removed or renamed. Point each reference to the shot that
  now shows what it showed, a new one if need be, and update its alt
  text if the content changed. Remove a reference only when no shot
  fits. Then tell the user which references changed and to which shots,
  so they can look at them where they appear, and which were removed, if
  any.

## Steps

You need `srl` on `PATH`, tmux, uv (for `srl-tui`), Node.js 20 or later,
Firefox, the fonts Fira Code and Noto Sans Mono CJK JP, and MeCab with its
IPA dictionary (for the `ja-words` cards; see `../kind-ja-words/`).

1. **Install** this folder's dependencies, and build `srl-web`:

   ```sh
   npm install
   (cd ../web-frontend && npm install && npm run build)
   ```

2. **Put the `ja-words` program in the store.** Its configuration names
   it as `kinds/ja_words.py`, relative to the store's folder; the program
   itself lives in `../kind-ja-words/`, and `store/kinds/` is ignored by
   git:

   ```sh
   mkdir -p store/kinds
   cp ../kind-ja-words/ja_words.py store/kinds/
   ```

3. **Copy the store** to a new folder, and check it:

   ```sh
   mkdir /tmp/srl-screenshots-store
   cp -r store/. /tmp/srl-screenshots-store/
   SRL_DIR=/tmp/srl-screenshots-store srl check
   ```

4. **Take the TUI shots:**

   ```sh
   ./capture-tui.sh /tmp/srl-screenshots-store /tmp/srl-shots/ansi
   node shoot.mjs tui /tmp/srl-shots/ansi /tmp/srl-shots
   ```

5. **Take the web shots**, with `srl-web` running on port 4417 over the
   same store, and stop it afterwards:

   ```sh
   (cd ../web-frontend && node bin/srl-web.js --dir /tmp/srl-screenshots-store --no-open --port 4417) &
   node shoot.mjs web /tmp/srl-shots
   kill %1
   ```

   Set `FIREFOX` to the Firefox program if it isn't `/usr/bin/firefox`.

6. **Look at every image**, then show the user (see "Rules"). Once they
   accept them, copy them to `images/`, fix the references to removed or
   renamed shots (see "Rules"), and commit.

## Changing what is shown

- **Other cards:** the selectors are in `capture-tui.sh` (`capture NAME
  SELECTOR REVEAL`) and in `shoot.mjs` (`study(…)`, and the card clicked
  in Browse). `srl next SELECTOR --json` on the copied store tells which
  card a selector shows.
- **Other notes:** edit or add notes in `store/`, then give new items
  their IDs in place, so every copy gets the same cards:
  `SRL_DIR="$PWD/store" srl id` (after step 2). Write the notes as plain
  Markdown that a terminal can show: sub- and superscripts as Unicode
  characters (R₁, I²), which terminals show; no LaTeX, which neither
  frontend renders.
- **Size, fonts, colors:** see "The user's current preferences" below, and
  the names given there.

## The user's current preferences

The shots in `images/` were taken with these choices. They are the user's
current preferences, not requirements: follow them unless the user asks
for something else, and update this section when they change.

- **Dark theme only.** The web shots are dark (`SCHEMES` in `shoot.mjs`).
  `srl-web` follows the system's setting and also has a light theme; the
  user is used to dark.
- **Web width:** 980 CSS pixels for Study and Add, so the page has little
  empty space at its sides, and 1280 for Browse, which stacks its list
  above the card when narrower (`WIDTH` and `BROWSE_WIDTH` in
  `shoot.mjs`). Pages are taken at twice their size, and in full, past
  the bottom of the window.
- **Terminal size:** 108 columns (`cols` in `capture-tui.sh`); the rows are
  as many as the screen uses, so there is no empty space under the keys.
- **Terminal font:** Fira Code for Latin text, at 30 px with a line height
  of 1.12 (`tui.html`), so the rating labels sit centered in their colored
  boxes. CJK text is Noto Sans Mono CJK JP scaled to 120% (`size-adjust`
  in `tui.html`), so that each character fills exactly two Fira Code
  cells. Fira Code alone leaves gaps between CJK characters; Noto alone
  puts Latin text low in its cells.
- **Terminal colors:** a Catppuccin Mocha-like theme, with black darkened
  to `#181825` and `drawBoldTextInBrightColors` off, so the labels on the
  rating keys (`bold black on red` and so on) are dark, not grey.

## Pitfalls already met

- xterm.js's canvas renderer draws everything twice too big when the page
  is zoomed (`deviceScaleFactor`), so the TUI is drawn at scale 1 with a
  font twice as big instead, which gives the same resolution.
- Firefox over WebDriver BiDi can't emulate `prefers-color-scheme`;
  `shoot.mjs` sets Firefox's own preference for it instead, with a
  browser per scheme. It also has no key named `"Space"`: press `" "`.
- Firefox's scrollbars stay light in the dark scheme, so `shoot.mjs` hides
  them.
- Playwright's own browsers are a large download that may stall; that is
  why the system's Firefox is used.
- In `srl-web`, inline code in notes shows its backticks, from the
  Tailwind typography plugin's default style. It is how `srl-web` looks,
  not a fault of the shots.
