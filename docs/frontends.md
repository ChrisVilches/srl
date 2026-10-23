# Frontends

A **frontend** is a program that shows cards to a person and records the
results. `srl` ships one, the terminal frontend `srl study`. Others are
separate programs: a web viewer, a shell-prompt question, a script.

Frontends understand the card format, and read an item's context from
`srl show`, and nothing else. They don't know about notes, kinds or cloze
syntax, so any frontend can show any card.

## The frontend contract

Every frontend runs the same loop:

1. **Get a card:** `srl next SELECTOR --json`.
2. **Show the prompt**, and wait until the person wants to see the answer.
3. **Show the answer**, then the item's context, which
   `srl show ID --json` gives as `context` (see [notes.md](notes.md),
   "Context").
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
  one, under a line `---`; `q` quits.
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

## A web viewer

A web viewer is a frontend like any other, split in two:

```
browser (HTML/JS)                     srl-web (small local server)       srl
──────────────────                    ────────────────────────────       ───
render Markdown + ==mark==   ◄─JSON─  GET  /next?sel=…    ──runs──►  srl next SEL --json
buttons, CSS theme           ◄─JSON─  GET  /item?id=…     ──runs──►  srl show ID --json
                             ──JSON─► POST /review        ──runs──►  srl review --stdin
```

- `srl-web` is a separate program on `PATH`, so `srl web` starts it. Each route
  runs one CLI command and returns its JSON. It contains no SRS logic.
- The browser renders Markdown with an ordinary library (for example
  markdown-it with a `==mark==` plugin) and styles it with CSS.

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
