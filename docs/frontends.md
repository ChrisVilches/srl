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
introduced, Again count).

Keys: Enter to reveal or submit, `1`–`4` to rate (Enter accepts the suggested
rating), `e` to open the atom in `$EDITOR`, `s` to suspend it, `q` to quit.

`srl play NAME` is the same frontend reading cards from a session file
instead of `srl next`.

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
# in .zshrc: show one due cue before each prompt, if any
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
