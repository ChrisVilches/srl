// Development: the API server on port 4318, and Vite on 5173 with hot
// reload, sending /api to it (see vite.config.ts). Arguments are the
// selector, as for srl-web; SRL_DIR chooses the store, as for srl.

import { createSrlServer } from './server.js';

const API_PORT = 4318;

createSrlServer({ selector: process.argv.slice(2).join(' ') }).listen(API_PORT, '127.0.0.1');

const { createServer } = await import('vite');
const vite = await createServer();
await vite.listen();
vite.printUrls();
