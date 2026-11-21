import { AdjustmentsHorizontalIcon } from '@heroicons/react/24/outline';
import { useQuery } from '@tanstack/react-query';
import { useState, type KeyboardEvent } from 'react';

import { type Collection } from './api';
import { BAR_ICON_BUTTON, DANGER_BUTTON, ErrorView, Loading, MAIN_BUTTON, Skeleton } from './CardParts';
import { filterCount, IS_VALUES, NO_FILTERS, writable, type Filters, type Menu, type Search } from './filters';
import { Modal } from './Modal';
import { queries } from './queries';

// The text being written, which the dialog's Text box shows and changes:
// Browse's search box's, so that the two are one box (src/TopBar.tsx).
export type TextDraft = { value: string; onChange: (text: string) => void };

// What opens the dialog of SEARCH, the page's (src/filters.ts): its text and
// its filters, which Apply calls ON_SEARCH with. The text is TEXT, another
// box's, when given; without it, the dialog has its own, set to the
// search's each time it opens. Gives how to open it, how many of the text
// and the filters are set, and the dialog, to render beside what opens it.
function useFilters(search: Search, onSearch: (search: Search) => void, text?: TextDraft) {
  const [open, setOpen] = useState(false);
  // How many times the dialog was opened: its key, so that each opening
  // makes it anew, set to the search, while closing keeps it as it is to
  // fade out (src/Modal.tsx).
  const [openings, setOpenings] = useState(0);
  return {
    open: () => {
      setOpen(true);
      setOpenings((n) => n + 1);
    },
    count: filterCount(search.filters) + (search.text === '' ? 0 : 1),
    dialog: (
      <FilterDialog
        key={openings}
        open={open}
        search={search}
        text={text}
        onApply={(set) => {
          setOpen(false);
          onSearch(set);
        }}
        onClose={() => setOpen(false)}
      />
    ),
  };
}

// The filters' button, of a bar: it opens their dialog (useFilters). While
// the text or any filter is set, it is filled and shows how many, since
// nothing else on the page may say so: inside it, at the left of its icon,
// so that the icon stays where it is (src/TopBar.tsx). CLASSNAME places it.
export function FilterButton({
  search,
  onSearch,
  text,
  className = '',
}: {
  search: Search;
  onSearch: (search: Search) => void;
  text?: TextDraft;
  className?: string;
}) {
  const { open, count, dialog } = useFilters(search, onSearch, text);
  return (
    <>
      <button
        type="button"
        onClick={open}
        aria-label={count === 0 ? 'Filters' : `Filters, ${count} set`}
        title={count === 0 ? 'Filters' : `Filters: ${count} set`}
        className={`flex shrink-0 items-center gap-1.5 max-md:-mr-2 ${BAR_ICON_BUTTON} ${count === 0 ? '' : 'bg-stone-200 text-stone-900 dark:bg-stone-800 dark:text-stone-100'} ${className}`}
      >
        {count > 0 && (
          <span aria-hidden className="text-sm font-semibold tabular-nums">
            {count}
          </span>
        )}
        <AdjustmentsHorizontalIcon className="size-5" aria-hidden />
      </button>
      {dialog}
    </>
  );
}

// The filters as a link in a text, where there is room to say how many are
// set: "Set filters", or "3 filters set, change them". It opens their
// dialog (useFilters), so it is a button, which looks like a link.
export function FilterLink({ search, onSearch }: { search: Search; onSearch: (search: Search) => void }) {
  const { open, count, dialog } = useFilters(search, onSearch);
  return (
    <>
      <button
        type="button"
        onClick={open}
        className="text-stone-700 underline decoration-stone-400 underline-offset-2 hover:decoration-stone-700 dark:text-stone-300 dark:decoration-stone-600 dark:hover:decoration-stone-300"
      >
        {count === 0 ? 'Set filters' : `${count} ${count === 1 ? 'filter' : 'filters'} set, change them`}
      </button>
      {dialog}
    </>
  );
}

// The labels of the menus, for the values srl lists and those of is:.
const LABELS: Record<Exclude<Menu, 'leech' | 'id'>, string> = {
  collection: 'Collection',
  source: 'Source',
  kind: 'Kind',
  status: 'Status',
  schedule: 'Schedule',
};

// The name of every collection, in the order srl lists them: the values
// collection: takes. Several collections may share an in name, such as
// a/questions.jsonl and b/questions.jsonl, so the whole name is what picks
// one. A name collection: can't take, one with a space or a comma, isn't
// offered.
const collectionNames = (collections: Collection[]) =>
  collections.map((collection) => collection.name).filter(writable);

// The text and the menus, set to the page's search, APPLIED, when it is
// made, which is each time it opens (FilterButton gives it a new key):
// what they are set to counts only once applied. The text is TEXT
// when given, another box's, which keeps what was typed when the dialog is
// closed without applying, as that box does. What the menus offer is read
// from srl when it opens, and kept as everything read is (src/main.tsx),
// so it is the same as Add reads. The form waits for all of it, shown as
// one skeleton until then, as Add's does; the first error is shown in its
// place.
function FilterDialog({
  open,
  search: applied,
  text: sharedText,
  onApply,
  onClose,
}: {
  open: boolean;
  search: Search;
  text?: TextDraft;
  onApply: (search: Search) => void;
  onClose: () => void;
}) {
  const [filters, setFilters] = useState<Filters>(applied.filters);
  const [ownText, setOwnText] = useState(applied.text);
  const text = sharedText ?? { value: ownText, onChange: setOwnText };
  const collections = useQuery({ ...queries.collections(), enabled: open, select: collectionNames });
  const sources = useQuery({ ...queries.sources(), enabled: open });
  const kinds = useQuery({ ...queries.kinds(), enabled: open });
  const reads = [collections, sources, kinds];
  const failed = reads.find((read) => read.isError && !read.isFetching);
  const retry = () => reads.filter((read) => read.isError).forEach((read) => read.refetch());
  const set = (menu: Menu, value: string | null) => setFilters((before) => ({ ...before, [menu]: value }));

  // The ID as typed, which applies without the spaces around it, and only
  // while a term can hold it: whether it is an ID is srl's to say, when the
  // search is made.
  const id = filters.id?.trim() || null;
  const idValid = id === null || writable(id);
  const applyFilters = () => onApply({ filters: { ...filters, id }, text: text.value.trim() });
  // Enter in a box applies, as Apply would.
  const applyOnEnter = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter' && idValid) applyFilters();
  };

  // Apply, at the right of Cancel; disabled until the menus are shown, and
  // while the ID can't be one. Clear, far at their left, is shown while the
  // text or any filter is set, applied or not, and empties them all.
  const apply = (ready: boolean) => (
    <button onClick={applyFilters} disabled={!ready || !idValid} className={`px-4 py-2 text-sm ${MAIN_BUTTON}`}>
      Apply
    </button>
  );

  if (!(collections.isSuccess && sources.isSuccess && kinds.isSuccess)) {
    return (
      <Modal open={open} title="Filters" closeLabel="Cancel" actions={apply(false)} onClose={onClose}>
        {failed?.error ? <ErrorView message={failed.error.message} onRetry={retry} /> : <FiltersSkeleton />}
      </Modal>
    );
  }

  const options: Record<Exclude<Menu, 'leech' | 'id'>, string[]> = {
    collection: collections.data,
    source: sources.data,
    kind: kinds.data.map((kind) => kind.name),
    status: IS_VALUES.status,
    schedule: IS_VALUES.schedule,
  };
  return (
    <Modal
      open={open}
      title="Filters"
      closeLabel="Cancel"
      actions={apply(true)}
      aside={
        (filterCount(filters) > 0 || text.value !== '') && (
          <button
            onClick={() => {
              setFilters(NO_FILTERS);
              text.onChange('');
            }}
            className={`px-4 py-2 text-sm ${DANGER_BUTTON}`}
          >
            Clear
          </button>
        )
      }
      onClose={onClose}
    >
      {/* The words the items must contain, each as text whatever it looks
          like (src/filters.ts). */}
      <label className="flex flex-col gap-1.5 text-sm text-stone-500">
        Text
        <input
          value={text.value}
          onChange={(event) => text.onChange(event.target.value)}
          onKeyDown={applyOnEnter}
          placeholder="Words the items must contain"
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          className={`${BOX} placeholder:text-stone-400`}
        />
      </label>
      <div className="grid gap-4 sm:grid-cols-2">
        {(Object.keys(LABELS) as Exclude<Menu, 'leech' | 'id'>[]).map((name) => (
          <label key={name} className="flex flex-col gap-1.5 text-sm text-stone-500">
            {LABELS[name]}
            <Select value={filters[name]} options={options[name]} onChange={(value) => set(name, value)} />
          </label>
        ))}
        {/* The text is only ever text: an ID, of an item (all its cards) or
            of a card, is typed here. */}
        <label className="flex flex-col gap-1.5 text-sm text-stone-500">
          ID
          <input
            value={filters.id ?? ''}
            onChange={(event) => set('id', event.target.value === '' ? null : event.target.value)}
            onKeyDown={applyOnEnter}
            placeholder="Of an item or a card"
            aria-invalid={!idValid}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            className={`${BOX} font-mono placeholder:font-sans placeholder:text-stone-400 aria-[invalid=true]:border-rose-500`}
          />
        </label>
      </div>
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={filters.leech !== null}
          onChange={(event) => set('leech', event.target.checked ? 'leech' : null)}
          className="size-4 accent-stone-700 dark:accent-stone-300"
        />
        Leeches only
      </label>
    </Modal>
  );
}

// The look of the dialog's text boxes, as tall as its menus.
const BOX =
  'h-10 w-full rounded-lg border border-stone-300 bg-white px-2 text-base text-stone-900 dark:border-stone-700 dark:bg-stone-900 dark:text-stone-100';

// The form while srl reads what the menus offer: the text, the menus, the
// ID and the checkbox, as grey bars.
function FiltersSkeleton() {
  return (
    <Loading label="Reading the collections, sources and kinds" className="flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <Skeleton className="h-5 w-12" />
        <Skeleton className="h-10" />
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        {[...Object.keys(LABELS), 'id'].map((name) => (
          <div key={name} className="flex flex-col gap-1.5">
            <Skeleton className="h-5 w-20" />
            <Skeleton className="h-10" />
          </div>
        ))}
      </div>
      <Skeleton className="h-5 w-32" />
    </Loading>
  );
}

// A menu of OPTIONS and Any, null. A VALUE that isn't among them, as one
// written in the address by hand, is offered too, so that it isn't lost.
function Select({
  value,
  options,
  onChange,
}: {
  value: string | null;
  options: string[];
  onChange: (value: string | null) => void;
}) {
  const shown = value !== null && !options.includes(value) ? [value, ...options] : options;
  return (
    <select
      value={value ?? ''}
      onChange={(event) => onChange(event.target.value === '' ? null : event.target.value)}
      className="h-10 w-full rounded-lg border border-stone-300 bg-white px-2 font-mono text-base text-stone-900 dark:border-stone-700 dark:bg-stone-900 dark:text-stone-100"
    >
      <option value="">Any</option>
      {shown.map((option) => (
        <option key={option} value={option}>
          {option}
        </option>
      ))}
    </select>
  );
}
