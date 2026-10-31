import { useEffect, useRef, type ReactNode } from 'react';

import { OUTLINE_BUTTON } from './CardParts';

// A modal dialog, shown while OPEN: the browser's own <dialog>, which keeps
// focus in it and closes on Escape. ON_CLOSE is called when it is closed,
// with Escape, its button or a click outside it. Every modal of the page is
// this one, so that a dialog library could replace it in one place (Radix's
// injects <style> elements, which the page's Content-Security-Policy
// refuses).
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

  return (
    <dialog
      ref={dialog}
      onClose={onClose}
      // A click on the backdrop is a click on the dialog itself.
      onClick={(event) => event.target === dialog.current && onClose()}
      className="m-auto w-[min(36rem,calc(100%-2rem))] rounded-2xl border border-stone-200 bg-white p-0 text-stone-900 shadow-xl backdrop:bg-stone-950/50 dark:border-stone-800 dark:bg-stone-900 dark:text-stone-100"
    >
      <div className="flex flex-col gap-4 p-6">
        <h2 className="text-lg font-semibold">{title}</h2>
        {children}
        <button onClick={onClose} className={`self-end ${OUTLINE_BUTTON}`}>
          Close
        </button>
      </div>
    </dialog>
  );
}
