// The local server: a JSON API over the srl CLI, and the built frontend.
//
// Every API route but /api/config and /api/on-change runs srl commands. There is no SRS logic
// here: scheduling belongs to srl (see docs/frontends.md).
//
//   GET  /api/config       nothing                     → { on_change }
//   GET  /api/next?sel=…   srl next SEL --json --context CONTEXT
//                                                      → { card } or { card: null }
//   POST /api/review       srl review --stdin          ← { card, rating, response? }
//   POST /api/suspend      srl suspend id:CARD --reason REASON
//                                                      ← { card, reason? }
//
// The card browser, which shows any card without studying it:
//
//   GET  /api/find?sel=…   srl find SEL --json, then srl show ITEMS --json
//                          --context none for each item's kind and first field
//                                                      → { cards: [found…] }
//   GET  /api/card?id=…    srl cards ID --json and srl show ID --json
//                          --context CONTEXT           → { card }
//   GET  /api/log?id=…     srl log id:ID --json        → { events: [event…] }
//
// /api/card makes the card, so a kind program runs and may take a while (an
// LLM, say). When the page stops waiting, srl is killed.
//
// Adding items, one at a time:
//
//   GET  /api/kinds        srl kinds --json            → { kinds: [kind…] }
//   GET  /api/collections  srl collections --json      → { collections: [address…] }
//   GET  /api/sources      srl sources --json          → { sources: [source…] }
//   POST /api/add          srl add ADDRESS --stdin [--dry-run], with ITEM as its
//                          one line                    ← { address, item, dry_run }
//                                                      → { ids } or, with
//                                                        dry_run, { text }
//
// Editing an item, of any source:
//
//   GET  /api/item?id=ID   srl show ID --json --context none → { item }
//   POST /api/edit         srl edit --stdin, with ITEM as its one line
//                                                      ← { item } → { ids }
//
// With --on-change, the program runs after each change srl-web makes to the
// store (review, suspend, add without dry_run; see on-change.js):
//
//   GET  /api/on-change    nothing                     → { busy, last }: last
//                                                        is { seq, error } or
//                                                        null before any run
//
// Errors are { error: MESSAGE } with status 400 (bad request) or 500 (srl
// failed), MESSAGE being srl's own message when it comes from srl.

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';

import { createOnChange } from './on-change.js';
import { SrlError, runSrl } from './srl.js';

// How much of the context to show after the answer: 'prose' (the collection
// without its items), 'full' (with them) or 'none' (see docs/items.md, "Context").
const CONTEXT = 'prose';

// How many item IDs to give one srl show, so that its command line stays
// short enough everywhere (Windows allows about 32,000 characters).
const SHOW_BATCH = 500;

const CONTENT_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.json': 'application/json',
  '.woff2': 'font/woff2',
};

// The longest POST body read, in characters: an item is far shorter.
const MAX_BODY = 1024 * 1024;

// Sent with every answer. The page runs only its own scripts and styles,
// can't be framed by another page (which could trick clicks into it), and
// sends no Referer when a link in a card or context is followed. Style
// attributes are allowed for the alignment of table columns. Images may
// come from anywhere, since an item's Markdown can show any.
const SECURITY_HEADERS = {
  'content-security-policy':
    "default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; " +
    "img-src * data:; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
  'x-content-type-options': 'nosniff',
  'x-frame-options': 'DENY',
  'referrer-policy': 'no-referrer',
};

class BadRequest extends Error {}

// Creates the server. DIR is the store passed to srl as --dir (or
// undefined, to let srl find it), STATIC_DIR the built frontend, or
// undefined to serve the API only (as in development, where Vite serves the
// frontend), and ON_CHANGE the program to run after each change, or
// undefined. The selector is the page's own, kept in its address.
export function createSrlServer({ dir, staticDir, onChange }) {
  const srl = (args, input, signal) => runSrl(args, { dir, input, signal });
  const hook = onChange ? createOnChange(onChange, dir) : null;
  // Called once srl has made a change.
  const changed = () => hook?.changed();

  const routes = {
    'GET /api/config': async () => ({ on_change: hook !== null }),

    'GET /api/on-change': async () => hook?.status() ?? { busy: false, last: null },

    'GET /api/next': async (url) => {
      const { found, stdout } = await srl(['next', '--json', '--context', CONTEXT, ...selectorArgs(url)]);
      return { card: found ? JSON.parse(stdout) : null };
    },

    // The body is passed to srl as it is: srl checks it.
    'POST /api/review': async (_url, body) => {
      await srl(['review', '--stdin'], JSON.stringify(body));
      changed();
      return {};
    },

    // An empty or missing reason is no reason.
    'POST /api/suspend': async (_url, body) => {
      const reason = body.reason ?? '';
      if (typeof reason !== 'string') {
        throw new BadRequest('reason must be text');
      }
      await srl(['suspend', `id:${requireId(body.card)}`, `--reason=${reason}`]);
      changed();
      return {};
    },

    // Each card as srl find --json gives it, with its item's kind and see,
    // and the first line of its first field, as srl find prints without --json.
    'GET /api/find': async (url) => {
      const { found, stdout } = await srl(['find', '--json', ...selectorArgs(url)]);
      const cards = found ? jsonLines(stdout) : [];
      const items = new Map();
      const itemIds = [...new Set(cards.map((card) => card.item))];
      for (let i = 0; i < itemIds.length; i += SHOW_BATCH) {
        const show = await srl(['show', '--json', '--context', 'none', ...itemIds.slice(i, i + SHOW_BATCH)]);
        for (const item of jsonLines(show.stdout)) {
          items.set(item.id, item);
        }
      }
      return {
        cards: cards.map((card) => {
          const item = items.get(card.item);
          // Field names are the item object's uppercase keys.
          const first = Object.entries(item).find(([key]) => /^[A-Z]/.test(key))?.[1] ?? '';
          return { ...card, kind: item.kind, see: item.see, excerpt: first.split('\n')[0] };
        }),
      };
    },

    // The card as srl cards --json makes it, with what srl next --json gives
    // around it but its schedule and leech, from its item.
    'GET /api/card': async (url, _body, signal) => {
      const id = requireId(url.searchParams.get('id'));
      const [cards, show] = await Promise.all([
        srl(['cards', '--json', id], undefined, signal),
        srl(['show', '--json', '--context', CONTEXT, id], undefined, signal),
      ]);
      const card = jsonLines(cards.stdout).find((c) => c.id === id);
      if (!card) {
        throw new BadRequest(`${id} is not a card ID`);
      }
      const item = jsonLines(show.stdout)[0];
      return {
        card: {
          ...card,
          kind: item.kind,
          address: item.address,
          see: item.see,
          context: item.context,
          context_address: item.context_address,
        },
      };
    },

    // The card's log events, in log order. id: also matches the cards of an
    // item of that ID, so only the card's own are kept: those naming it, and
    // the moves of history to it.
    'GET /api/log': async (url, _body, signal) => {
      const id = requireId(url.searchParams.get('id'));
      const { found, stdout } = await srl(['log', '--json', '--', `id:${id}`], undefined, signal);
      return { events: found ? jsonLines(stdout).filter((event) => event.card === id || event.to === id) : [] };
    },

    'GET /api/kinds': async () => ({ kinds: jsonLines((await srl(['kinds', '--json'])).stdout) }),

    'GET /api/sources': async () => ({ sources: jsonLines((await srl(['sources', '--json'])).stdout) }),

    'GET /api/collections': async () => {
      const { found, stdout } = await srl(['collections', '--json']);
      return { collections: found ? jsonLines(stdout).map((collection) => collection.address) : [] };
    },

    // The item is passed to srl as it is: srl checks it, and the address of
    // the collection, given as `address`. With dry_run, srl prints the item
    // as its source would add it, as written in a note or a source program's
    // preview of it, and adds nothing.
    'POST /api/add': async (_url, body) => {
      if (typeof body.address !== 'string' || body.address === '') {
        throw new BadRequest('address must be the address of a collection');
      }
      if (body.item === null || typeof body.item !== 'object' || Array.isArray(body.item)) {
        throw new BadRequest('item must be a JSON object');
      }
      const dryRun = body.dry_run === true;
      // The address goes after `--`, so that one starting with `-` is never
      // read as an option.
      const args = ['add', '--stdin', ...(dryRun ? ['--dry-run'] : []), '--', body.address];
      const { stdout } = await srl(args, JSON.stringify(body.item) + '\n');
      if (!dryRun) {
        changed();
      }
      return dryRun ? { text: stdout } : { ids: stdout.split('\n').filter((line) => line.trim() !== '') };
    },

    // The item of a card or item ID, as srl show --json prints it, to edit.
    'GET /api/item': async (url) => {
      const id = requireId(url.searchParams.get('id'));
      const { stdout } = await srl(['show', '--json', '--context', 'none', id]);
      return { item: jsonLines(stdout)[0] };
    },

    // The item, given whole, replaces the item of its id: srl checks it,
    // and its source writes it, whatever source that is.
    'POST /api/edit': async (_url, body) => {
      if (body.item === null || typeof body.item !== 'object' || Array.isArray(body.item)) {
        throw new BadRequest('item must be a JSON object');
      }
      const { stdout } = await srl(['edit', '--stdin'], JSON.stringify(body.item) + '\n');
      changed();
      return { ids: stdout.split('\n').filter((line) => line.trim() !== '') };
    },
  };

  return createServer(async (req, res) => {
    const url = new URL(req.url, 'http://localhost');
    // Aborted when the page stops waiting before the answer is sent.
    const abort = new AbortController();
    res.on('close', () => {
      if (!res.writableFinished) abort.abort();
    });
    for (const [name, value] of Object.entries(SECURITY_HEADERS)) {
      res.setHeader(name, value);
    }
    try {
      checkHost(req);
      checkFetchSite(req);
      if (url.pathname.startsWith('/api/')) {
        const route = routes[`${req.method} ${url.pathname}`];
        if (!route) {
          throw new BadRequest(`no route ${req.method} ${url.pathname}`);
        }
        const body = req.method === 'POST' ? await readJson(req) : undefined;
        sendJson(res, 200, await route(url, body, abort.signal));
      } else if (staticDir && req.method === 'GET') {
        await sendStatic(res, staticDir, url.pathname);
      } else {
        sendJson(res, 404, { error: 'not found' });
      }
    } catch (error) {
      if (abort.signal.aborted) {
        // Nobody is waiting for the answer.
      } else if (error instanceof BadRequest) {
        sendJson(res, 400, { error: error.message });
      } else if (error instanceof SrlError) {
        sendJson(res, 500, { error: error.message });
      } else {
        console.error(error);
        sendJson(res, 500, { error: `srl-web: ${error.message}` });
      }
    }
  });
}

// The selector goes after `--`, so that a term starting with `-` (a
// negation) is never read as an option. srl joins arguments with spaces, so
// the whole selector can be one argument.
function selectorArgs(url) {
  const selector = (url.searchParams.get('sel') ?? '').trim();
  return selector === '' ? [] : ['--', selector];
}

// srl prints JSON objects one per line.
function jsonLines(text) {
  return text
    .split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => JSON.parse(line));
}

// Card IDs are lowercase letters, digits, `-` and `.` (docs/cards.md).
function requireId(id) {
  if (typeof id !== 'string' || !/^[a-z0-9][a-z0-9.-]*$/.test(id)) {
    throw new BadRequest(`not a card ID: ${JSON.stringify(id)}`);
  }
  return id;
}

// Only pages served from localhost may use the API: a Host header naming
// another domain means a page from that domain resolved its name to us (DNS
// rebinding).
function checkHost(req) {
  const host = (req.headers.host ?? '').replace(/:\d+$/, '');
  if (!['localhost', '127.0.0.1', '[::1]'].includes(host)) {
    throw new BadRequest(`refusing requests for host ${JSON.stringify(host)}`);
  }
}

// Only the page itself may use the API: browsers say in Sec-Fetch-Site where
// a request comes from, and a request from another site is refused even
// when the browser would send it with credentials, such as the basic auth of
// a reverse proxy in front of this server. A GET /api/card from another
// page, an <img> say, would otherwise run a kind program. Requests without
// the header (curl, older browsers) are let through: the other checks still
// apply to them.
function checkFetchSite(req) {
  const site = req.headers['sec-fetch-site'];
  if (site !== undefined && site !== 'same-origin' && site !== 'none') {
    throw new BadRequest(`refusing a ${site} request`);
  }
}

// Reads a JSON object from the request. Requiring the JSON content type
// keeps other web pages from posting here: a cross-site request can't set it
// without a CORS preflight, which this server never allows.
async function readJson(req) {
  if (!(req.headers['content-type'] ?? '').startsWith('application/json')) {
    throw new BadRequest('POST bodies must be application/json');
  }
  let text = '';
  for await (const chunk of req.setEncoding('utf8')) {
    text += chunk;
    if (text.length > MAX_BODY) {
      throw new BadRequest(`POST bodies must be at most ${MAX_BODY} characters`);
    }
  }
  let body;
  try {
    body = JSON.parse(text);
  } catch {
    throw new BadRequest('the body is not valid JSON');
  }
  if (body === null || typeof body !== 'object' || Array.isArray(body)) {
    throw new BadRequest('the body must be a JSON object');
  }
  return body;
}

function sendJson(res, status, value) {
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
  });
  res.end(JSON.stringify(value));
}

// Serves a file of the built frontend; any path that isn't a file gets
// index.html, the single page.
async function sendStatic(res, staticDir, pathname) {
  const root = path.resolve(staticDir);
  let file = path.resolve(root, '.' + decodeURIComponent(pathname));
  if (!file.startsWith(root + path.sep) || !(await isFile(file))) {
    file = path.join(root, 'index.html');
  }
  const type = CONTENT_TYPES[path.extname(file)] ?? 'application/octet-stream';
  res.writeHead(200, { 'content-type': type });
  createReadStream(file).pipe(res);
}

async function isFile(file) {
  try {
    return (await stat(file)).isFile();
  } catch {
    return false;
  }
}
