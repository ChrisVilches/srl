import { useEffect, useState } from 'react';

import { api, onStoreChange } from './api';
import { Modal } from './Modal';

// How often the page asks whether the --on-change program has finished.
const POLL_MS = 1000;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// Shows the error of the --on-change program, when srl-web was started with
// one. After each change the page makes, it asks the server how the
// program is doing until every change has had its run, then shows the last
// run's error if it failed. A run that failed but was followed by one that
// succeeded is not shown, and a later error replaces the one shown: there is
// never more than one dialog.
export function OnChangeError() {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // The seq of the last run whose outcome was seen: runs from before the
    // page was opened are not shown.
    // null when there is no program.
    const seen = api
      .hasOnChange()
      .then(async (has) => (has ? ((await api.onChangeStatus()).last?.seq ?? 0) : null))
      .catch(() => null);
    let shown = 0;
    let polling = false;
    let changedAgain = false;

    const poll = async () => {
      polling = true;
      try {
        const first = await seen;
        if (first === null) return;
        shown = Math.max(shown, first);
        for (;;) {
          changedAgain = false;
          await sleep(POLL_MS);
          const { busy, last } = await api.onChangeStatus();
          if (busy || changedAgain) continue;
          if (last !== null && last.seq > shown) {
            shown = last.seq;
            if (last.error !== null) setError(last.error);
          }
          return;
        }
      } catch {
        // srl-web is gone, or the network: the next change asks again.
      } finally {
        polling = false;
      }
    };

    return onStoreChange(() => {
      changedAgain = true;
      if (!polling) poll();
    });
  }, []);

  return (
    <Modal open={error !== null} title="The --on-change program failed" onClose={() => setError(null)}>
      <p className="text-sm text-stone-500">
        The change is saved in the store. srl-web ran its --on-change program after it, which failed with:
      </p>
      <pre className="max-h-[50vh] overflow-auto rounded-lg bg-rose-50 p-3 font-mono text-sm whitespace-pre-wrap text-rose-800 dark:bg-rose-950/40 dark:text-rose-300">
        {error}
      </pre>
    </Modal>
  );
}
