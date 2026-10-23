// Runs srl commands. Every route of the server is one call to `srl`, so this
// is the only place that knows how srl is started and what its exit codes
// mean (0 found, 1 nothing, 2 error; see docs/cli.md).

import { spawn } from 'node:child_process';

export class SrlError extends Error {}

// Runs `srl ARGS…` with `--dir DIR` if a store was given, writes INPUT to
// its standard input, and resolves to { found, stdout }: found is false when
// srl exits with 1. Rejects with an SrlError carrying srl's message on any
// other exit code.
export function runSrl(args, { dir, input } = {}) {
  const fullArgs = dir ? ['--dir', dir, ...args] : args;
  return new Promise((resolve, reject) => {
    const child = spawn('srl', fullArgs, { stdio: ['pipe', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8').on('data', (chunk) => (stdout += chunk));
    child.stderr.setEncoding('utf8').on('data', (chunk) => (stderr += chunk));
    child.on('error', (error) => {
      reject(new SrlError(`cannot run srl (${error.message}); is it on PATH?`));
    });
    child.on('close', (code) => {
      if (code === 0 || code === 1) {
        resolve({ found: code === 0, stdout });
      } else {
        const message = stderr.trim() || `srl ${args[0]} failed with exit code ${code}`;
        reject(new SrlError(message));
      }
    });
    child.stdin.end(input ?? '');
  });
}

// Parses JSONL output into an array of objects.
export function parseLines(stdout) {
  return stdout
    .split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => JSON.parse(line));
}
