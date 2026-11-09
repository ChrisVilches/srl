import { useEffect, useRef, type ReactNode } from 'react';
import { useBlocker } from 'react-router';

import { DANGER_BUTTON, OUTLINE_BUTTON } from './CardParts';

// A modal dialog, shown while OPEN: the browser's own <dialog>, which keeps
// focus in it and closes on Escape. ON_CLOSE is called when it is closed,
// with Escape, its button or a click outside it. Every modal of the page is
// this one, so that a dialog library could replace it in one place (Radix's
// injects <style> elements, which the page's Content-Security-Policy
// refuses). It fades and grows in, and out, by the class `modal`
// (src/index.css); while it fades out, it keeps showing what it showed
// when it was open, though its parent has moved on. A form whose button
// opens one keeps its buttons disabled until it closes (README.md, "A
// button whose answer is a modal"). A TALL one is as tall as the window
// whatever it shows, for one whose content comes in parts and would
// otherwise grow and move with each.
export function Modal({
  open,
  title,
  tall,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
  tall?: boolean;
  onClose: () => void;
  children: ReactNode;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = dialog.current!;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);
  const shown = useRef({ title, children });
  if (open) shown.current = { title, children };

  return (
    <dialog
      ref={dialog}
      onClose={onClose}
      // A click on the backdrop is a click on the dialog itself.
      onClick={(event) => event.target === dialog.current && onClose()}
      className={`modal m-auto max-h-[calc(100dvh-2rem)] ${tall ? 'h-[calc(100dvh-2rem)]' : ''} w-[min(36rem,calc(100%-2rem))] rounded-2xl border border-stone-200 bg-white p-0 text-stone-900 shadow-xl backdrop:bg-stone-950/50 dark:border-stone-800 dark:bg-stone-900 dark:text-stone-100`}
    >
      {/* Only what it shows scrolls, between its title and its button. The
          box, not the <dialog>, is a flex column: a <dialog> shown as flex
          would be shown when closed too. Its height is the dialog's, less
          the dialog's border; in a tall one, what it shows fills the room
          left, keeping the button at the bottom. Its 4px more above and
          below keep a focus ring inside it whole. */}
      <div className="flex h-full max-h-[calc(100dvh-2rem-2px)] flex-col gap-4 py-6">
        <h2 className="px-6 text-lg font-semibold">{shown.current.title}</h2>
        <div className="-my-1 flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 py-1">
          {shown.current.children}
        </div>
        <button onClick={onClose} className={`mx-6 self-end ${OUTLINE_BUTTON}`}>
          Close
        </button>
      </div>
    </dialog>
  );
}

// Asks before going to another path while UNSAVED, since the form showing
// it goes with its view: Leave goes on, and closing the modal stays.
// WHAT says what would be lost. Another selector, which keeps the path,
// isn't asked about. Reloading or closing the page, which the router never
// sees, is asked about by the browser, in its own words (beforeunload).
export function UnsavedGuard({ unsaved, what }: { unsaved: boolean; what: string }) {
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) => unsaved && currentLocation.pathname !== nextLocation.pathname,
  );
  useEffect(() => {
    if (!unsaved) return;
    const ask = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener('beforeunload', ask);
    return () => window.removeEventListener('beforeunload', ask);
  }, [unsaved]);
  return (
    <Modal open={blocker.state === 'blocked'} title="Leave without saving?" onClose={() => blocker.reset?.()}>
      <p className="text-sm text-stone-500">{what}</p>
      <button onClick={() => blocker.proceed?.()} className={`self-start px-4 py-2 text-sm ${DANGER_BUTTON}`}>
        Leave
      </button>
    </Modal>
  );
}

const RESULT_BOX = {
  ok: 'bg-emerald-50 text-emerald-950 dark:bg-emerald-950/40 dark:text-emerald-100',
  error: 'bg-rose-50 text-rose-950 dark:bg-rose-950/40 dark:text-rose-200',
};

// What srl answered to a change, in a modal: in green when it worked, in red
// when it didn't. Its text keeps its line breaks and wraps, and a long
// answer scrolls in the box; MONO for text written for srl, such as an item.
// Every view that changes the store shows its outcome this way.
export function ResultBox({ tone, mono, children }: { tone: 'ok' | 'error'; mono?: boolean; children: ReactNode }) {
  return (
    <div
      className={`max-h-[50vh] overflow-y-auto rounded-lg p-3 text-sm break-words whitespace-pre-wrap ${mono ? 'font-mono' : ''} ${RESULT_BOX[tone]}`}
    >
      {children}
    </div>
  );
}
