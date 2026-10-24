// The local server: a JSON API over the srl CLI, and the built frontend.
//
// Every API route runs one srl command and returns its JSON. There is no
// SRS logic here: scheduling belongs to srl (see docs/frontends.md, "The
// frontend contract").
//
//   GET  /api/config          the selector and store given on the command line
//   GET  /api/next?sel=…      srl next SEL --json     → { card } or { card: null }
//                             (with --context CONTEXT, chosen below)
//   POST /api/review          srl review --stdin      ← { card, rating, response? }
//   POST /api/suspend         srl suspend id:CARD     ← { card }
//
// Errors are { error: MESSAGE } with status 400 (bad request) or 500 (srl
// failed), MESSAGE being srl's own message when it comes from srl.

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';

import { SrlError, runSrl } from './srl.js';

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

// How much of the note to show after the answer: 'prose' (the note without its
// items), 'full' (with them) or 'none' (see docs/notes.md, "Context").
const CONTEXT = 'prose';

class BadRequest extends Error {}

// Creates the server. DIR is the store passed to srl as --dir (or
// undefined, to let srl find it), SELECTOR the default selector shown in the
// frontend, and STATIC_DIR the built frontend, or undefined to serve the API
// only (as in development, where Vite serves the frontend).
export function createSrlServer({ dir, selector, staticDir }) {
  const srl = (args, input) => runSrl(args, { dir, input });

  const routes = {
    'GET /api/config': async () => ({ selector, dir: dir ?? null }),

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
      const id = requireId(body.card);
      const { found } = await srl(['suspend', `id:${id}`]);
      return { suspended: found };
    },
  };

  return createServer(async (req, res) => {
    const url = new URL(req.url, 'http://localhost');
    try {
      checkHost(req);
      if (url.pathname.startsWith('/api/')) {
        const route = routes[`${req.method} ${url.pathname}`];
        if (!route) {
          throw new BadRequest(`no route ${req.method} ${url.pathname}`);
        }
        const body = req.method === 'POST' ? await readJson(req) : undefined;
        sendJson(res, 200, await route(url, body));
      } else if (staticDir && req.method === 'GET') {
        await sendStatic(res, staticDir, url.pathname);
      } else {
        sendJson(res, 404, { error: 'not found' });
      }
    } catch (error) {
      if (error instanceof BadRequest) {
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

// Card and item IDs are lowercase letters, digits and `-` (docs/cards.md).
function requireId(id) {
  if (typeof id !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(id)) {
    throw new BadRequest(`not a card or item ID: ${JSON.stringify(id)}`);
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
