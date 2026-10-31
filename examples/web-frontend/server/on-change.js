// Runs the --on-change program after srl-web changes the store: a review, a
// suspension, an item added.
//
// Runs never overlap and are coalesced: a change starts a run a moment
// later (DELAY_MS), so that changes made close together get one run, and a
// change made while the program runs gets one more run after it, however
// many changes there were. The page learns how the last run went from
// status(), and shows its error.

import { spawn } from 'node:child_process';

// How long after a change the program starts, so that a burst of changes
// gets one run.
const DELAY_MS = 1000;

// How long a run may take before it is killed and counted as failed, so
// that a program that hangs (a git push waiting for a password, say)
// doesn't keep every later change from its run.
const TIMEOUT_MS = 120_000;

// PROGRAM is run with no arguments, in srl-web's working directory (a path
// with no directory in it is looked for on PATH), with
// SRL_DIR set to DIR when a store was given, so that the program, and srl in
// it, works on the store srl-web uses.
export function createOnChange(program, dir) {
  let timer = null; // a run is waiting for DELAY_MS
  let running = false;
  let again = false; // a change came while the program ran
  let last = null; // { seq, error }: how the last run went, error null on success
  let seq = 0;

  const start = () => {
    timer = null;
    running = true;
    again = false;
    run(program, dir).then((error) => {
      seq += 1;
      last = { seq, error };
      if (error !== null) {
        console.error(`srl-web: --on-change: ${error}`);
      }
      running = false;
      if (again) {
        changed();
      }
    });
  };

  const changed = () => {
    if (running) {
      again = true;
    } else if (timer === null) {
      timer = setTimeout(start, DELAY_MS);
    }
  };

  return {
    changed,
    // busy is true until every change so far has had its run.
    status: () => ({ busy: timer !== null || running, last }),
  };
}

// Resolves to null when PROGRAM exits with 0, or else to its error: what it
// printed to stderr, or a message saying how it ended when it printed
// nothing. What it prints to stdout goes to srl-web's. On Windows, Node runs
// only programs, such as .exe files, this way: not .bat or .cmd scripts.
function run(program, dir) {
  return new Promise((resolve) => {
    const env = dir ? { ...process.env, SRL_DIR: dir } : process.env;
    const child = spawn(program, [], { stdio: ['ignore', 'inherit', 'pipe'], env, timeout: TIMEOUT_MS });
    let stderr = '';
    child.stderr.setEncoding('utf8').on('data', (chunk) => (stderr += chunk));
    child.on('error', (error) => resolve(`cannot run ${program} (${error.message})`));
    child.on('close', (code, signal) => {
      if (code === 0) {
        resolve(null);
      } else if (stderr.trim() !== '') {
        resolve(stderr.trim());
      } else if (signal !== null) {
        resolve(`${program} was stopped by ${signal}${child.killed ? `, after ${TIMEOUT_MS / 1000} s` : ''}`);
      } else {
        resolve(`${program} exited with code ${code}`);
      }
    });
  });
}
