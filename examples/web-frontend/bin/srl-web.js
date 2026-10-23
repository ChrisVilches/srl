#!/usr/bin/env node
// srl-web: a web viewer for srl. On PATH, it runs as `srl web [SELECTOR]`.
//
// Starts a server on localhost that runs srl commands for a frontend in the
// browser, and opens the browser on it.

import { existsSync } from 'node:fs';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { createSrlServer } from '../server/server.js';

const USAGE = `usage: srl web [--port N] [--dir DIR] [--no-open] [SELECTOR]

Starts a web viewer for srl on http://localhost:PORT and opens it.

  --port N    the port to listen on (default 4317; 0 picks a free one)
  --dir DIR   the store, passed to srl as --dir (default: srl finds it)
  --no-open   don't open the browser
  SELECTOR    the cards to study, as in srl next (can be changed in the page)`;

const staticDir = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'dist');

let options;
try {
  options = parseArgs({
    allowPositionals: true,
    options: {
      port: { type: 'string', default: '4317' },
      dir: { type: 'string' },
      'no-open': { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
    },
  });
} catch (error) {
  fail(`${error.message}\n\n${USAGE}`);
}
const { values, positionals } = options;
if (values.help) {
  console.log(USAGE);
  process.exit(0);
}
const port = Number(values.port);
if (!Number.isInteger(port) || port < 0 || port > 65535) {
  fail(`--port must be a number from 0 to 65535, not ${values.port}`);
}
if (!existsSync(path.join(staticDir, 'index.html'))) {
  fail(`the frontend is not built: run \`npm run build\` in ${path.dirname(staticDir)}`);
}

const server = createSrlServer({
  dir: values.dir && path.resolve(values.dir),
  selector: positionals.join(' '),
  staticDir,
});
server.on('error', (error) => {
  fail(error.code === 'EADDRINUSE' ? `port ${port} is in use; try --port` : error.message);
});
server.listen(port, '127.0.0.1', () => {
  const url = `http://localhost:${server.address().port}/`;
  console.log(`srl-web: serving on ${url} (Ctrl-C to stop)`);
  if (!values['no-open']) {
    openBrowser(url);
  }
});

function openBrowser(url) {
  const [command, args] =
    process.platform === 'darwin'
      ? ['open', [url]]
      : process.platform === 'win32'
        ? ['cmd', ['/c', 'start', '', url]]
        : ['xdg-open', [url]];
  const child = spawn(command, args, { stdio: 'ignore', detached: true });
  child.on('error', () => console.error(`srl-web: cannot open a browser; open ${url}`));
  child.unref();
}

function fail(message) {
  console.error(`srl-web: ${message}`);
  process.exit(2);
}
