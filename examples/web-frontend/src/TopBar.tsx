import { MoonIcon, SunIcon, XMarkIcon } from '@heroicons/react/24/outline';
import { useRef, useState, type ReactNode } from 'react';
import { Link, useLocation } from 'react-router';

import { VIEW_PATHS, type View } from './address';
import type { Search } from './filters';
import { BAR_ICON_BUTTON } from './CardParts';
import { FilterButton } from './Filters';
import { currentTheme, setTheme } from './theme';

// The bar at the top of each view: the switch between views, then the
// view's own controls. It spans the whole window in every view, however
// wide the view's content is, so it doesn't change width between tabs. The
// name links to the root, the study view with no search in the address,
// and loads the page anew. Wide (md and up) it is one row; narrow, the
// name, the tabs and the theme button make the first row and the view's
// controls the second. Below sm, its gaps and the tabs' padding are
// smaller, so that the first row fits a phone 320 px wide. The theme
// button's icon ends at the bar's padding, as far from the right as the
// name is from the left: the button takes as much of the padding as it has
// at its sides (BAR_ICON_BUTTON). So does the filters' button while there
// are two rows, so that its icon, under the theme button's, is aligned
// with it.
//
// A tab is a link to its view's path, keeping the search, and opens it in
// a new tab when clicked with a modifier or the middle button.
//
// Every view takes the window's height, h-dvh, and puts the bar at its top:
// the window itself never scrolls, only the view's parts under the bar do,
// each in its own box spanning to the window's edge or a border, so that its
// scrollbar is there and not beside the content. The scrollbar's room is
// kept in each, so the content doesn't shift as it comes and goes. A swipe
// or a wheel past either end of one stops there (VIEW_OVERSCROLL) rather
// than going on to the window, which would hide a phone's address bar, or
// stretch or bounce the whole page with its top bar.
export const VIEW_OVERSCROLL = 'overscroll-y-contain';

// The part of Study and Add under the bar, which scrolls, and the column in
// it, centered, as tall as the part at least, so that something can be put
// at its bottom with mt-auto.
export const VIEW_SCROLL = `min-h-0 flex-1 overflow-y-auto [scrollbar-gutter:stable] ${VIEW_OVERSCROLL}`;
export const VIEW_COLUMN = 'mx-auto flex min-h-full w-full max-w-3xl flex-col px-4 py-8 sm:px-6';

export function TopBar({ view, children }: { view: View; children: ReactNode }) {
  const { search } = useLocation();
  const tab = (target: View, name: string) => (
    <Link
      to={{ pathname: VIEW_PATHS[target], search }}
      className={
        view === target
          ? 'rounded-md bg-stone-200 px-1.5 py-1 sm:px-2.5 font-medium dark:bg-stone-800'
          : 'rounded-md px-1.5 py-1 sm:px-2.5 text-stone-500 hover:bg-stone-100 dark:hover:bg-stone-900'
      }
    >
      {name}
    </Link>
  );
  return (
    <header className="border-b border-stone-200 bg-white/70 backdrop-blur dark:border-stone-800 dark:bg-stone-900/70">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-2 px-4 sm:gap-x-4 py-3 sm:px-6">
        <a href="/" className="font-mono text-lg font-bold tracking-tight">
          srl
        </a>
        <nav className="flex gap-0.5 text-sm sm:gap-1">
          {tab('study', 'Study')}
          {tab('browse', 'Browse')}
          {tab('add', 'Add')}
          {tab('stats', 'Stats')}
        </nav>
        {children != null && (
          <div className="order-last flex w-full flex-wrap items-center gap-x-4 gap-y-2 md:contents">{children}</div>
        )}
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
      className={`-mr-2 ml-auto ${BAR_ICON_BUTTON}`}
    >
      <Icon className="size-5" aria-hidden />
    </button>
  );
}

// Browse's search (src/filters.ts): a box for the text, the words the items
// must contain, and at its right the filters' button (src/Filters.tsx),
// whose Text box is this one: they share what is typed, applied or not.
// Submitting the box, or applying the filters, calls ON_SEARCH with both.
// While the box holds text, an X at its right empties it and, if a text is
// submitted, submits none, keeping the filters. The other views have the
// filters' button alone, the text in it.
//
// The box starts with the search's text, and starts anew whenever the text
// submitted changes, as when back or forward comes to another search: it
// is made again by its key, rather than set from an effect, which would
// show the text typed before for a render first. So what was typed and not
// submitted is dropped then, and the box isn't focused any more.
export function SearchInput({ search, onSearch }: { search: Search; onSearch: (search: Search) => void }) {
  return <SearchBox key={search.text} search={search} onSearch={onSearch} />;
}

function SearchBox({ search, onSearch }: { search: Search; onSearch: (search: Search) => void }) {
  const [draft, setDraft] = useState(search.text);
  const input = useRef<HTMLInputElement>(null);

  return (
    <div className="flex min-w-48 flex-1 items-center gap-1">
      <form
        className="relative min-w-0 flex-1"
        onSubmit={(event) => {
          event.preventDefault();
          input.current?.blur();
          onSearch({ ...search, text: draft.trim() });
        }}
      >
        <input
          ref={input}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="Search text…"
          aria-label="Search the items' text"
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          className="w-full rounded-md border border-stone-300 bg-transparent py-1.5 pr-9 pl-3 text-sm outline-none placeholder:text-stone-400 focus:border-stone-500 dark:border-stone-700 dark:focus:border-stone-400"
        />
        {draft !== '' && (
          <button
            type="button"
            onClick={() => {
              // A text submitted is submitted empty, which makes the box
              // anew, empty; one only typed is emptied here.
              if (search.text !== '') {
                onSearch({ ...search, text: '' });
              } else {
                setDraft('');
                input.current?.focus();
              }
            }}
            aria-label="Clear the text"
            title="Clear the text"
            className="absolute inset-y-0 right-1 my-auto h-fit rounded-md p-1 text-stone-400 hover:bg-stone-200 hover:text-stone-600 dark:hover:bg-stone-800 dark:hover:text-stone-300"
          >
            <XMarkIcon className="size-4" aria-hidden />
          </button>
        )}
      </form>
      <FilterButton search={search} onSearch={onSearch} text={{ value: draft, onChange: setDraft }} />
    </div>
  );
}
