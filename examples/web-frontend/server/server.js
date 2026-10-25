// The local server: a JSON API over the srl CLI, and the built frontend.
//
// Every API route but /api/config runs srl commands. There is no SRS logic
// here: scheduling belongs to srl (see docs/frontends.md).
//
//   GET  /api/config       nothing                     → { selector }
//   GET  /api/next?sel=…   srl next SEL --json --context CONTEXT
//                                                      → { card } or { card: null }
//   POST /api/review       srl review --stdin          ← { card, rating, response? }
//   POST /api/suspend      srl suspend id:CARD         ← { card }
//
// The card browser, which shows any card without studying it:
//
//   GET  /api/find?sel=…   srl find SEL --json, then srl show ITEMS --json
//                          --context none for each item's kind and first field
//                                                      → { cards: [found…] }
//   GET  /api/card?id=…    srl cards ID --json and srl show ID --json
//                          --context CONTEXT           → { card }
//
// /api/card makes the card, so a kind program runs and may take a while (an
// LLM, say). When the page stops waiting, srl is killed.
//
// Errors are { error: MESSAGE } with status 400 (bad request) or 500 (srl
// failed), MESSAGE being srl's own message when it comes from srl.

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';

import { SrlError, runSrl } from './srl.js';

// How much of the note to show after the answer: 'prose' (the note without its
// items), 'full' (with them) or 'none' (see docs/notes.md, "Context").
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

class BadRequest extends Error {}

// Creates the server. DIR is the store passed to srl as --dir (or
// undefined, to let srl find it), SELECTOR the selector the page starts
// with, and STATIC_DIR the built frontend, or undefined to serve the API
// only (as in development, where Vite serves the frontend).
export function createSrlServer({ dir, selector, staticDir }) {
  const srl = (args, input, signal) => runSrl(args, { dir, input, signal });

  const routes = {
    'GET /api/config': async () => ({ selector }),

    'GET /api/next': async (url) => {
      const { found, stdout } = await srl(['next', '--json', '--context', CONTEXT, ...selectorArgs(url)]);
      return { card: found ? JSON.parse(stdout) : null };
    },

    // The body is passed to srl as it is: srl checks it.
    'POST /api/review': async (_url, body) => {
      await srl(['review', '--stdin'], JSON.stringify(body));
      return {};
    },

    'POST /api/suspend': async (_url, body) => {
      await srl(['suspend', `id:${requireId(body.card)}`]);
      return {};
    },

    // Each card as srl find --json gives it, with its item's kind and the
    // first line of its first field, as srl find prints without --json.
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
          const first = Object.values(item.fields)[0] ?? '';
          return { ...card, kind: item.kind, excerpt: first.split('\n')[0] };
        }),
      };
    },

    // The card as srl cards --json makes it, with what srl next --json gives
    // around it but the state, from its item.
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
      return { card: { ...card, kind: item.kind, path: item.path, see: item.see, context: item.context } };
    },
  };

  return createServer(async (req, res) => {
    const url = new URL(req.url, 'http://localhost');
    // Aborted when the page stops waiting before the answer is sent.
    const abort = new AbortController();
    res.on('close', () => {
      if (!res.writableFinished) abort.abort();
    });
    try {
      checkHost(req);
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

// Card IDs are lowercase letters, digits and `-` (docs/cards.md).
function requireId(id) {
  if (typeof id !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(id)) {
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
