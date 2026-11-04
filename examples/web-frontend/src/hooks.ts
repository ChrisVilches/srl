import { useEffect, useState } from 'react';

// The page's selector, with the number of selectors submitted in any view,
// so that submitting the same selector again counts as a new submission.
// A new one is made only when either changes: by submitting, or by back or
// forward coming to an address with another selector.
export type Submitted = { selector: string; n: number };

// Calls LOAD with the selector when the view is shown, and with each new one
// while it is. LOAD stays the same function, so that it runs only then.
export function useSubmitted(submitted: Submitted, load: (selector: string) => void) {
  useEffect(() => {
    load(submitted.selector);
  }, [submitted, load]);
}

// VALUE once it has stayed the same for MS, and at first VALUE itself, so
// that what follows from a choice runs only for the one that stays: moving
// through a list with the keys runs nothing for the rows passed by.
export function useSettled<T>(value: T, ms: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return settled;
}

// Gives ON_KEY the keys pressed, but those with Ctrl, Meta or Alt, those
// typed in a text box and those pressed while a dialog is open
// (src/Modal.tsx), which belong to it.
export function useKeys(onKey: (event: KeyboardEvent) => void) {
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      if (document.querySelector('dialog[open]')) return;
      onKey(event);
    };
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, [onKey]);
}
