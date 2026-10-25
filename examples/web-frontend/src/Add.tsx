import { useCallback, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';

import { api, type KindDescription, type NewItem } from './api';
import { ErrorView, Key, KindBadge, Label, Notice, OUTLINE_BUTTON, QUIET_BUTTON } from './CardParts';
import { useStartWhenShown } from './hooks';
import { TopBar } from './TopBar';

// Adding items, one at a time: a kind from `srl kinds`, its fields, and the
// note to append it to, picked from `srl notes`. srl checks the item, with
// `srl add --dry-run` (Check) and when it is added: the page knows no rule of
// any kind, and shows what srl says.

type Store = { kind: 'loading' } | { kind: 'ready'; kinds: KindDescription[]; notes: string[] } | { kind: 'error'; message: string };

// What the last Check or Add gave.
type Result =
  | { kind: 'none' }
  | { kind: 'checked'; text: string }
  | { kind: 'added'; ids: string[]; note: string }
  | { kind: 'error'; message: string };

// Where the note last added to is remembered, in this browser, for this
// address. It is offered again only while srl notes still lists it.
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
      const [kinds, notes] = await Promise.all([api.kinds(), api.notes()]);
      setStore({ kind: 'ready', kinds, notes });
      setNote((current) => (current !== null && notes.includes(current) ? current : rememberedNote(notes)));
      setSee((current) => (current !== null && notes.includes(current) ? current : null));
    } catch (error) {
      setStore({ kind: 'error', message: (error as Error).message });
    }
  }, []);
  useStartWhenShown(active, load);

  const kinds = store.kind === 'ready' ? store.kinds : [];
  const notes = store.kind === 'ready' ? store.notes : [];
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
              <Section key={field.name} label={field.name} hint={field.description}>
                <textarea
                  value={texts[field.name] ?? ''}
                  onChange={(event) => setTexts((t) => ({ ...t, [field.name]: event.target.value }))}
                  rows={3}
                  spellCheck={false}
                  className={`${INPUT} resize-y`}
                />
              </Section>
            ))}

            <Section
              label="Note"
              hint="The note to append the item to. Only existing notes are offered; to add to a new one, such as an inbox, create it first."
              action={
                <button onClick={load} className={`py-0.5 text-xs ${QUIET_BUTTON}`}>
                  Reload notes
                </button>
              }
            >
              {notes.length === 0 ? (
                <p className="text-sm text-stone-500">The store has no notes yet.</p>
              ) : (
                <NotePicker notes={notes} value={note} onChange={setNote} placeholder="Find a note…" />
              )}
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
                className="rounded-lg bg-stone-900 px-5 py-2.5 font-medium text-white hover:bg-stone-700 disabled:opacity-50 dark:border dark:border-stone-700 dark:bg-stone-800 dark:text-stone-100 dark:hover:bg-stone-700"
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
  action,
  children,
}: {
  label: string;
  hint?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section>
      <div className="flex items-baseline justify-between gap-2">
        <Label>{label}</Label>
        {action}
      </div>
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

function Panel({ tone, title, children }: { tone: 'ok' | 'error'; title: string; children: ReactNode }) {
  const style =
    tone === 'ok'
      ? 'bg-emerald-50 text-emerald-950 dark:bg-emerald-950/40 dark:text-emerald-100'
      : 'bg-rose-50 text-rose-950 dark:bg-rose-950/40 dark:text-rose-200';
  return (
    <div className={`rounded-lg p-4 ${style}`}>
      <div className="mb-2 text-sm font-semibold">{title}</div>
      {children}
    </div>
  );
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
}: {
  notes: string[];
  value: string | null;
  onChange: (note: string | null) => void;
  placeholder: string;
  clearable?: boolean;
}) {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const matches = useMemo(() => fuzzy(notes, query).slice(0, SHOWN), [notes, query]);

  const choose = (note: string) => {
    onChange(note);
    setOpen(false);
    input.current?.blur();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const move = event.key === 'ArrowDown' ? 1 : -1;
      setAt((i) => Math.max(0, Math.min(i + move, matches.length - 1)));
    } else if (event.key === 'Enter' && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      if (matches[at] !== undefined) choose(matches[at]);
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
          {matches.length === 0 && <li className="px-3 py-1.5 text-sm text-stone-400">No note matches.</li>}
          {matches.map((note, i) => (
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
              {note}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
