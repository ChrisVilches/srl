import { useCallback, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';

import { api, type KindDescription, type NewItem, type SourceDescription } from './api';
import { DraftCheckbox, ErrorView, Key, KindBadge, Label, MAIN_BUTTON, Notice, OUTLINE_BUTTON, QUIET_BUTTON, WITH_KEY } from './CardParts';
import { useStartWhenShown } from './hooks';
import { Modal, ResultBox } from './Modal';
import { TopBar } from './TopBar';

// Adding items, one at a time: a kind from `srl kinds`, its fields, and the
// collection to add it to, of any source, picked from
// `srl collections`, or a new one where `srl sources` says one may be
// created; its source writes the item its own way. srl checks the item, with
// `srl add --dry-run` (Check) and when it is added: the page knows no rule of
// any kind, and shows what srl says.

type Store =
  | { kind: 'loading' }
  | { kind: 'ready'; kinds: KindDescription[]; collections: string[]; sources: SourceDescription[] }
  | { kind: 'error'; message: string };

// What the last Check or Add gave.
type Result =
  | { kind: 'none' }
  | { kind: 'checked'; text: string }
  | { kind: 'added'; ids: string[]; collection: string }
  | { kind: 'error'; message: string };

// Where the collection last added to is remembered, in this browser, for
// this address. It is offered again only while srl collections still lists
// it. The key keeps its old name, from when srl added to notes only, so that
// a choice remembered before is still found.
const LAST_COLLECTION = 'srl-web:last-note';

function rememberedCollection(collections: string[]): string | null {
  try {
    const address = localStorage.getItem(LAST_COLLECTION);
    return address !== null && collections.includes(address) ? address : null;
  } catch {
    return null;
  }
}

function rememberCollection(address: string) {
  try {
    localStorage.setItem(LAST_COLLECTION, address);
  } catch {
    // Not remembered: the next visit starts without a collection.
  }
}

export function Add({ active }: { active: boolean }) {
  const [store, setStore] = useState<Store>({ kind: 'loading' });
  const [kindName, setKindName] = useState('qa');
  // Field texts by name, kept when the kind changes, so that a field of the
  // same name keeps its text.
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [collection, setCollection] = useState<string | null>(null);
  const [see, setSee] = useState<string | null>(null);
  const [draft, setDraft] = useState(false);
  const [result, setResult] = useState<Result>({ kind: 'none' });
  const [busy, setBusy] = useState(false);
  // The buttons wait while srl works and while its answer is shown, so they
  // stay dimmed under the answer's modal instead of flashing back between.
  const waiting = busy || result.kind !== 'none';

  const load = useCallback(async () => {
    setStore({ kind: 'loading' });
    try {
      const [kinds, collections, sources] = await Promise.all([api.kinds(), api.collections(), api.sources()]);
      setStore({ kind: 'ready', kinds, collections, sources });
      setCollection((current) =>
        current !== null && collections.includes(current) ? current : rememberedCollection(collections),
      );
      setSee((current) => (current !== null && collections.includes(current) ? current : null));
    } catch (error) {
      setStore({ kind: 'error', message: (error as Error).message });
    }
  }, []);
  useStartWhenShown(active, load);

  const kinds = store.kind === 'ready' ? store.kinds : [];
  const collections = store.kind === 'ready' ? store.collections : [];
  const sources = store.kind === 'ready' ? store.sources : [];
  const kind = kinds.find((k) => k.name === kindName) ?? kinds[0];

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
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 py-8 sm:px-6">
        {store.kind === 'loading' && <Notice>Loading…</Notice>}
        {store.kind === 'error' && <ErrorView message={store.message} onRetry={load} />}
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
              <Section key={field.name} label={field.name}>
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
              hint="Where the item is added: an existing collection of any source, such as a note or a database table, or a new one, typed as its address, where a source of the store can create it (srl sources)."
            >
              <CollectionPicker
                collections={collections}
                value={collection}
                onChange={setCollection}
                placeholder={
                  collections.length === 0 ? 'Type the address of a new collection…' : 'Find a collection, or type a new one…'
                }
                creatable={(address) => creatable(sources, address)}
              />
            </Section>

            <Section
              label="Context"
              hint={'Optional: the collection shown for context along with the answer (the item\'s "see" entry). Default: same as collection.'}
            >
              {/* The input is controlled, so its placeholder follows the
                  collection chosen above like any other prop. */}
              <CollectionPicker
                collections={collections}
                value={see}
                onChange={setSee}
                placeholder={collection ?? ''}
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

// The addresses that have the letters of QUERY in order, ignoring case,
// best first: the fewer letters between the matched ones, the better, then
// the shorter address. Every address matches an empty query, in order.
function fuzzy(addresses: string[], query: string): string[] {
  const q = query.toLowerCase().replace(/\s+/g, '');
  if (q === '') return addresses;
  const scored: [number, string][] = [];
  for (const address of addresses) {
    const lower = address.toLowerCase();
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
    if (at !== -2) scored.push([gaps, address]);
  }
  return scored.sort((a, b) => a[0] - b[0] || a[1].length - b[1].length).map(([, address]) => address);
}

// A collection chosen from COLLECTIONS, the addresses `srl collections`
// lists, by typing part of its address. Only a listed one can be chosen,
// and, with CREATABLE, a new one. With CLEARABLE, the choice can be taken
// back.
function CollectionPicker({
  collections,
  value,
  onChange,
  placeholder,
  clearable,
  creatable,
}: {
  collections: string[];
  value: string | null;
  onChange: (address: string | null) => void;
  placeholder: string;
  clearable?: boolean;
  // Whether a new collection may be at an address that is typed: then it
  // is offered too, after the collections that match.
  creatable?: (address: string) => boolean;
}) {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const matches = useMemo(() => fuzzy(collections, query).slice(0, SHOWN), [collections, query]);
  const typed = query.trim();
  const fresh = typed !== '' && !collections.includes(typed) && creatable?.(typed) === true ? typed : null;
  const options = fresh === null ? matches : [...matches, fresh];

  const choose = (address: string) => {
    onChange(address);
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
          placeholder={open ? 'Type part of an address' : placeholder}
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
          {options.map((address, i) => (
            <li
              key={address}
              // Before the input's blur, which closes the list.
              onMouseDown={(event) => {
                event.preventDefault();
                choose(address);
              }}
              onMouseEnter={() => setAt(i)}
              className={`cursor-pointer px-3 py-1.5 font-mono text-sm ${i === at ? 'bg-stone-100 dark:bg-stone-800' : ''}`}
            >
              {address === fresh ? (
                <>
                  <span className="font-sans text-stone-500">New collection: </span>
                  {address}
                </>
              ) : (
                address
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// Whether a source of the store can create a collection at ADDRESS, as
// `srl add` decides it (docs/sources.md, "Addresses"): one
// mounted deepest above it, whose `creates.include` globs (`srl sources
// --json`) match the rest of the address and whose `creates.exclude` don't.
// srl checks it again when the item is checked or added.
function creatable(sources: SourceDescription[], address: string): boolean {
  if (address.split('/').some((part) => part === '' || part.startsWith('.'))) return false;
  const above = sources.filter((source) => address.startsWith(source.prefix));
  const deepest = Math.max(-1, ...above.map((source) => source.prefix.length));
  return above
    .filter((source) => source.prefix.length === deepest)
    .some((source) => {
      const rest = address.slice(source.prefix.length);
      const { include, exclude } = source.creates;
      return include.some((glob) => globMatches(glob, rest)) && !exclude.some((glob) => globMatches(glob, rest));
    });
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
