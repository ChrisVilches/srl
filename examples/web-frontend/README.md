# A web viewer

`srl-web` is a web viewer, a frontend in the sense of
[docs/frontends.md](../../docs/frontends.md): a small server on localhost
that runs `srl` commands, and a page in the browser that shows the cards
to study, a browser of any card, and a form that adds items. It
needs `srl` on `PATH` and Node.js 20 or later, and should work on Linux,
macOS and Windows (it is tried on Linux only).

- Card text and context are rendered as CommonMark plus GFM tables, with
  `==highlights==` marked. Raw HTML in notes is shown as text.
- Showing the answer turns the question into its revealed prompt, with a
  cloze's blanks filled in (or leaves it as it is when the card has none,
  `prompt_revealed` being `null`), and adds the answer below it when there
  is one.
- Each card is labelled with its ID and kind, the kind in violet when it
  isn't `qa`, such as a kind that asks an LLM.
- After the answer it shows the context in `prose` mode, under the name of
  its note; `CONTEXT` in `server/server.js` chooses the mode. A context
  whose `context_format` isn't `markdown`, such as a JSONL note, is shown
  as a code block.
- Keys: space (or Enter) shows the answer, `1`–`4` rate, `s` suspends the
  card without recording a review.
- When the next card takes a while to make, as when its kind program asks
  an LLM, a spinner says so, with the seconds it has taken. Cards that
  come in under 300 ms replace the last one without it.
- The selector can be changed in the page; the counters at the top are the
  cards reviewed, the new ones among them, and the Again ratings.
- Dark, or light with the button at the right of the top bar. The choice
  is kept in the browser's localStorage, not taken from the system's
  setting, and is applied before the page is drawn, so it doesn't flash
  the other theme.

It also has a **card browser** (the Browse tab, or `#browse` in the
address), for seeing what a store holds and how a kind's cards look, such
as how `ja_word` marks words, without studying:

- It lists the cards of a selector as `srl find` does, in the order they
  are written and under the name of each note: each card's ID, line,
  kind, status (when not active) and state, and the first line of its
  item's first field.
- Choosing a card makes it with `srl cards`, running its kind program, and
  shows it as studying does: the prompt, or, turned, the revealed prompt,
  the answer and the context. The JSON side shows the card as `srl cards
  --json` prints it.
- A kind program can take a while: the page counts the seconds while it
  runs. A card is made once it has stayed chosen
  for a moment, so moving through the list makes no card on the way, and
  choosing another card stops the one being made (see "How it works").
- Each card is made once and kept; **Make again** (`r`) runs its kind
  program again, and submitting the selector forgets every card made.
- Keys: ↑/↓ (or `k`/`j`) choose a card, space (or Enter) turns it.
- It records nothing: no review, no suspension. Studying starts only when
  the Study tab is first shown, so a page opened on `#browse` runs no
  `srl next`.

It also **adds items** (the Add tab, or `#add`), one at a time:

- The kind is chosen among those `srl kinds` describes, and the form
  shows one box per field, with the kind's description of it. Changing
  the kind keeps the text of a field of the same name.
- The note it is appended to is chosen among the notes `srl notes`
  lists, by typing any part of its path (the letters in order); nothing
  else can be chosen, so the page never creates a note. To add to a new
  note, such as an inbox, create it, then **Reload notes**.
- The note last added to is chosen again on the next visit. It is kept
  in the browser's `localStorage`, so it is per browser and per address
  (port), and forgotten when `srl notes` no longer lists it.
- An optional context note, chosen the same way, becomes the item's
  `see:`; **Draft** makes it a draft.
- **Check** runs `srl add --dry-run` and shows the item as srl would
  write it; **Add** (Ctrl+Enter) adds it and shows its ID, then empties
  the fields. The page knows no rule of any kind: srl checks the item, and
  the page shows what srl says, such as a field the kind needs. Fields
  left blank are left out of the item, and `kind` is left out for `qa`.

It is an npm package of its own, kept apart from `srl` and from the other
examples: everything it needs is in this folder.

## Using it

```sh
npm install
npm run build
npm link          # puts srl-web on PATH
srl-web in:grammar
```

`srl-web [--port N] [--dir DIR] [--on-change PROGRAM] [--no-open] [SELECTOR]`
serves on `http://localhost:4317/` and opens the browser. `--dir` is given
to every `srl` command it runs; without it, `srl` finds the store as
always (see [docs/store.md](../../docs/store.md), "Finding the store"),
starting from the directory `srl-web` was started in.

### Running a program after changes

`--on-change PROGRAM` runs `PROGRAM` after the page changes the store: a
review, a suspension, an item added (not a check). It is meant for
keeping a store in sync, such as a script that commits it with git and
pushes it; without the option nothing runs.

- `PROGRAM` is run with no arguments, in the directory `srl-web` was
  started in, with `SRL_DIR` set to `--dir` when it is given. A name with
  no directory in it is looked for on `PATH`. On Windows it must be a
  program, such as an `.exe`, not a `.bat` or `.cmd` script.
- It succeeds when it exits with 0. Otherwise the page shows what it
  printed to stderr, as it is, in a dialog. The change itself is already
  in the store either way. What it prints to stdout goes to `srl-web`'s,
  and its errors to `srl-web`'s stderr too.
- Runs never overlap, and changes made close together get one run: a run
  starts a second after a change, and changes made while the program runs
  get one more run once it has ended, however many there were.
- After each change the page asks the server every second how the program
  is doing, until every change has had its run, and shows the last run's
  error if it failed. An error followed by a run that succeeded is not
  shown, and a new error replaces the one shown, so there is never more
  than one dialog.
- A run that takes more than 2 minutes, such as a `git push` waiting for a
  password, is killed and counted as failed.

## How it works

```
bin/srl-web.js      the command: options, starting the server, opening the browser
server/server.js    the server: the API below, and the built frontend from dist/
server/srl.js       running srl and reading its exit codes
server/on-change.js running the --on-change program
src/                the frontend: React, Tailwind CSS, markdown-it
```

Each API route but `/api/config` and `/api/on-change` runs `srl` commands. There is no
scheduling here; that belongs to `srl`.

| Route | Runs | Answers |
|---|---|---|
| `GET /api/config` | nothing | `{"selector": …, "on_change": …}`, the selector given to `srl-web`, and whether it has `--on-change` |
| `GET /api/on-change` | nothing | `{"busy": …, "last": …}`: whether a change is still waiting for its run, and the last run, `{"seq": N, "error": …}` with `error` `null` on success, or `null` before any |
| `GET /api/next?sel=…` | `srl next SEL --json --context prose` | `{"card": …}`, what `srl` printed, or `null` when nothing is left |
| `POST /api/review` | `srl review --stdin`, with the request body | `{}` |
| `POST /api/suspend` | `srl suspend id:CARD`, for `{"card": CARD}` | `{}` |
| `GET /api/find?sel=…` | `srl find SEL --json`, then `srl show ITEM… --json --context none` | `{"cards": […]}`: each card `srl find` printed, with its item's `kind` and `excerpt`, the first line of its first field |
| `GET /api/card?id=…` | `srl cards ID --json` and `srl show ID --json --context prose` | `{"card": …}`: the card `srl cards` printed, with its item's `kind`, `path`, `see` and `context`, as `srl next --json` gives them |
| `GET /api/kinds` | `srl kinds --json` | `{"kinds": […]}`, each kind as `srl kinds` printed it |
| `GET /api/notes` | `srl notes --json` | `{"notes": [PATH…]}` |
| `POST /api/add` | `srl add NOTE --stdin`, with `--dry-run` for `{"dry_run": true}`, for `{"note": NOTE, "item": ITEM}`; `ITEM` is its one line | `{"ids": […]}`, or `{"text": …}`, the item as it would be written, for a dry run |

`/api/find` gives `srl show` at most 500 item IDs at a time, to keep its
command line short enough for Windows. When the page stops waiting for
`/api/card`, the server kills `srl`; the kind program `srl` started is not
killed with it, and runs until it ends on its own (an LLM's answer, say,
is still paid for).

When `srl` fails, the route answers with `{"error": MESSAGE}`, `MESSAGE`
being what `srl` printed, and the page shows it.

Every dialog of the page is `src/Modal.tsx`, the browser's own `<dialog>`.
Radix UI's dialog, say, would need the `Content-Security-Policy` below to
allow inline styles, since it adds `<style>` elements to the page. While a
dialog is open, the views take no keys.

The server listens on `127.0.0.1` only. So that other web pages open in
the browser can't use it, it refuses requests whose `Host` is not
localhost, requests that the browser says (with `Sec-Fetch-Site`) come
from another site, and POST bodies that are not `application/json`. Every
answer carries a `Content-Security-Policy` that lets the page run only its
own scripts (which is why the theme is set by `public/theme.js` rather than
an inline script) and keeps other pages from framing it.

### Behind a reverse proxy

To reach it from elsewhere, put it behind a reverse proxy that adds HTTPS
and authentication, such as basic auth, and run `srl-web --no-open` on
the server. Anyone who gets through can do what the page does: read every
note, add items, and run kind programs, some of which may ask an LLM, at
your cost. The proxy must send `Host: localhost` (or `127.0.0.1`), or
srl-web refuses every request: nginx's `proxy_pass http://127.0.0.1:4317;`
does so by default, Caddy's `reverse_proxy` needs `header_up Host
localhost`. A card made by a slow kind program can take longer than a
proxy's default timeout (60 s in nginx: `proxy_read_timeout`).

Highlights are paired in order, each `==` with the next one, as `srl-tui`
does (`src/highlight.ts`). Plugins that treat `==` like `*`, with
CommonMark's rules, get text without spaces wrong: in `==[...]==を==[...]==`
they nest the marks.

## Changing it

```sh
npm run dev       # Vite with hot reload on :5173, the API on :4318
```

In development the selector is the arguments (`npm run dev -- in:grammar`).
npm runs it in this folder, so choose the store with `SRL_DIR`, or `srl`
uses the default store. `npm run build` checks the types and builds
`dist/`.
