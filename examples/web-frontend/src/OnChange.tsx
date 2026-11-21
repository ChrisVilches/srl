import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import { type OnChangeStatus } from './api';
import { Modal } from './Modal';
import { queries } from './queries';

// How often the page asks whether the --on-change program has finished.
const POLL_MS = 1000;

// Shows the error of the --on-change program, when srl-web was started with
// one. After each change the page makes, it asks the server how the
// program is doing until every change has had its run, then shows the last
// run's error if it failed. A run that failed but was followed by one that
// succeeded is not shown, and a later error replaces the one shown: there is
// never more than one dialog.
//
// The status is read as other reads are (src/queries.ts): again after each
// change, which makes it out of date (src/main.tsx), and then every POLL_MS
// while the server says the program is busy. While the dialog is open, the
// page takes no change, so the status stays as it is.
export function OnChangeError() {
  const has = useQuery(queries.hasOnChange()).data === true;
  const status = useQuery({ ...queries.onChangeStatus(POLL_MS), enabled: has }).data;
  // Made once the first status is read, which it starts from; the status
  // read before stays while it is read again, so it is made only once.
  return status === undefined ? null : <OnChangeDialog status={status} />;
}

function OnChangeDialog({ status }: { status: OnChangeStatus }) {
  // The seq of the last run whose outcome was seen: of the first status
  // read, when this is made, so that runs from before the page was opened
  // are not shown, and then of the last error shown.
  const [seen, setSeen] = useState(status.last?.seq ?? 0);
  // The last run once every change has had its run, if it is new and failed.
  const last = !status.busy ? status.last : null;
  const failed = last !== null && last.seq > seen && last.error !== null ? last : null;

  return (
    <Modal
      open={failed !== null}
      title="The --on-change program failed"
      onClose={() => failed !== null && setSeen(failed.seq)}
    >
      <p className="text-sm text-stone-500">
        The change is saved in the store. srl-web ran its --on-change program after it, which failed with:
      </p>
      <pre className="max-h-[50vh] overflow-auto rounded-lg bg-rose-50 p-3 font-mono text-sm whitespace-pre-wrap text-rose-800 dark:bg-rose-950/40 dark:text-rose-300">
        {failed?.error}
      </pre>
    </Modal>
  );
}
