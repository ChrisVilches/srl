# A web viewer

`srl-web` is a web viewer, a frontend in the sense of
[docs/frontends.md](../../docs/frontends.md): a small server on localhost
that runs `srl` commands, and a page in the browser that shows the cards. It
needs `srl` on `PATH` and Node.js 20 or later, and should work on Linux,
macOS and Windows (it is tried on Linux only).

- Card text and context are rendered as CommonMark plus GFM tables, with
  `==highlights==` marked. Raw HTML in notes is shown as text.
- Showing the answer turns the question into its revealed prompt, with a
  cloze's blanks filled in (or leaves it as it is when the card has none,
  `prompt_revealed` being `null`), and adds the answer below it when there
  is one.
- After the answer it shows the context in `prose` mode, under the name of
  its note; `CONTEXT` in `server/server.js` chooses the mode.
- Keys: space (or Enter) shows the answer, `1`–`4` rate, `s` suspends the
  card without recording a review.
- The selector can be changed in the page; the counters at the top are the
  cards reviewed, the new ones among them, and the Again ratings.
- Dark or light, as the system is set.

It is an npm package of its own, kept apart from `srl` and from the other
examples: everything it needs is in this folder.

## Using it

```sh
npm install
npm run build
npm link          # puts srl-web on PATH
srl-web in:grammar
```

`srl-web [--port N] [--dir DIR] [--no-open] [SELECTOR]` serves on
`http://localhost:4317/` and opens the browser. `--dir` is given to every
`srl` command it runs; without it, `srl` finds the store as always (see
[docs/store.md](../../docs/store.md), "Finding the store"), starting from
the directory `srl-web` was started in.

## How it works

```
bin/srl-web.js      the command: options, starting the server, opening the browser
server/server.js    the server: the API below, and the built frontend from dist/
server/srl.js       running srl and reading its exit codes
src/                the frontend: React, Tailwind CSS, markdown-it
```

Each API route but `/api/config` runs one `srl` command. There is no
scheduling here; that belongs to `srl`.

| Route | Runs | Answers |
|---|---|---|
| `GET /api/config` | nothing | `{"selector": …}`, the selector given to `srl-web` |
| `GET /api/next?sel=…` | `srl next SEL --json --context prose` | `{"card": …}`, what `srl` printed, or `null` when nothing is left |
| `POST /api/review` | `srl review --stdin`, with the request body | `{}` |
| `POST /api/suspend` | `srl suspend id:CARD`, for `{"card": CARD}` | `{}` |

When `srl` fails, the route answers with `{"error": MESSAGE}`, `MESSAGE`
being what `srl` printed, and the page shows it.

The server listens on `127.0.0.1` only. It refuses requests whose `Host` is
not localhost, and POST bodies that are not `application/json`, so that
other web pages open in the browser can't use it.

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
