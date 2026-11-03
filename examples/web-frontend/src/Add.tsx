import { useCallback, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';

import { api, type KindDescription, type NewItem, type SourceDescription } from './api';
import { ErrorView, Key, KindBadge, Label, MAIN_BUTTON, Notice, OUTLINE_BUTTON, Panel, QUIET_BUTTON, WITH_KEY } from './CardParts';
import { useStartWhenShown } from './hooks';
import { TopBar } from './TopBar';

// Adding items, one at a time: a kind from `srl kinds`, its fields, and the
// note to append it to, picked from `srl collections`, or a new one where
// `srl sources` says one may be created. srl checks the item, with
// `srl add --dry-run` (Check) and when it is added: the page knows no rule of
// any kind, and shows what srl says.

type Store =
  | { kind: 'loading' }
  | { kind: 'ready'; kinds: KindDescription[]; notes: string[]; sources: SourceDescription[] }
  | { kind: 'error'; message: string };

// What the last Check or Add gave.
type Result =
  | { kind: 'none' }
  | { kind: 'checked'; text: string }
  | { kind: 'added'; ids: string[]; note: string }
  | { kind: 'error'; message: string };

// Where the note last added to is remembered, in this browser, for this
// address. It is offered again only while srl collections still lists it.
const LAST_NOTE = 'srl-web:last-note';

function rememberedNote(notes: string[]): string | null {
  try {
    const note = localStorage.getItem(LAST_NOTE);
    return note !== null && notes.includes(note) ? note : null;
  } catch {
    return null;
  }
}

function rememberNote(note: string) {
  try {
    localStorage.setItem(LAST_NOTE, note);
  } catch {
    // Not remembered: the next visit starts without a note.
  }
}

export function Add({ active }: { active: boolean }) {
  const [store, setStore] = useState<Store>({ kind: 'loading' });
  const [kindName, setKindName] = useState('qa');
  // Field texts by name, kept when the kind changes, so that a field of the
  // same name keeps its text.
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [note, setNote] = useState<string | null>(null);
  const [see, setSee] = useState<string | null>(null);
  const [draft, setDraft] = useState(false);
  const [result, setResult] = useState<Result>({ kind: 'none' });
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setStore({ kind: 'loading' });
    try {
      const [kinds, notes, sources] = await Promise.all([api.kinds(), api.collections(), api.sources()]);
      setStore({ kind: 'ready', kinds, notes, sources });
      setNote((current) => (current !== null && notes.includes(current) ? current : rememberedNote(notes)));
      setSee((current) => (current !== null && notes.includes(current) ? current : null));
    } catch (error) {
      setStore({ kind: 'error', message: (error as Error).message });
    }
  }, []);
  useStartWhenShown(active, load);

  const kinds = store.kind === 'ready' ? store.kinds : [];
  const notes = store.kind === 'ready' ? store.notes : [];
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
    if (note === null || busy) return;
    run(async () => ({ kind: 'checked', text: await api.check(note, item()) }));
  };

  // Once added, the fields are emptied for the next item; the kind, the
  // notes and draft stay.
  const add = () => {
    if (note === null || busy) return;
    run(async () => {
      const ids = await api.add(note, item());
      rememberNote(note);
      setTexts((t) => Object.fromEntries(Object.entries(t).filter(([name]) => !kind.fields.some((f) => f.name === name))));
      return { kind: 'added', ids, note };
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
              label="Note"
              hint="The note to append the item to: an existing one, or a new one, typed as its path, where a source of the store can create it (srl sources)."
            >
              <NotePicker
                notes={notes}
                value={note}
                onChange={setNote}
                placeholder={notes.length === 0 ? 'Type the path of a new note…' : 'Find a note, or type a new one…'}
                creatable={(path) => creatable(sources, path)}
              />
            </Section>

            <Section
              label="Context"
              hint="Optional: the note shown after the answer (see:), when it isn't the note above."
            >
              <NotePicker notes={notes} value={see} onChange={setSee} placeholder="The note above" clearable />
            </Section>

            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={draft} onChange={(event) => setDraft(event.target.checked)} />
              Draft: not studied until accepted (<code className="font-mono">srl accept</code>)
            </label>

            <div className="flex flex-wrap items-center gap-2">
              <button
                onClick={add}
                disabled={note === null || busy}
                className={`px-5 py-2.5 ${WITH_KEY} ${MAIN_BUTTON}`}
              >
                Add <Key>ctrl+enter</Key>
              </button>
              <button onClick={check} disabled={note === null || busy} className={`${OUTLINE_BUTTON} disabled:opacity-50`}>
                Check
              </button>
              {busy && <span className="text-sm text-stone-400">Asking srl…</span>}
            </div>

            <ResultView result={result} kind={kind.name} />
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

function ResultView({ result, kind }: { result: Result; kind: string }) {
  switch (result.kind) {
    case 'none':
      return null;
    case 'checked':
      return (
        <Panel tone="ok" title="Valid. srl would append:">
          <pre className="overflow-x-auto font-mono text-sm whitespace-pre-wrap">{result.text}</pre>
        </Panel>
      );
    case 'added':
      return (
        <Panel tone="ok" title={`Added to ${result.note}`}>
          <div className="flex flex-wrap items-center gap-2 text-sm">
            {result.ids.map((id) => (
              <code key={id} className="font-mono">
                {id}
              </code>
            ))}
            <KindBadge kind={kind} />
          </div>
        </Panel>
      );
    case 'error':
      return (
        <Panel tone="error" title="srl refused the item">
          <pre className="overflow-x-auto text-sm whitespace-pre-wrap">{result.message}</pre>
        </Panel>
      );
  }
}

// How many matching notes the picker shows.
const SHOWN = 50;

// The notes whose path has the letters of QUERY in order, ignoring case,
// best first: the fewer letters between the matched ones, the better, then
// the shorter path. Every note matches an empty query, in order.
function fuzzy(notes: string[], query: string): string[] {
  const q = query.toLowerCase().replace(/\s+/g, '');
  if (q === '') return notes;
  const scored: [number, string][] = [];
  for (const note of notes) {
    const path = note.toLowerCase();
    let at = -1;
    let gaps = 0;
    for (const char of q) {
      const next = path.indexOf(char, at + 1);
      if (next === -1) {
        at = -2;
        break;
      }
      if (at >= 0) gaps += next - at - 1;
      at = next;
    }
    if (at !== -2) scored.push([gaps, note]);
  }
  return scored.sort((a, b) => a[0] - b[0] || a[1].length - b[1].length).map(([, note]) => note);
}

// A note chosen from NOTES by typing part of its path. Only a listed note
// can be chosen. With CLEARABLE, the choice can be taken back.
function NotePicker({
  notes,
  value,
  onChange,
  placeholder,
  clearable,
  creatable,
}: {
  notes: string[];
  value: string | null;
  onChange: (note: string | null) => void;
  placeholder: string;
  clearable?: boolean;
  // Whether a new note may be at a path that is typed: then it is offered
  // too, after the notes that match.
  creatable?: (path: string) => boolean;
}) {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const matches = useMemo(() => fuzzy(notes, query).slice(0, SHOWN), [notes, query]);
  const typed = query.trim();
  const fresh = typed !== '' && !notes.includes(typed) && creatable?.(typed) === true ? typed : null;
  const options = fresh === null ? matches : [...matches, fresh];

  const choose = (note: string) => {
    onChange(note);
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
          placeholder={open ? 'Type part of a path' : placeholder}
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
          {options.length === 0 && <li className="px-3 py-1.5 text-sm text-stone-400">No note matches.</li>}
          {options.map((note, i) => (
            <li
              key={note}
              // Before the input's blur, which closes the list.
              onMouseDown={(event) => {
                event.preventDefault();
                choose(note);
              }}
              onMouseEnter={() => setAt(i)}
              className={`cursor-pointer px-3 py-1.5 font-mono text-sm ${i === at ? 'bg-stone-100 dark:bg-stone-800' : ''}`}
            >
              {note === fresh ? (
                <>
                  <span className="font-sans text-stone-500">New note: </span>
                  {note}
                </>
              ) : (
                note
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// Whether a source of the store can create a note at `path`, as `srl add`
// decides it (docs/sources.md, "Addresses"): one mounted deepest above it,
// whose `include` matches the rest of the path and whose `exclude` doesn't.
// srl checks it again when the item is checked or added.
function creatable(sources: SourceDescription[], path: string): boolean {
  if (path.split('/').some((part) => part === '' || part.startsWith('.'))) return false;
  const above = sources.filter((source) => path.startsWith(source.prefix));
  const deepest = Math.max(-1, ...above.map((source) => source.prefix.length));
  return above
    .filter((source) => source.prefix.length === deepest)
    .some((source) => {
      const rest = path.slice(source.prefix.length);
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
