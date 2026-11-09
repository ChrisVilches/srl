import { useQuery } from '@tanstack/react-query';
import { useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';

import { api, fieldLabel, type KindDescription, type NewItem, type SourceDescription } from './api';
import { DraftCheckbox, ErrorView, Key, KindBadge, Label, Loading, MAIN_BUTTON, OUTLINE_BUTTON, QUIET_BUTTON, Skeleton, WITH_KEY } from './CardParts';
import { Modal, ResultBox, UnsavedGuard } from './Modal';
import { TopBar } from './TopBar';

// Adding items, one at a time: a kind from `srl kinds`, its fields, and the
// collection to add it to, of any source, picked from
// `srl collections`, or a new one where `srl sources` says one may be
// created; its source writes the item its own way. Its context, if any, is
// a note picked from `srl notes`, never the collection. srl checks the item, with
// `srl add --dry-run` (Check) and when it is added: the page knows no rule of
// any kind, and shows what srl says.

type Store =
  | { kind: 'loading' }
  | { kind: 'ready'; kinds: KindDescription[]; collections: string[]; notes: string[]; sources: SourceDescription[] }
  | { kind: 'error'; message: string };

// What the last Check or Add gave.
type Result =
  | { kind: 'none' }
  | { kind: 'checked'; text: string }
  | { kind: 'added'; ids: string[]; collection: string }
  | { kind: 'error'; message: string };

// Where the collection last added to is remembered, in this browser, for
// this address. It is offered again only while srl collections still lists
// it.
const LAST_COLLECTION = 'srl-web:last-collection';

function rememberedCollection(collections: string[]): string | null {
  try {
    const remembered = localStorage.getItem(LAST_COLLECTION);
    return remembered !== null && collections.includes(remembered) ? remembered : null;
  } catch {
    return null;
  }
}

function rememberCollection(collection: string) {
  try {
    localStorage.setItem(LAST_COLLECTION, collection);
  } catch {
    // Not remembered: the next visit starts without a collection.
  }
}

// What the form offers, as srl lists it: read anew after each change to the
// store, as when an item added makes a new collection (src/main.tsx).
const addQuery = {
  queryKey: ['add'],
  queryFn: async () => {
    const [kinds, collections, notes, sources] = await Promise.all([
      api.kinds(),
      api.collections(),
      api.notes(),
      api.sources(),
    ]);
    return { kinds, collections, notes, sources };
  },
};

export function Add() {
  const read = useQuery(addQuery);
  const store: Store = read.isSuccess
    ? { kind: 'ready', ...read.data }
    : read.isError && !read.isFetching
      ? { kind: 'error', message: read.error.message }
      : { kind: 'loading' };
  const [kindName, setKindName] = useState('qa');
  // Field texts by name, kept when the kind changes, so that a field of the
  // same name keeps its text.
  const [texts, setTexts] = useState<Record<string, string>>({});
  // The collection chosen, or, until one is, the one last added to while
  // srl still lists it.
  const [chosen, setCollection] = useState<string | null | undefined>(undefined);
  const [see, setSee] = useState<string | null>(null);
  const [draft, setDraft] = useState(false);
  const [result, setResult] = useState<Result>({ kind: 'none' });
  const [busy, setBusy] = useState(false);
  // The buttons wait while srl works and while its answer is shown, so they
  // stay dimmed under the answer's modal instead of flashing back between.
  const waiting = busy || result.kind !== 'none';

  const kinds = store.kind === 'ready' ? store.kinds : [];
  const collections = store.kind === 'ready' ? store.collections : [];
  const collection = chosen !== undefined ? chosen : rememberedCollection(collections);
  const notes = store.kind === 'ready' ? store.notes : [];
  const sources = store.kind === 'ready' ? store.sources : [];
  const kind = kinds.find((k) => k.name === kindName) ?? kinds[0];
  // A field of the kind holds text: the item would be lost by leaving.
  const unsaved = kind !== undefined && kind.fields.some((field) => (texts[field.name] ?? '').trim() !== '');

  // The item for srl add: `kind` unless it is qa (the default), `see` and
  // `draft` when set, and the fields of the kind that aren't blank.
  const item = (): NewItem => {
    const made: NewItem = {};
    if (kind.name !== 'qa') made.kind = kind.name;
    if (see !== null) made.see = see;
    if (draft) made.draft = true;
    for (const field of kind.fields) {
      const text = texts[field.name] ?? '';
      if (text.trim() !== '') made[field.name] = text;
    }
    return made;
  };

  const run = async (action: () => Promise<Result>) => {
    setBusy(true);
    try {
      setResult(await action());
    } catch (error) {
      setResult({ kind: 'error', message: (error as Error).message });
    } finally {
      setBusy(false);
    }
  };

  const check = () => {
    if (collection === null || waiting) return;
    run(async () => ({ kind: 'checked', text: await api.check(collection, item()) }));
  };

  // Once added, the fields are emptied for the next item; the kind, the
  // collections and draft stay.
  const add = () => {
    if (collection === null || waiting) return;
    run(async () => {
      const ids = await api.add(collection, item());
      rememberCollection(collection);
      setTexts((t) => Object.fromEntries(Object.entries(t).filter(([name]) => !kind.fields.some((f) => f.name === name))));
      return { kind: 'added', ids, collection };
    });
  };

  // Ctrl+Enter (or ⌘+Enter) adds, from anywhere in the form.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      add();
    }
  };

  return (
    <div className="flex min-h-screen flex-col">
      <TopBar view="add">{null}</TopBar>
      <UnsavedGuard unsaved={unsaved} what="The item being written isn't added yet, and leaving Add loses it." />
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 py-8 sm:px-6">
        {store.kind === 'loading' && <FormSkeleton />}
        {store.kind === 'error' && <ErrorView message={store.message} onRetry={() => read.refetch()} />}
        {store.kind === 'ready' && kind !== undefined && (
          <div className="flex flex-col gap-6" onKeyDown={onKeyDown}>
            <Section label="Kind">
              <div className="flex flex-wrap items-center gap-3">
                <select
                  value={kind.name}
                  onChange={(event) => setKindName(event.target.value)}
                  className={`${INPUT} w-auto font-mono`}
                >
                  {kinds.map((k) => (
                    <option key={k.name} value={k.name}>
                      {k.name}
                    </option>
                  ))}
                </select>
                <span className="text-sm text-stone-500">{kind.description}</span>
              </div>
            </Section>

            {kind.fields.map((field) => (
              // The kind's description of the field is its placeholder: there
              // while the box is empty, out of the way once it is written in.
              <Section key={field.name} label={fieldLabel(field)}>
                <textarea
                  value={texts[field.name] ?? ''}
                  placeholder={field.description}
                  onChange={(event) => setTexts((t) => ({ ...t, [field.name]: event.target.value }))}
                  rows={3}
                  spellCheck={false}
                  className={`${INPUT} resize-y`}
                />
              </Section>
            ))}

            <Section
              label="Collection"
              hint="Where the item is added: an existing collection of any source, such as a JSONL or XML file or a database table, or a new one, typed as its name, where a source of the store can create it (srl sources)."
            >
              <PathPicker
                paths={collections}
                value={collection}
                onChange={setCollection}
                placeholder={
                  collections.length === 0 ? 'Type the name of a new collection…' : 'Find a collection, or type a new one…'
                }
                creatable={(collection) => creatable(sources, collection)}
              />
            </Section>

            <Section
              label="Context"
              hint={'Optional: the note shown for context along with the answer (the item\'s "see" entry), one of your study notes (srl notes). Default: no context.'}
            >
              <PathPicker
                paths={notes}
                value={see}
                onChange={setSee}
                placeholder={notes.length === 0 ? 'The store has no notes' : 'Find a note…'}
                clearable
              />
            </Section>

            <DraftCheckbox checked={draft} onChange={setDraft} />

            <div className="flex flex-wrap items-center gap-2">
              <button
                onClick={add}
                disabled={collection === null || waiting}
                className={`px-5 py-2.5 ${WITH_KEY} ${MAIN_BUTTON}`}
              >
                Add <Key>ctrl+enter</Key>
              </button>
              <button onClick={check} disabled={collection === null || waiting} className={OUTLINE_BUTTON}>
                Check
              </button>
            </div>

            <ResultModal result={result} kind={kind.name} onClose={() => setResult({ kind: 'none' })} />
          </div>
        )}
      </main>
    </div>
  );
}

// The form while srl reads the kinds, collections, notes and sources: the kind,
// two fields, the collection and context with their hints, Draft and the
// buttons, as laid out once they come.
function FormSkeleton() {
  const label = <Skeleton className="mb-2 h-3 w-20" />;
  return (
    <Loading label="Reading the kinds and collections" className="flex flex-col gap-6">
      <section>
        {label}
        <Skeleton className="h-9" />
        <Skeleton className="mt-3 h-4 w-80 max-w-full" />
      </section>
      {[0, 1].map((i) => (
        <section key={i}>
          {label}
          <Skeleton className="h-20" />
        </section>
      ))}
      {['w-1/2', 'w-1/6'].map((rest) => (
        <section key={rest}>
          {label}
          <Skeleton className="h-4" />
          <Skeleton className={`mt-1 mb-2 h-4 ${rest}`} />
          <Skeleton className="h-9" />
        </section>
      ))}
      <Skeleton className="h-4 w-16" />
      <div className="flex gap-2">
        <Skeleton className="h-11 w-36" />
        <Skeleton className="h-9 w-20" />
      </div>
    </Loading>
  );
}

const INPUT =
  'w-full rounded-md border border-stone-300 bg-white px-3 py-2 text-sm outline-none focus:border-stone-500 dark:border-stone-700 dark:bg-stone-900 dark:focus:border-stone-400';

function Section({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <section>
      <Label>{label}</Label>
      {hint && <p className="-mt-1 mb-2 text-sm text-stone-500">{hint}</p>}
      {children}
    </section>
  );
}

// What the last Check or Add gave, in a modal, closed to go on writing. The
// browser focuses its Close button, so Enter, space or Escape closes it,
// and gives the focus back to where it was in the form.
function ResultModal({ result, kind, onClose }: { result: Result; kind: string; onClose: () => void }) {
  const title = {
    none: '',
    checked: 'Valid. srl would add:',
    added: result.kind === 'added' ? `Added to ${result.collection}` : '',
    error: 'srl refused the item',
  }[result.kind];
  return (
    <Modal open={result.kind !== 'none'} title={title} onClose={onClose}>
      {result.kind === 'checked' && (
        <ResultBox tone="ok" mono>
          {result.text}
        </ResultBox>
      )}
      {result.kind === 'added' && (
        <ResultBox tone="ok">
          <div className="flex flex-wrap items-center gap-2">
            {result.ids.map((id) => (
              <code key={id} className="font-mono">
                {id}
              </code>
            ))}
            <KindBadge kind={kind} />
          </div>
        </ResultBox>
      )}
      {result.kind === 'error' && <ResultBox tone="error">{result.message}</ResultBox>}
    </Modal>
  );
}

// How many matching collections the picker shows.
const SHOWN = 50;

// The paths that have the letters of QUERY in order, ignoring case,
// best first: the fewer letters between the matched ones, the better, then
// the shorter path. Every path matches an empty query, in order.
function fuzzy(paths: string[], query: string): string[] {
  const q = query.toLowerCase().replace(/\s+/g, '');
  if (q === '') return paths;
  const scored: [number, string][] = [];
  for (const path of paths) {
    const lower = path.toLowerCase();
    let at = -1;
    let gaps = 0;
    for (const char of q) {
      const next = lower.indexOf(char, at + 1);
      if (next === -1) {
        at = -2;
        break;
      }
      if (at >= 0) gaps += next - at - 1;
      at = next;
    }
    if (at !== -2) scored.push([gaps, path]);
  }
  return scored.sort((a, b) => a[0] - b[0] || a[1].length - b[1].length).map(([, path]) => path);
}

// A path chosen from PATHS, the collections `srl collections` lists or the
// notes `srl notes` lists, by typing part of it. Only a listed one can be
// chosen, and, with CREATABLE, a new one. With CLEARABLE, the choice can be
// taken back.
function PathPicker({
  paths,
  value,
  onChange,
  placeholder,
  clearable,
  creatable,
}: {
  paths: string[];
  value: string | null;
  onChange: (path: string | null) => void;
  placeholder: string;
  clearable?: boolean;
  // Whether a new collection may have the name that is typed: then it
  // is offered too, after the collections that match.
  creatable?: (path: string) => boolean;
}) {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const matches = useMemo(() => fuzzy(paths, query).slice(0, SHOWN), [paths, query]);
  const typed = query.trim();
  const fresh = typed !== '' && !paths.includes(typed) && creatable?.(typed) === true ? typed : null;
  const options = fresh === null ? matches : [...matches, fresh];

  const choose = (path: string) => {
    onChange(path);
    setOpen(false);
    input.current?.blur();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const move = event.key === 'ArrowDown' ? 1 : -1;
      setAt((i) => Math.max(0, Math.min(i + move, options.length - 1)));
    } else if (event.key === 'Enter' && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      if (options[at] !== undefined) choose(options[at]);
    } else if (event.key === 'Escape') {
      setOpen(false);
      input.current?.blur();
    }
  };

  return (
    <div className="relative">
      <div className="flex gap-2">
        <input
          ref={input}
          value={open ? query : (value ?? '')}
          onChange={(event) => {
            setQuery(event.target.value);
            setAt(0);
          }}
          onFocus={() => {
            setQuery('');
            setAt(0);
            setOpen(true);
          }}
          onBlur={() => setOpen(false)}
          onKeyDown={onKeyDown}
          placeholder={open ? 'Type part of a name' : placeholder}
          spellCheck={false}
          className={`${INPUT} font-mono`}
        />
        {clearable && value !== null && (
          <button onClick={() => onChange(null)} className={`text-sm ${QUIET_BUTTON}`}>
            Clear
          </button>
        )}
      </div>
      {open && (
        <ul className="absolute z-10 mt-1 max-h-72 w-full overflow-y-auto rounded-md border border-stone-200 bg-white py-1 shadow-lg dark:border-stone-700 dark:bg-stone-900">
          {options.length === 0 && <li className="px-3 py-1.5 text-sm text-stone-400">No collection matches.</li>}
          {options.map((path, i) => (
            <li
              key={path}
              // Before the input's blur, which closes the list.
              onMouseDown={(event) => {
                event.preventDefault();
                choose(path);
              }}
              onMouseEnter={() => setAt(i)}
              className={`cursor-pointer px-3 py-1.5 font-mono text-sm ${i === at ? 'bg-stone-100 dark:bg-stone-800' : ''}`}
            >
              {path === fresh ? (
                <>
                  <span className="font-sans text-stone-500">New collection: </span>
                  {path}
                </>
              ) : (
                path
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// Whether a source of the store can create COLLECTION, as
// `srl add` decides it (docs/sources.md, "Collections"): exactly one source
// whose `creates.include` globs (`srl sources --json`) match its name
// and whose `creates.exclude` don't. srl checks it again, with the format
// the extension names, when the item is checked or added.
function creatable(sources: SourceDescription[], collection: string): boolean {
  if (collection.split('/').some((part) => part === '' || part.startsWith('.'))) return false;
  const accepting = sources.filter((source) => {
    const { include, exclude } = source.creates;
    return include.some((glob) => globMatches(glob, collection)) && !exclude.some((glob) => globMatches(glob, collection));
  });
  return accepting.length === 1;
}

// Whether `glob` matches `path` as srl matches globs: `*` within one folder,
// `**` across folders. Only `*`, `**` and `?` are understood; anything else
// is matched as written, and srl has the last word.
function globMatches(glob: string, path: string): boolean {
  const pattern = glob
    .split(/(\*\*\/|\*\*|\*|\?)/)
    .map((part) => {
      switch (part) {
        case '**/':
          return '(?:.*/)?';
        case '**':
          return '.*';
        case '*':
          return '[^/]*';
        case '?':
          return '[^/]';
        default:
          return part.replace(/[.+^${}()|[\]\\]/g, '\\$&');
      }
    })
    .join('');
  return new RegExp(`^${pattern}$`).test(path);
}
