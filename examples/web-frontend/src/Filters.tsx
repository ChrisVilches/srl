import { FunnelIcon } from '@heroicons/react/24/outline';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import { MAIN_BUTTON, OUTLINE_BUTTON, QUIET_BUTTON, Skeleton } from './CardParts';
import { IS_VALUES, NO_FILTERS, readFilters, writable, writeFilters, type Filters, type Menu } from './filters';
import { Modal } from './Modal';
import { queries } from './queries';

// A button that opens the filters (src/filters.ts) of SELECTOR, the text of
// the selector box. Apply calls ON_SELECTOR with the selector they write.
export function FilterButton({ selector, onSelector }: { selector: string; onSelector: (selector: string) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label="Filters"
        title="Filters: write the selector with menus"
        className={`shrink-0 py-1.5 ${QUIET_BUTTON}`}
      >
        <FunnelIcon className="size-5" aria-hidden />
      </button>
      <FilterDialog
        open={open}
        selector={selector}
        onApply={(filters) => {
          setOpen(false);
          onSelector(writeFilters(selector, filters));
        }}
        onClose={() => setOpen(false)}
      />
    </>
  );
}

// The labels of the menus, for the values srl lists and those of is:.
const LABELS: Record<Exclude<Menu, 'leech'>, string> = {
  in: 'Collection',
  source: 'Source',
  kind: 'Kind',
  status: 'Status',
  schedule: 'Schedule',
};

// The in names of every collection, each once, sorted: the values in: takes.
// A name in: can't take, one with a space or a comma, isn't offered.
const inNames = (collections: { in: string[] }[]) =>
  [...new Set(collections.flatMap((collection) => collection.in))].filter(writable).sort((a, b) => a.localeCompare(b));

// The menus, set from the selector each time it opens. What they offer is
// read from srl when it opens, and kept as everything read is (src/main.tsx),
// so it is the same as Add reads.
function FilterDialog({
  open,
  selector,
  onApply,
  onClose,
}: {
  open: boolean;
  selector: string;
  onApply: (filters: Filters) => void;
  onClose: () => void;
}) {
  const [filters, setFilters] = useState<Filters>(NO_FILTERS);
  useEffect(() => {
    if (open) setFilters(readFilters(selector));
  }, [open, selector]);
  const collections = useQuery({ ...queries.collections(), enabled: open, select: inNames });
  const sources = useQuery({ ...queries.sources(), enabled: open });
  const kinds = useQuery({ ...queries.kinds(), enabled: open });
  const set = (menu: Menu, value: string | null) => setFilters((before) => ({ ...before, [menu]: value }));

  const menu = (name: Exclude<Menu, 'leech'>, read: { data?: string[]; error: Error | null; isPending: boolean }) => (
    <label className="flex flex-col gap-1.5 text-sm text-stone-500">
      {LABELS[name]}
      {read.error ? (
        <span className="text-rose-600 dark:text-rose-400">{read.error.message}</span>
      ) : read.isPending ? (
        <Skeleton className="h-10" />
      ) : (
        <Select value={filters[name]} options={read.data ?? []} onChange={(value) => set(name, value)} />
      )}
    </label>
  );
  const fixed = (values: string[]) => ({ data: values, error: null, isPending: false });

  return (
    <Modal open={open} title="Filters" onClose={onClose}>
      <div className="grid gap-4 sm:grid-cols-2">
        {menu('in', collections)}
        {menu('source', sources)}
        {menu('kind', { ...kinds, data: kinds.data?.map((kind) => kind.name) })}
        {menu('status', fixed(IS_VALUES.status))}
        {menu('schedule', fixed(IS_VALUES.schedule))}
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
      <p className="text-xs text-stone-400">
        The other terms of the selector, such as text, are kept as written.
      </p>
      <div className="flex gap-3">
        <button onClick={() => onApply(filters)} className={`px-5 py-2 ${MAIN_BUTTON}`}>
          Apply
        </button>
        <button onClick={() => setFilters(NO_FILTERS)} className={OUTLINE_BUTTON}>
          Clear
        </button>
      </div>
    </Modal>
  );
}

// A menu of OPTIONS and Any, null. A VALUE that isn't among them, as one typed
// in the box, is offered too, so that it isn't lost.
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
