import { MoonIcon, SunIcon, XMarkIcon } from '@heroicons/react/24/outline';
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { Link, useLocation } from 'react-router';

import { VIEW_PATHS, type View } from './address';
import { QUIET_BUTTON } from './CardParts';
import { currentTheme, setTheme } from './theme';

// The last path of each view, which its tab goes back to (see App).
export const ViewPaths = createContext<Record<View, string>>(VIEW_PATHS);

// The bar at the top of each view: the switch between views, then the
// view's own controls. It spans the whole window in every view, however
// wide the view's content is, so it doesn't change width between tabs. The
// name links to the root, the study view with no selector in the address,
// and loads the page anew.
//
// A tab goes to the view's last address, keeping the selector, so Browse
// comes back to its card, and opens it in a new tab when clicked with a
// modifier or the middle button. The tab of the view shown stays on its
// address, adding nothing to the browser's history.
export function TopBar({ view, children }: { view: View; children: ReactNode }) {
  const paths = useContext(ViewPaths);
  const { search } = useLocation();
  const tab = (target: View, name: string) => (
    <Link
      to={{ pathname: paths[target], search }}
      className={
        view === target
          ? 'rounded-md bg-stone-200 px-2.5 py-1 font-medium dark:bg-stone-800'
          : 'rounded-md px-2.5 py-1 text-stone-500 hover:bg-stone-100 dark:hover:bg-stone-900'
      }
    >
      {name}
    </Link>
  );
  return (
    <header className="border-b border-stone-200 bg-white/70 backdrop-blur dark:border-stone-800 dark:bg-stone-900/70">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 sm:px-6">
        <a href="/" className="font-mono text-lg font-bold tracking-tight">
          srl
        </a>
        <nav className="flex gap-1 text-sm">
          {tab('study', 'Study')}
          {tab('browse', 'Browse')}
          {tab('add', 'Add')}
        </nav>
        {children}
        <ThemeButton />
      </div>
    </header>
  );
}

// Switches between the dark and light themes, showing the icon of the one
// it switches to: the sun in dark, the moon in light.
function ThemeButton() {
  const [theme, setThemeState] = useState(currentTheme);
  const other = theme === 'dark' ? 'light' : 'dark';
  const Icon = other === 'light' ? SunIcon : MoonIcon;
  return (
    <button
      onClick={() => {
        setTheme(other);
        setThemeState(other);
      }}
      aria-label={`Switch to the ${other} theme`}
      title={`Switch to the ${other} theme, kept in this browser`}
      className={`ml-auto py-1.5 ${QUIET_BUTTON}`}
    >
      <Icon className="size-5" aria-hidden />
    </button>
  );
}

// The selector box of a view. Submitting it calls ON_SELECTOR. Its
// placeholder is a selector that works, the same in every view. While it
// holds text, an X at its right empties it and, if a selector is
// submitted, submits none, every card, keeping the box focused to type
// another.
export function SelectorInput({
  selector,
  onSelector,
}: {
  selector: string;
  onSelector: (selector: string) => void;
}) {
  const [draft, setDraft] = useState(selector);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => setDraft(selector), [selector]);

  return (
    <form
      className="relative min-w-48 flex-1"
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
        placeholder="is:young in:grammar"
        spellCheck={false}
        className="w-full rounded-md border border-stone-300 bg-transparent py-1.5 pr-9 pl-3 font-mono text-sm outline-none placeholder:text-stone-400 focus:border-stone-500 dark:border-stone-700 dark:focus:border-stone-400"
      />
      {draft !== '' && (
        <button
          type="button"
          onClick={() => {
            setDraft('');
            if (selector !== '') onSelector('');
            input.current?.focus();
          }}
          aria-label="Clear the selector"
          title="Clear the selector"
          className="absolute inset-y-0 right-1 my-auto h-fit rounded-md p-1 text-stone-400 hover:bg-stone-200 hover:text-stone-600 dark:hover:bg-stone-800 dark:hover:text-stone-300"
        >
          <XMarkIcon className="size-4" aria-hidden />
        </button>
      )}
    </form>
  );
}
