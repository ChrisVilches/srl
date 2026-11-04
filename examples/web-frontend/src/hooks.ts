import { useEffect, useRef } from 'react';

// Runs START the first time ACTIVE is true: a view starts when first shown.
export function useStartWhenShown(active: boolean, start: () => void) {
  const started = useRef(false);
  useEffect(() => {
    if (!active || started.current) return;
    started.current = true;
    start();
  }, [active, start]);
}

// The page's selector, with the number of selectors submitted in any view,
// so that submitting the same selector again counts as a new submission.
// A new one is made only when either changes: by submitting, or by back or
// forward coming to an address with another selector.
export type Submitted = { selector: string; n: number };

// Calls LOAD with each new selector, once ACTIVE is true: a view applies
// the page's selector when shown, so a hidden view runs no srl.
export function useSubmitted(active: boolean, submitted: Submitted, load: (selector: string) => void) {
  const loaded = useRef<Submitted | null>(null);
  useEffect(() => {
    if (!active || loaded.current === submitted) return;
    loaded.current = submitted;
    load(submitted.selector);
  }, [active, submitted, load]);
}

// Gives ON_KEY the keys pressed while ACTIVE, but those with Ctrl, Meta or
// Alt, those typed in a text box and those pressed while a dialog is open
// (src/Modal.tsx), which belong to it.
export function useKeys(active: boolean, onKey: (event: KeyboardEvent) => void) {
  useEffect(() => {
    if (!active) return;
    const listener = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      if (document.querySelector('dialog[open]')) return;
      onKey(event);
    };
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, [active, onKey]);
}
