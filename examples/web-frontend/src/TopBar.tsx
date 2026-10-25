import { useEffect, useRef, useState, type ReactNode } from 'react';

// The views: studying (the page's start), the card browser (#browse), and
// adding items (#add).
export type View = 'study' | 'browse' | 'add';

// The bar at the top of each view: the switch between views, then the
// view's own controls.
export function TopBar({ view, wide, children }: { view: View; wide?: boolean; children: ReactNode }) {
  const tab = (target: View, href: string, name: string) => (
    <a
      href={href}
      className={
        view === target
          ? 'rounded-md bg-stone-200 px-2.5 py-1 font-medium dark:bg-stone-800'
          : 'rounded-md px-2.5 py-1 text-stone-500 hover:bg-stone-100 dark:hover:bg-stone-900'
      }
    >
      {name}
    </a>
  );
  return (
    <header className="border-b border-stone-200 bg-white/70 backdrop-blur dark:border-stone-800 dark:bg-stone-900/70">
      <div
        className={`mx-auto flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 sm:px-6 ${wide ? 'max-w-7xl' : 'max-w-3xl'}`}
      >
        <span className="font-mono text-lg font-bold tracking-tight">srl</span>
        <nav className="flex gap-1 text-sm">
          {tab('study', '#', 'Study')}
          {tab('browse', '#browse', 'Browse')}
          {tab('add', '#add', 'Add')}
        </nav>
        {children}
      </div>
    </header>
  );
}

// The selector box of a view. Submitting it calls ON_SELECTOR.
export function SelectorInput({
  selector,
  placeholder,
  onSelector,
}: {
  selector: string | null;
  placeholder: string;
  onSelector: (selector: string) => void;
}) {
  const [draft, setDraft] = useState(selector ?? '');
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => setDraft(selector ?? ''), [selector]);

  return (
    <form
      className="min-w-48 flex-1"
      onSubmit={(event) => {
        event.preventDefault();
        input.current?.blur();
        onSelector(draft.trim());
      }}
    >
      <input
        ref={input}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        placeholder={placeholder}
        spellCheck={false}
        className="w-full rounded-md border border-stone-300 bg-transparent px-3 py-1.5 font-mono text-sm outline-none placeholder:text-stone-400 focus:border-stone-500 dark:border-stone-700 dark:focus:border-stone-400"
      />
    </form>
  );
}
