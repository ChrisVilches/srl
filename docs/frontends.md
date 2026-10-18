# Frontends

A **frontend** is a program that shows cards to a person and records the
results. `srl` ships one, the terminal frontend `srl study`. Others are
separate programs: a web viewer, a shell-prompt question, a script.

Frontends understand the card format and nothing else. They don't know about
facets, drills or where a card came from, so any frontend can show any card,
including cards from drills written after it.

## The frontend contract

Every frontend runs the same loop:

1. **Get a card:** `srl next SELECTOR --json` (or the next card of a session).
2. **Show the prompt.**
3. **Collect the answer**, according to `grading`:
   - `self`: wait until the person wants to see the answer;
   - `exact`: one input per entry of `accept`;
   - `choice`: show `choices` and take one pick;
   - `cmd`: one free-text input.
4. **Grade it**, for `exact`, `choice` and `cmd`: send card and response to
   `srl grade`. If the result says `"fallback": "self"`, continue as for a
   `self` card.
5. **Show the result:** the answer, which blanks were right, the grader's
   feedback, and the explanation.
6. **Get a rating** from 1 to 4, with the suggested rating preselected.
7. **Record it:** card, rating and response to `srl review --stdin`.

A frontend never compares answers itself and never computes schedules. Both
belong to the core, so every frontend behaves the same.

## Rendering

Card text is Markdown with `==highlight==` (see [cards.md](cards.md)).

- How highlighted text looks is a frontend setting, never card content. The
  terminal frontend uses bold yellow; a web frontend uses CSS.
- Printing the raw text must always remain acceptable. A simple frontend can
  skip rendering entirely.

## The terminal frontend: `srl study`

`srl study [SELECTOR] [--drill NAME]` runs the loop above in the terminal until
nothing is left, then prints a short summary (reviews done, new atoms
introduced, Again count): `5 cards reviewed, 2 atoms new, 1 Again.` It reads
atoms and the log again before every card, so edits and reviews from other
frontends count at once. Like every interactive command, it refuses to run
when standard input is not a terminal.

Input is read a line at a time: type, then Enter.

- A `self` card: Enter shows the answer; `q` quits.
- An `exact` card: one line per blank, asked as `[1]`, `[2]`, …
- A `choice` card: the choices are numbered; type the number of one.
- A `cmd` card: one line.
- The rating: `1`–`4`. Enter alone takes the suggested rating, if there is
  one (there is none for `self` cards and fallbacks). `e` opens the atom in
  the editor (see [cli.md](cli.md), "Editing in the editor") and asks for
  the rating again. `s` suspends the atom without recording a review. `q`
  quits without recording this card.
- The end of input (Ctrl-D, or Ctrl-Z then Enter on Windows) quits at any
  point, without recording the card on screen.

Answers can be any text, including `q`, so commands are only read where no
answer is expected: before showing a `self` card's answer, and at the
rating.

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

`srl study --session NAME` is the same frontend reading cards from a session
file instead of `srl next` (see [cli.md](cli.md), "Sessions"). `--drill` and a selector can't be given with it.

## A web viewer

A web viewer is a frontend like any other, split in two:

```
browser (HTML/JS)                     srl-web (small local server)       srl
──────────────────                    ────────────────────────────       ───
render Markdown + ==mark==   ◄─JSON─  GET  /next?sel=…    ──runs──►  srl next SEL --json
inputs, buttons, CSS theme   ──JSON─► POST /grade         ──runs──►  srl grade
                             ──JSON─► POST /review        ──runs──►  srl review --stdin
```

- `srl-web` is a separate program on `PATH`, so `srl web` starts it. Each route
  runs one CLI command and returns its JSON. It contains no SRS logic.
- The browser renders Markdown with an ordinary library (for example
  markdown-it with a `==mark==` plugin) and styles it with CSS.
- Drills never produce HTML.

Because grading and scheduling live in the core, the web viewer and the
terminal behave identically.

## Specialized frontends

Nothing stops a frontend from handling only some cards in a richer way, for
example a kanji-drawing frontend that only studies `--drill writing` cards.
That is allowed, never required, and still goes through `srl next`,
`srl grade` and `srl review`.

## Small examples

A question in the shell prompt, rated later:

```sh
# in .zshrc: show one due card before each prompt, if any
precmd() { srl next --json 2>/dev/null | jq -r '"srl: " + .prompt' }
```

A frontend in a few lines of shell, self-grading only:

```sh
while card=$(srl next "$1" --json); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r .answer <<<"$card"; read -r -p 'rating 1-4: ' r
  jq -n --argjson card "$card" --argjson rating "$r" '{card: $card, rating: $rating}' | srl review --stdin
done
```
