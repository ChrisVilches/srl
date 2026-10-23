# Frontends

A **frontend** is a program that shows cards to a person and records the
results. `srl` has none built in (see [rationale.md](rationale.md), "Why no
built-in frontend"): every frontend is a separate program, such as a web
viewer, a shell-prompt question or a script. The repository's `examples/`
has two, not part of `srl` (see [overview.md](overview.md), "Examples"):
the web viewer `srl web` and the Bash frontend `srl quiz`. They are made to
be copied and changed.

Frontends understand the output of `srl next --json`, and nothing else.
They don't know about notes, kinds or cloze syntax, so any frontend can
show any card.

## The frontend contract

Every frontend runs the same loop:

1. **Get a card:** `srl next SELECTOR --json`. It prints the card, its
   state, and its item's note and context (see [cli.md](cli.md),
   "Studying"); exit 1 means nothing is left.
2. **Show the prompt**, and wait until the person wants to see the answer.
3. **Show the answer**, then the context, if it is not `null` (see
   [notes.md](notes.md), "Context"). The note it comes from is `see`, or
   `path` when `see` is `null`; a frontend may name it.
4. **Get a rating** from 1 to 4.
5. **Record it:** the card ID and the rating to `srl review --stdin`.

The person rates every card; nothing is judged automatically. A frontend may
also collect an answer before step 3 and pass it as `response`, which is
only kept in the log.

A frontend may offer more, with the commands everyone has: suspending the
card (`srl suspend id:ID`), opening its note (`srl edit ID`, or the file at
`path` in an editor of its own), or a summary at the end, counting the
cards whose `state` was `new`.

A frontend never computes schedules. That belongs to the core, so every
frontend behaves the same. Since `srl next` reads the notes and the log
every time, edits and reviews made elsewhere count at once.

## Rendering

Card text and context are Markdown; card text may also hold `==highlight==`
(see [cards.md](cards.md)).

- How highlighted text looks is a frontend setting, never card content.
  `srl quiz` uses bold yellow; `srl web` uses CSS.
- Printing the raw text must always remain acceptable. A simple frontend can
  skip rendering entirely.

## The web viewer: `srl web`

The web viewer is an example, not part of `srl`: an npm package in the
repository's `examples/web-frontend/`, which puts `srl-web` on `PATH`, so
that `srl web [SELECTOR]` starts it. Its README says how to install and
change it. It is a frontend like any other, split in two:

```
browser (React page)                  srl-web (Node.js, on localhost)    srl
────────────────────                  ───────────────────────────────    ───
render Markdown + ==mark==   ◄─JSON─  GET  /api/next?sel=…  ──runs──►  srl next SEL --json
buttons, keys, CSS theme     ──JSON─► POST /api/review      ──runs──►  srl review --stdin
                             ──JSON─► POST /api/suspend     ──runs──►  srl suspend id:CARD
```

- **The server contains no SRS logic.** Each route runs one CLI command and
  returns its JSON; when `srl` fails, it returns `srl`'s message, which the
  page shows. `--dir DIR` is passed to every command; without it, `srl`
  finds the store as always.
- **The page runs the frontend loop** of "The frontend contract" above.
  Space or Enter shows the answer, `1`–`4` rate, `s` suspends. The
  selector can be changed in the page, and the cards reviewed, the new ones
  among them and the Again ratings are counted at the top.
- **Card text and context are rendered as Markdown** with markdown-it. Raw
  HTML is shown as text. Highlights are paired in order, each `==` with the
  next one, as `srl quiz` does: Markdown plugins that treat `==`
  like `*` nest the marks in text without spaces, such as
  `==[...]==を==[...]==`.
- **Only the browser on the same computer can use it.** The server listens
  on `127.0.0.1`, and refuses requests for any host but localhost and POST
  bodies that are not JSON, so that other web pages can't record reviews.

Because scheduling lives in the core, the web viewer and every other
frontend behave identically.

## Small examples

A question in the shell prompt, rated later:

```sh
# in .zshrc: show one due card before each prompt, if any
precmd() { srl next --json 2>/dev/null | jq -r '"srl: " + .prompt' }
```

A frontend in a few lines of shell:

```sh
while card=$(srl next "$1" --json); do
  jq -r .prompt <<<"$card"; read -r _
  jq -r '.answer, "---", .context // empty' <<<"$card"; read -r -p 'rating 1-4: ' r
  jq -n --argjson card "$card" --argjson rating "$r" '{card: $card.id, rating: $rating}' | srl review --stdin
done
```

A fuller one, with keys that act without Enter, colors, and the context
written to a file for another program to show, is in the repository's
`examples/bash-frontend/` (see [overview.md](overview.md), "Examples").
