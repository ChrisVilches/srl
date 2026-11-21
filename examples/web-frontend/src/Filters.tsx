import { AdjustmentsHorizontalIcon } from '@heroicons/react/24/outline';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import { BAR_ICON_BUTTON, DANGER_BUTTON, ErrorView, Loading, MAIN_BUTTON, Skeleton } from './CardParts';
import { filterCount, IS_VALUES, NO_FILTERS, writable, type Filters, type Menu } from './filters';
import { Modal } from './Modal';
import { queries } from './queries';

// A button that opens the menus of FILTERS, the page's (src/filters.ts).
// Apply calls ON_FILTERS with what they are set to. While any filter is
// set, the button is filled and shows how many, since nothing else on the
// page says so: inside it, at the left of its icon, so that the icon stays
// where it is (src/TopBar.tsx).
export function FilterButton({ filters, onFilters }: { filters: Filters; onFilters: (filters: Filters) => void }) {
  const [open, setOpen] = useState(false);
  const count = filterCount(filters);
  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label={count === 0 ? 'Filters' : `Filters, ${count} set`}
        title={count === 0 ? 'Filters' : `Filters: ${count} set`}
        className={`flex shrink-0 items-center gap-1.5 max-md:-mr-2 ${BAR_ICON_BUTTON} ${count === 0 ? '' : 'bg-stone-200 text-stone-900 dark:bg-stone-800 dark:text-stone-100'}`}
      >
        {count > 0 && (
          <span aria-hidden className="text-sm font-semibold tabular-nums">
            {count}
          </span>
        )}
        <AdjustmentsHorizontalIcon className="size-5" aria-hidden />
      </button>
      <FilterDialog
        open={open}
        filters={filters}
        onApply={(set) => {
          setOpen(false);
          onFilters(set);
        }}
        onClose={() => setOpen(false)}
      />
    </>
  );
}

// The labels of the menus, for the values srl lists and those of is:.
const LABELS: Record<Exclude<Menu, 'leech' | 'id'>, string> = {
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

// The menus, set to the page's filters, APPLIED, each time it opens: what
// they are set to counts only once applied. What they offer is read from
// srl when it opens, and kept as everything read is (src/main.tsx), so it
// is the same as Add reads. The menus wait for all of it, shown as one
// skeleton until then, as Add's form does; the first error is shown in
// their place.
function FilterDialog({
  open,
  filters: applied,
  onApply,
  onClose,
}: {
  open: boolean;
  filters: Filters;
  onApply: (filters: Filters) => void;
  onClose: () => void;
}) {
  const [filters, setFilters] = useState<Filters>(applied);
  useEffect(() => {
    if (open) setFilters(applied);
  }, [open, applied]);
  const collections = useQuery({ ...queries.collections(), enabled: open, select: inNames });
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
  const applyFilters = () => onApply({ ...filters, id });

  // Apply, at the right of Cancel; disabled until the menus are shown, and
  // while the ID can't be one. Clear, far at their left, is shown while any
  // filter is set, applied or not.
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
    in: collections.data,
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
        filterCount(filters) > 0 && (
          <button onClick={() => setFilters(NO_FILTERS)} className={`px-4 py-2 text-sm ${DANGER_BUTTON}`}>
            Clear
          </button>
        )
      }
      onClose={onClose}
    >
      <div className="grid gap-4 sm:grid-cols-2">
        {(Object.keys(LABELS) as Exclude<Menu, 'leech' | 'id'>[]).map((name) => (
          <label key={name} className="flex flex-col gap-1.5 text-sm text-stone-500">
            {LABELS[name]}
            <Select value={filters[name]} options={options[name]} onChange={(value) => set(name, value)} />
          </label>
        ))}
        {/* The search box searches text only: an ID, of an item (all its
            cards) or of a card, is typed here. Enter applies. */}
        <label className="flex flex-col gap-1.5 text-sm text-stone-500">
          ID
          <input
            value={filters.id ?? ''}
            onChange={(event) => set('id', event.target.value === '' ? null : event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && idValid) applyFilters();
            }}
            placeholder="Of an item or a card"
            aria-invalid={!idValid}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            className="h-10 w-full rounded-lg border border-stone-300 bg-white px-2 font-mono text-base text-stone-900 placeholder:font-sans placeholder:text-stone-400 aria-[invalid=true]:border-rose-500 dark:border-stone-700 dark:bg-stone-900 dark:text-stone-100"
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

// The filters while srl reads what they offer: the menus, the ID and the
// checkbox, as grey bars.
function FiltersSkeleton() {
  return (
    <Loading label="Reading the collections, sources and kinds" className="flex flex-col gap-4">
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
