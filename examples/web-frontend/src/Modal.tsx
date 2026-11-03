import { useEffect, useRef, type ReactNode } from 'react';

import { OUTLINE_BUTTON } from './CardParts';

// A modal dialog, shown while OPEN: the browser's own <dialog>, which keeps
// focus in it and closes on Escape. ON_CLOSE is called when it is closed,
// with Escape, its button or a click outside it. Every modal of the page is
// this one, so that a dialog library could replace it in one place (Radix's
// injects <style> elements, which the page's Content-Security-Policy
// refuses). It fades and grows in, and out, by the class `modal`
// (src/index.css); while it fades out, it keeps showing what it showed
// when it was open, though its parent has moved on. A form whose button
// opens one keeps its buttons disabled until it closes (README.md, "A
// button whose answer is a modal").
export function Modal({
  open,
  title,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
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
      className="modal m-auto w-[min(36rem,calc(100%-2rem))] rounded-2xl border border-stone-200 bg-white p-0 text-stone-900 shadow-xl backdrop:bg-stone-950/50 dark:border-stone-800 dark:bg-stone-900 dark:text-stone-100"
    >
      <div className="flex flex-col gap-4 p-6">
        <h2 className="text-lg font-semibold">{shown.current.title}</h2>
        {shown.current.children}
        <button onClick={onClose} className={`self-end ${OUTLINE_BUTTON}`}>
          Close
        </button>
      </div>
    </dialog>
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
