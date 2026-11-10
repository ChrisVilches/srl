# A web viewer

`srl-web` is a web viewer, a frontend in the sense of
[docs/frontends.md](../../docs/frontends.md): a small server on localhost
that runs `srl` commands, and a page in the browser that shows the cards
to study, a browser of any card, and a form that adds items. It
needs `srl` on `PATH` and Node.js 22.22 or later (for React Router and
Vite), and should work on Linux,
macOS and Windows (it is tried on Linux only).

- Card text and context are rendered as CommonMark plus GFM tables, with
  `==highlights==` marked. Raw HTML in them is shown as text.
- Showing the answer turns the question into its revealed prompt, with a
  cloze's blanks filled in (or leaves it as it is when the card has none,
  `prompt_revealed` being `null`), and adds the answer below it when there
  is one.
- Each card is labelled with its kind in violet when the kind is
  a program, such as one that asks an LLM, and `new` when it is new. A
  built-in kind (only `qa` for now) is shown only in the card's details
  (below): srl doesn't say which kinds are built in, so the page
  lists them in `BUILT_IN_KINDS` in `src/CardParts.tsx`, and a new one
  must be added there. A card that isn't new is always due, so it gets
  no label for that, unless it is a leech: then it is labelled `due` and,
  in amber, `leech`, as its question likely needs rewriting.
- The card's ID is not on the card. **Details** (`i`), at the left of its
  header, opens its details in a modal: everything srl says about it, to
  trace it back to where it comes from and what happened to it. Its card
  and item IDs, its kind (a built-in one too), its source, the name
  of its collection, the note its context comes from, its status (with the
  reason, if it is suspended), its place in the schedule, the day it is
  due, whether it is a leech, and its history: every event of the log
  for it, newest first, each with its time, as `srl log` gives them (a
  review's rating and the hash of its question). They are `src/CardDetails.tsx`, which Browse's
  Details side shows too. The modal is as tall as the window, so it
  doesn't move as the card, then its history, come in.
- Showing the answer scrolls smoothly to put it in the middle of the
  window, or, when it is taller than the window, its top at the top of
  the window, so its start shows. Browse doesn't: turning a card to its answer there leaves the
  page where it is.
- After the answer it shows the context, the note the item names with
  `see`, under the note's path. A note in anything but Markdown (by the
  extension of `see`), such as a text file, is shown as a code block, a
  JSON or JSONL one in colors. A note srl gives no text for, such as a
  PDF or an image, which isn't UTF-8, is shown by its path alone, saying
  there is no text to show. An item without `see` has no context. A table
  wider than the page scrolls sideways in its own box, leaving the text
  around it in place. A long kind is cut short in the card's header, a
  long collection over the context and a long value in the details, whole
  in its tooltip.
- Keys: space (or Enter) shows the answer, `1`–`4` rate, `i` shows the
  card's details, `s` suspends the
  card without recording a review, after asking why in a dialog: what to
  fix before studying it again. The reason may be left empty, or take
  several lines, kept as they are: Enter starts a new line, Escape
  cancels, and only a click on the dialog's Suspend suspends. Buttons name
  their key, as **Make again** `r` does, everywhere on the page, but not
  on a touch screen (a coarse primary pointer, as on a phone), which has
  no keys to press.
- When the next card takes a while to make, as when its kind program asks
  an LLM, a spinner says so, with the seconds it has taken. Cards that
  come in under 300 ms replace the last one without it. Everything else
  the page waits for, such as the cards of Browse, an item to edit or what
  Add needs, is shown as a skeleton, grey bars laid out as it will be, in
  place of a text saying it is loading.
- The selector can be changed in the page, and the X at the right of its
  box, shown while the box holds text, empties it and applies no
  selector, every card; the counters at the top are the
  cards reviewed today, the new ones among them, and the Again ratings,
  of every card whatever the selector, as `srl stats` counts them: read
  when Study is shown and again after each review (see "What srl
  answers"), so a reload keeps them and they include reviews recorded
  elsewhere. The
  page has one selector, the same in Study and Browse: submitting it in
  either changes it in both, as the other view finds when it is shown.
  The selector
  submitted is kept in the address's query, one parameter per term:
  `key:value` is `key=value`, text (quoted or not) is `text=…`, and a
  negated term puts its `-` before the name, so `in:a,b -kind:qa "two
  words"` is `?in=a,b&-kind=qa&text=two%20words`, and `in:a in:b` is
  `?in=a&in=b`. Reloading the page rebuilds the selector from the
  parameters, in their order, whichever view the address shows, and an
  address with a selector can be bookmarked or written by hand; without
  one, the page starts with no selector, every card. The address is the
  only place a selector comes from: `srl-web` takes none.
- Each view has its own path: `/` for Study, `/browse` and `/add`. The
  address is the only place the view, the selector and Browse's card and
  side are kept: the page shows whatever address it is on, and changing
  any of them goes to another address. So back and forward go through
  them all, and the page follows: coming back to another selector applies
  it as if submitted, so the box shows it and the cards are found or
  asked for again. Switching views with the tabs, submitting another
  selector (or emptying the box with its X), choosing a card in Browse
  and turning it to another side each add to the browser's history. Doing again what the address already shows, such as
  submitting its selector or clicking the side shown, adds nothing. A
  tab goes to its view's path, keeping the selector: Browse's tab goes
  to the list, with no card chosen. A view starts anew each time it is
  shown, and what it had is gone once it is left (see "Routing the
  usual way"); leaving with text typed and not saved, in Add or in
  Browse's Edit, asks first, in a dialog, whether to leave and lose it,
  and reloading or closing the page then asks too, in the browser's own
  dialog. Leaving Study while `srl next` makes a card, or Browse while a
  card is made, stops `srl` and the kind program making it. The name
  srl at the left of the top bar links to the root, loading the page
  anew on the study view with the selector cleared.
- Dark, or light with the button at the right of the top bar, which
  shows the theme it switches to: a sun in dark, a moon in light. The choice
  is kept in the browser's localStorage, not taken from the system's
  setting, and is applied before the page is drawn, so it doesn't flash
  the other theme.

It also has a **card browser** (the Browse tab, or `/browse` in the
address), for seeing what a store holds and how a kind's cards look, such
as how `ja_word` marks words, without studying:

- It lists the cards of a selector as `srl find` does, in the order they
  are written and under the name of each collection: each card's ID,
  kind (unless built in), status (when not active), place in the schedule
  and whether it is a leech, and the
  first line of its item's first field.
- Choosing a card makes it with `srl cards`, running its kind program, and
  shows it as studying does: the prompt, or, turned, the revealed prompt,
  the answer and the context. The JSON side shows the card as `srl cards
  --json` prints it, in colors.
- The **Details** side, after Edit, shows the card's details, the same
  as Study's modal, and is the only side that shows them. A suspended
  card says so at their top, with the reason if it was given one, so
  `is:suspended` lists the cards to fix, and what to fix. They come from
  the list, so they show at once; only the history is read, with `srl
  log`, once a card has stayed chosen for a moment, and again whenever
  the cards are found again.
- A kind program can take a while: the page counts the seconds while it
  runs. A card is made once it has stayed chosen
  for a moment, so moving through the list makes no card on the way, and
  choosing another card stops the one being made (see "How it works").
  It is made only on a side that shows it, Prompt, Revealed or JSON:
  on Edit or Details, choosing a card or saving its item makes nothing
  until another side is shown.
- Each card is made once and kept while Browse is shown; **Make again**
  (`r`) runs its kind program again once the card has been made, on
  Prompt, Revealed and
  JSON only, since Edit shows the item and Details what srl says about
  the card, not the card made from it. A card made stays made when
  another is chosen and when Browse is left and shown again, and a
  change to the store doesn't make it again; submitting the selector
  shown again forgets every card made, as it finds the cards again,
  while submitting another one keeps them, so a card made before shows
  at once. Saving an item forgets its cards, and
  choosing another card or leaving Browse stops the one being made.
- Keys: ↑/↓ (or `k`/`j`) choose a card. Turning it is done with the
  mouse only, so that space and Enter stay free for typing.
- The chosen card and side are kept in the address, as
  `/browse/CARD/SIDE`, the side left out for Prompt: `/browse/x7k2m9`,
  `/browse/x7k2m9/edit`, `/browse/x7k2m9/details`, before the selector's query if there is one. Reloading
  the page opens the same card on the same side, and scrolls the list to
  it once the cards are found. Back turns the card back to the side it
  was on before, and back to the card it was on before another selector
  was submitted. The list always scrolls the chosen card to its
  middle, whether after a reload or when choosing another with the keys. A card that isn't among the selector's cards,
  such as one whose item was deleted, is said so in place of the card,
  and an unknown side is Prompt.
- On a narrow screen, such as a phone's, the list and the card don't fit
  side by side: the list takes the page until a card is chosen, and the
  card then takes it, with a **← Cards** button back to the list, which
  is as it was left, scrolled where it was. The button leaves the card,
  going to `/browse` with the selector as Browse's tab does, so the address always says whether the list or a
  card is shown. Which of the two layouts shows is up to the page's width only,
  so widening or narrowing the window moves between them as it goes.
- It records nothing: no review, no suspension. Studying starts only when
  Study is shown, so a page opened on `/browse` runs no `srl next`.

In the card browser, the **Edit** side, next to Prompt, Revealed and
JSON, edits the item of the chosen card, whatever its source: a file of
the store, or a database or service behind a source program. It shows the item's
fields and whether it is a draft, as `srl show --json` gives them, each
field under its label from `srl kinds` (`Question` for `Q`), or its name
if its kind gives none or `srl kinds` fails, and **Save** gives the whole item to `srl edit
--stdin`; `srl` finds the item by its ID, and its source writes it. The
cards are then found again in place: the list stays where it is scrolled,
its rows change where the item's cards changed, and only the item's
cards are forgotten, to be made anew when shown. The page stays on Edit,
and says whether it worked in the same modal as Add: **Saved** with the
item's ID, srl's error if it didn't save the item, or srl's error if it
saved it but couldn't find the cards again. Reset puts back the fields
and Draft as srl has the item, as read or as last saved, and stays on
Edit. Choosing another card, turning to another side or leaving Browse
while the fields or Draft differ from the item as srl has it asks
first, in a dialog, whether to leave and lose the edits; submitting
another selector doesn't, since the form stays.

It also **adds items** (the Add tab, or `/add`), one at a time:

- The kind is chosen among those `srl kinds` describes, and the form
  shows one box per field, under the field's label (`Question` and
  `Answer` for `qa`), or its name if its kind gives none, with the
  kind's description of it as the
  box's placeholder: shown while the box is empty, gone once something
  is typed, since a kind is soon learned. Changing
  the kind keeps the text of a field of the same name.
- The **Collection** it is added to, of any source (a JSONL, JSON or
  XML file, or a table of a source program), is chosen among those `srl
  collections` lists, by typing any part of its name (the letters in
  order), or is a new one: a name typed in full, offered as **New
  collection** when exactly one source of the store could create it
  there, as `srl sources` describes (a name its `include` globs
  match and its `exclude` globs don't). `srl add` creates it, and has
  the last word on whether it may; the source writes the item its own
  way. Notes are never offered: they hold no items. The collections are
  read each time Add is shown.
- The collection last added to is chosen again on the next visit. It is kept
  in the browser's `localStorage`, so it is per browser and per address
  (port), and forgotten when `srl collections` no longer lists it.
- An optional **Context**, a note chosen the same way among those `srl
  notes` lists (your study material, by default the store's Markdown and
  text files), becomes the item's `see`. Without one, the item has no
  context: the collection it is added to is never its context.
  **Draft** makes the item a draft.
- **Check** runs `srl add --dry-run` and shows the item as srl would
  add it: as it would be written in its collection, or the preview a source
  program gives; **Add** (Ctrl+Enter) adds it and shows its ID, then empties
  the fields. Leaving Add while a field of the kind holds text asks
  first, in a dialog, whether to leave and lose the item. The page knows
  no rule of any kind: srl checks the item, and the page shows what srl
  says, such as a field the kind needs. Fields
  left blank are left out of the item, and `kind` is left out for `qa`.
  While srl works, and while its answer is shown, both buttons are
  disabled, with no other sign; Save and Reset in Edit too.
- What Check and Add give, the item, its ID or srl's error, is shown in
  a modal, `src/Modal.tsx` as every dialog of the page. It opens with its
  Close button focused, so Enter, space or Escape closes it and gives the
  focus back to the form, to write the next item.

It is an npm package of its own, kept apart from `srl` and from the other
examples: everything it needs is in this folder.

## Using it

```sh
npm install
npm run build
npm link          # puts srl-web on PATH
srl-web
```

`srl-web [--port N] [--dir DIR] [--on-change PROGRAM] [--no-open]`
serves on `http://localhost:4317/` and opens the browser. To start on a
selector, open its address, such as
`http://localhost:4317/?in=grammar`, or bookmark it. `--dir` is given
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
src/                the frontend: React, React Router, TanStack Query, Tailwind CSS, markdown-it, Heroicons
```

Icons come from Heroicons (`@heroicons/react`): a small set of general UI
icons, with no brand icons, made for Tailwind. Each icon is a module of
its own, so the build keeps only those imported. Import from
`@heroicons/react/24/outline`, and give the icon `aria-hidden` and its
button an `aria-label`, since the icon alone names nothing to a screen
reader.

Each API route but `/api/config` and `/api/on-change` runs `srl` commands. There is no
scheduling here; that belongs to `srl`.

| Route | Runs | Answers |
|---|---|---|
| `GET /api/config` | nothing | `{"on_change": …}`, whether `srl-web` has `--on-change` |
| `GET /api/on-change` | nothing | `{"busy": …, "last": …}`: whether a change is still waiting for its run, and the last run, `{"seq": N, "error": …}` with `error` `null` on success, or `null` before any |
| `GET /api/next?sel=…` | `srl next SEL --json` | `{"card": …}`, what `srl` printed, or `null` when nothing is left |
| `POST /api/review` | `srl review CARD --rating=N`, for `{"card": CARD, "rating": N}`; `N` must be 1, 2, 3 or 4 | `{}` |
| `POST /api/suspend` | `srl suspend id:CARD --reason=REASON`, for `{"card": CARD, "reason": REASON}`; an empty or missing `reason` is none | `{}` |
| `GET /api/find?sel=…` | `srl find SEL --json`, then `srl show ITEM… --json --no-context` | `{"cards": […]}`: each card `srl find` printed, with its item's `see` and `excerpt`, the first line of its first field |
| `GET /api/log?id=…` | `srl log id:ID --json` | `{"events": […]}`: the card's events, in log order, as stored; those of other cards of an item of the same ID are left out |
| `GET /api/card?id=…` | `srl cards ID --json` and `srl show ID --json` | `{"card": …}`: the card `srl cards` printed, with its item's `kind`, `collection`, `see` and `context`, as `srl next --json` gives them (not its `source`) |
| `GET /api/kinds` | `srl kinds --json` | `{"kinds": […]}`, each kind as `srl kinds` printed it |
| `GET /api/sources` | `srl sources --json` | `{"sources": […]}`, each source as `srl sources` printed it |
| `GET /api/collections` | `srl collections --json` | `{"collections": [COLLECTION…]}` |
| `GET /api/notes` | `srl notes --json` | `{"notes": [PATH…]}` |
| `POST /api/add` | `srl add COLLECTION --stdin`, with `--dry-run` for `{"dry_run": true}`, for `{"collection": COLLECTION, "item": ITEM}`; `ITEM` is its one line | `{"ids": […]}`, or `{"text": …}`, what `srl add --dry-run` printed: the item as its source would add it, as written in its collection or a source program's preview |
| `GET /api/item?id=…` | `srl show ID --json --no-context` | `{"item": …}`, the item object `srl show` printed |
| `POST /api/edit` | `srl edit --stdin`, for `{"item": ITEM}`; `ITEM` is its one line | `{"ids": […]}` |

`/api/find` gives `srl show` at most 500 item IDs at a time, to keep its
command line short enough for Windows. When the page stops waiting for
`/api/next` or `/api/card`, the server kills `srl`, which stops the kind
program making the card, with every process it started (see
[docs/cards.md](../../docs/cards.md), "Stopping a program"): an LLM's
answer no longer wanted stops being made.

When `srl` fails, the route answers with `{"error": MESSAGE}`, `MESSAGE`
being what `srl` printed, and the page shows it. Under every error about
the selector (a message with `invalid selector term`, which starts every
such error of `srl`), whatever the error, it shows the same guide to
writing a selector, `src/SelectorHelp.tsx`: the rules, and examples of
common selectors. The selector is never changed.

Every dialog of the page is `src/Modal.tsx`, the browser's own `<dialog>`.
Radix UI's dialog, say, would need the `Content-Security-Policy` below to
allow inline styles, since it adds `<style>` elements to the page. While a
dialog is open, the views take no keys. A dialog taller than the window
scrolls what it shows, keeping its title and Close button in view, and
the page under an open dialog doesn't scroll (`src/index.css`). Every
dialog fades in while
growing to its size, and out while shrinking, in 150 ms, by its class
`modal` in `src/index.css`, and not at all with reduced motion. The
fade-out needs a browser that can keep `display` during a transition,
such as Chrome; in Firefox the dialog still goes at once when closed.

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
item of the store, add items, and run kind programs, some of which may
ask an LLM, at your cost. The proxy must send `Host: localhost` (or `127.0.0.1`), or
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

npm runs it in this folder, so choose the store with `SRL_DIR`, or `srl`
uses the default store. `npm run build` checks the types and builds
`dist/`. `npm test` runs the tests of `test/` with Node's own test
runner: how the selector is written in the address's query and read back
from it.

### The server never blocks

The server is one Node process serving every request, so a synchronous
call there makes all of them wait for it. Once it is serving, all its
I/O is asynchronous: `spawn`, never `execSync` or `spawnSync`, and
`node:fs/promises` or streams, never `readFileSync`, `existsSync` or
another `…Sync` function. This covers the request handlers and anything
they call (`server/`), the `--on-change` runs, and timers.

Only the start, before the server listens, may be synchronous, as in
`bin/srl-web.js` checking with `existsSync` that the frontend is built:
nothing is waiting for it yet.

### Routing the usual way

Routing should feel the same everywhere and have nothing hacky about
it, so it is done the way React Router means it to be, even where that
costs something. The router is a data router (`createBrowserRouter` in
`src/main.tsx`). Each view is a route, and only the route shown is
mounted: leaving a view unmounts it, and coming back starts it anew, so
Study asks `srl next` again. That is accepted. Don't keep views mounted
while hidden, keep their state outside them so that it survives, or work
out from the address by hand what the router can give, such as a view's
own part of the path (use `useParams()`, as Browse does for its card and
side). The one exception is what the views read from srl, kept by
TanStack Query (below): Browse's cards, the cards it made and Add's lists
show at once when the view comes back.

What leaving a view would lose that can't be made again, text typed and
not yet saved, is guarded with `UnsavedGuard` (`src/Modal.tsx`, with the
router's `useBlocker`): going to another path while there is any asks
first, and so does reloading or closing the page, which the router never
sees, through the browser's `beforeunload`. Add uses it for the item
being written, and Edit for the item's edits. A new form that holds work
the user typed uses it too.

### What srl answers: by what was asked, through TanStack Query

What a view shows from srl must be the answer for what its address asks
now, whatever order srl's answers come in. So what the views read is read
with TanStack Query's `useQuery`, kept by what was asked: Browse's cards
by the selector submitted and the cards it made by their ID, Add's kinds,
collections, notes and sources, a card's details and history and an
item to edit by their ID, Study's counts, and the `--on-change`
program's status. A list that comes late for a selector submitted before
never replaces the one asked for. Every read is in `src/queries.ts`, its
key, what it asks and how, so that two views asking the same thing share
it (Add and Edit share the kinds). Read what a view shows the same way,
`useQuery(queries.X(…))`, rather than setting it from a request's answer
with `useState`, which shows whichever answer comes last.

What was read is kept, so a view shown again shows it at once, and is
read again when it may have changed (the client in `src/main.tsx`):

- Each change the page makes to the store (a review, a suspension, an
  item added or edited) makes everything read out of date. What is shown
  is read again at once, in place: Browse's list after an item is edited,
  Add's collections after an item made a new one, Study's counts after a
  review, the `--on-change` program's status, which is then read every
  second while the program is busy. The rest is read again when it is
  next shown. This is done in one place, for every change: a request that
  changes the store goes through `change()` in `src/api.ts`, never
  `call()`, and nothing else is needed.
- A card made by its kind program is the exception (`ONLY_WHEN_ASKED` in
  `src/queries.ts`): a program may take long or cost something, as an
  LLM does, so it is made again only when asked, never because the store
  changed.
- A change made elsewhere, with `srl` or an editor, is seen once what was
  read is 30 seconds old: when its view is next shown, or when the
  window gets the focus again, as when coming back from the editor.
- What no view has shown for 10 minutes is forgotten, and an error, which
  is srl's and the same when asked again, isn't retried.

Edit's form shows the item as read until it is edited, and from then on
the edits, kept apart from it, so that reading the item again never
changes what is being typed.

Not read this way: `srl next`. It isn't something to show again but the
next step of studying, which each review moves on: reading it again on
its own, when the window gets the focus or the store changes, would
replace the card being answered. Study asks for it once per card, and
stops a request no longer wanted.

### Where the page is: the address, through the router

Where the page is, what back should undo, is kept in the address and
nowhere else: the view, the selector, Browse's card and side. Anything
new of that kind, such as another view, a part of a view to go to, or
a filter, goes there too, in the path or the query (`src/address.ts`
lists them all). It follows one set of rules, so that every part works
the same way with back, forward, reload and bookmarks:

- **Read it from the router on every render.** Use `useParams()` or
  `useLocation()`, and derive the value from it, like the card and side
  in `src/Browse.tsx`. Don't copy it into `useState` or a ref to keep in
  step, and don't read `window.location`.
- **Change it only by going to another address.** Use `<Link>`, or
  `navigate()` from `useNavigate()`. Never call `window.history`, listen
  to `popstate`, or fire events of your own to make the page follow the
  address: the router does both.
- **Every change adds to the browser's history**, so back and forward
  step through everything shown, such as each card chosen in Browse,
  with ↑/↓ or the mouse. Going to the address already shown adds
  nothing: `<Link>` replaces by itself, and `navigate()` needs a check,
  as `setSelected` and `setSide` in `src/Browse.tsx` and `onSelector` in
  `src/App.tsx` do.
- **What follows from the address runs because the address changed**, not
  in the handler that changes it. A view loads its cards when the
  selector it derives changes (`useSubmitted` in `src/hooks.ts`), so
  back and forward load them as submitting does.
- **A new view is a route.** Add it to the routes of `src/main.tsx` and
  to `VIEW_PATHS` in `src/address.ts`, which its tab links to. It gets
  the selector from App with `useView()`.

What isn't where the page is stays in React state: data from srl, the
text of a form, the study session, an open dialog, the theme. Back
doesn't undo these, and they go with their view when it is left (see
"Routing the usual way").

After a change that touches any of this, try back and forward through
it, and reload, and check that the page shows what its address says.

### A red button

A button is red (`DANGER_BUTTON` in `src/CardParts.tsx`) when clicking
it loses something or takes a card out of study: Leave, when leaving
Add or Edit with unsaved text, and Suspend in the suspend dialog. A
button that only leads to one, such as Suspend in Study's bar, which
opens the dialog, keeps its own look.

A red button has no key: what it does is hard to undo, so it takes a
deliberate click, never a key pressed by habit.

### A button whose answer is a modal

A button that asks srl for something and shows the answer in a modal,
such as Add and Check in Add, or Save in Edit, follows one rule:

**The form's buttons stay disabled from the click until the answer's
modal is closed**, not only while srl works. Derive it from the state the
form already has: disabled while srl works *or* while there is an answer
to show, as `waiting` in `src/Add.tsx` and `saving.kind !== 'idle'` in
`src/EditItem.tsx`. Check the same condition wherever the action can
start besides the button, such as a key (Ctrl+Enter in Add).

Why: a disabled button is dimmed (`disabled:opacity-50`). If it were
enabled as soon as srl answers, it would turn bright for a moment, then
dim again under the modal's backdrop as it fades in: a flash. Kept
disabled, it stays dimmed under the backdrop and brightens once, as the
modal closes. It also keeps a second click from starting another request
while the answer is shown.

The rest comes with `src/Modal.tsx`: show the answer in `ResultBox`
(green when srl did it, red with srl's message when it didn't), and let
the modal give the focus back to the form when it closes.

## TODO

- In the card browser, Prompt, Revealed and JSON show nothing under the
  sides at first: while the cards are being found, and then for the
  moment (`CHOICE_DELAY` in `src/Browse.tsx`) before the card starts to be
  made. Only then does "Making the card…" show. Ideally something would
  show from the start, as Edit's form skeleton does, and lead into
  "Making the card…" without a blank, then a change. Not critical, since
  "Making the card…" does come soon. (The skeleton of a card shared by
  every side was tried and removed: it vanished, then "Making the card…"
  replaced it, which looked odd.)

- The collection and note pickers of the Add tab (`PathPicker` in
  `src/Add.tsx`) are written by hand, a plain `<input>` and a `<ul>` under it, and are not
  accessible: it has no `role="combobox"`, `listbox` or `option`, no
  `aria-expanded`, `aria-controls` or `aria-activedescendant`, so a
  screen reader announces neither the list nor the chosen row, and moving
  the chosen row with ↑/↓ doesn't scroll it into view.
