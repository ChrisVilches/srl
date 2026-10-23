# Frontends

A **frontend** is a program that shows cards to a person and records the
results. `srl` ships one, the terminal frontend `srl study`. Others are
separate programs: a web viewer, a shell-prompt question, a script. The
repository's `examples/` has two, not part of `srl` (see
[overview.md](overview.md), "Examples"): the web viewer `srl web` and the
Bash frontend `srl quiz`.

Frontends understand the card format, and read an item's context from
`srl show`, and nothing else. They don't know about notes, kinds or cloze
syntax, so any frontend can show any card.

## The frontend contract

Every frontend runs the same loop:

1. **Get a card:** `srl next SELECTOR --json`.
2. **Show the prompt**, and wait until the person wants to see the answer.
3. **Show the answer**, then the item's context, which
   `srl show ID --json` gives as `context` (see [notes.md](notes.md),
   "Context"). The note it comes from is `see`, or `path` when `see` is
   `null`; a frontend may name it.
4. **Get a rating** from 1 to 4.
5. **Record it:** the card ID and the rating to `srl review --stdin`.

The person rates every card; nothing is judged automatically. A frontend may
also collect an answer before step 3 and pass it as `response`, which is
only kept in the log.

A frontend never computes schedules. That belongs to the core, so every
frontend behaves the same.

## Rendering

Card text and context are Markdown; card text may also hold `==highlight==`
(see [cards.md](cards.md)).

- How highlighted text looks is a frontend setting, never card content. The
  terminal frontend uses bold yellow; a web frontend uses CSS.
- Printing the raw text must always remain acceptable. A simple frontend can
  skip rendering entirely.

## The terminal frontend: `srl study`

`srl study [SELECTOR]` runs the loop above in the terminal until nothing is
left, then prints a short summary (reviews done, new cards introduced,
Again count): `5 cards reviewed, 2 new, 1 Again.` It reads the notes and
the log again before every card, so edits and reviews from other frontends
count at once. Like every interactive command, it refuses to run when
standard input is not a terminal.

Input is read a line at a time: type, then Enter.

- The prompt: Enter shows the answer, followed by the context if there is
  one, under a line `--- PATH` naming the note it comes from (the item's
  `see:`, or its own note); `q` quits.
- The rating: `1`–`4`. `e` opens the item's note in the editor (see
  [cli.md](cli.md), "Editing in the editor") and asks for the rating again.
  `s` suspends the card without recording a review. `q` quits without
  recording this card. Anything else asks again.
- The end of input (Ctrl-D, or Ctrl-Z then Enter on Windows) quits at any
  point, without recording the card on screen.

`srl study` never asks for a typed answer, so its reviews have no
`response`.

Highlighted text (`==…==`) is bold yellow when standard output is a terminal
and `NO_COLOR` is not set, and raw otherwise. Other Markdown is printed as
it is.

On Windows, `srl study` turns on the console's virtual terminal mode, which
makes it show the color codes; the old console (conhost) needs this,
Windows Terminal has it on already. If that fails (Windows before 10, or
standard error redirected to a file, since the mode is set for both
streams), highlights are printed raw and the reason is reported on standard
error.

> **TODO (platform):** not run on Windows yet.

## The web viewer: `srl web`

The web viewer is an example, not part of `srl`: an npm package in the
repository's `examples/web-frontend/`, which puts `srl-web` on `PATH`, so
that `srl web [SELECTOR]` starts it. Its README says how to install and
change it. It is a frontend like any other, split in two:

```
browser (React page)                  srl-web (Node.js, on localhost)    srl
────────────────────                  ───────────────────────────────    ───
render Markdown + ==mark==   ◄─JSON─  GET  /api/next?sel=…  ──runs──►  srl next SEL --json
buttons, keys, CSS theme     ◄─JSON─  GET  /api/find?sel=…  ──runs──►  srl find SEL --json
                             ◄─JSON─  GET  /api/item?id=…   ──runs──►  srl show ID --json
                             ──JSON─► POST /api/review      ──runs──►  srl review --stdin
                             ──JSON─► POST /api/suspend     ──runs──►  srl suspend id:CARD
```

- **The server contains no SRS logic.** Each route runs one CLI command and
  returns its JSON; when `srl` fails, it returns `srl`'s message, which the
  page shows. `--dir DIR` is passed to every command; without it, `srl`
  finds the store as always.
- **The page runs the frontend loop** of "The frontend contract" above,
  with the keys of `srl study`: space or Enter shows the answer, `1`–`4`
  rate, `s` suspends. The selector can be changed in the page, and the
  counts of `srl study`'s summary are kept at the top.
- **Card text and context are rendered as Markdown** with markdown-it. Raw
  HTML is shown as text. Highlights are paired in order, each `==` with the
  next one, as in the terminal frontends: Markdown plugins that treat `==`
  like `*` nest the marks in text without spaces, such as
  `==[...]==を==[...]==`.
- **Only the browser on the same computer can use it.** The server listens
  on `127.0.0.1`, and refuses requests for any host but localhost and POST
  bodies that are not JSON, so that other web pages can't record reviews.

Because scheduling lives in the core, the web viewer and the terminal behave
identically.

## Small examples

A question in the shell prompt, rated later:

```sh
# in .zshrc: show one due card before each prompt, if any
precmd() { srl next --json 2>/dev/null | jq -r '"srl: " + .prompt' }
```

A frontend in a few lines of shell, without the context:

```sh
while card=$(srl next "$1" --json); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r .answer <<<"$card"; read -r -p 'rating 1-4: ' r
  jq -n --argjson card "$card" --argjson rating "$r" '{card: $card.id, rating: $rating}' | srl review --stdin
done
```

A fuller one, with keys that act without Enter, colors, and the context
written to a file for another program to show, is in the repository's
`examples/bash-frontend/` (see [overview.md](overview.md), "Examples").
