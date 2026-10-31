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

// Gives ON_KEY the keys pressed while ACTIVE, but those with Ctrl, Meta or
// Alt, those typed in a text box and those pressed while a dialog is open
// (src/Modal.tsx), which belong to it.
export function useKeys(active: boolean, onKey: (event: KeyboardEvent) => void) {
  useEffect(() => {
    if (!active) return;
    const listener = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.target instanceof HTMLInputElement) return;
      if (document.querySelector('dialog[open]')) return;
      onKey(event);
    };
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, [active, onKey]);
}
