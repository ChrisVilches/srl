import { useEffect, useState } from 'react';

import { api } from './api';
import { ErrorView, Label, MAIN_BUTTON, QUIET_BUTTON } from './CardParts';

// Editing an item of any source: its fields and whether it is a draft, as
// srl show --json gives them, written back whole with srl edit --stdin. The
// page knows no source: srl finds the item by its ID and its source writes
// it, so an item kept in a database is edited as a note's item is.

type Loaded = { kind: 'loading' } | { kind: 'ready'; item: Record<string, unknown> } | { kind: 'error'; message: string };

// Field names are uppercase (docs/notes.md, "The item object").
const isField = (key: string) => /^[A-Z][A-Z0-9_]*$/.test(key);

export function EditItem({ id, onSaved, onCancel }: { id: string; onSaved: () => void; onCancel: () => void }) {
  const [loaded, setLoaded] = useState<Loaded>({ kind: 'loading' });
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [draft, setDraft] = useState(false);
  const [saving, setSaving] = useState<{ error: string | null; busy: boolean }>({ error: null, busy: false });

  const load = () => {
    setLoaded({ kind: 'loading' });
    api.item(id).then(
      (item) => {
        setLoaded({ kind: 'ready', item });
        setTexts(Object.fromEntries(Object.entries(item).filter(([key]) => isField(key)).map(([key, value]) => [key, String(value)])));
        setDraft(item.draft === true);
      },
      (error: Error) => setLoaded({ kind: 'error', message: error.message }),
    );
  };
  useEffect(load, [id]);

  if (loaded.kind === 'loading') return <p className="text-stone-400">Reading the item…</p>;
  if (loaded.kind === 'error') return <ErrorView message={loaded.message} onRetry={load} />;

  const save = () => {
    // The whole item: its entries as read, the fields as edited. The keys
    // only srl gives are ignored by srl edit.
    const item = { ...loaded.item, ...texts, draft };
    setSaving({ error: null, busy: true });
    api.edit(item).then(onSaved, (error: Error) => setSaving({ error: error.message, busy: false }));
  };

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-stone-500">
        Item <span className="font-mono">{String(loaded.item.id)}</span> of <span className="font-mono">{String(loaded.item.address)}</span>{' '}
        (source {String(loaded.item.source)}).
      </p>
      {Object.entries(texts).map(([key, text]) => (
        <label key={key} className="flex flex-col gap-1">
          <Label>{key}</Label>
          <textarea
            value={text}
            onChange={(event) => setTexts((t) => ({ ...t, [key]: event.target.value }))}
            rows={Math.min(12, Math.max(2, text.split('\n').length + 1))}
            className="rounded-lg border border-stone-300 bg-white p-2 font-mono text-sm dark:border-stone-700 dark:bg-stone-900"
          />
        </label>
      ))}
      <label className="flex items-center gap-2 text-sm">
        <input type="checkbox" checked={draft} onChange={(event) => setDraft(event.target.checked)} />
        Draft
      </label>
      {saving.error !== null && <p className="text-sm whitespace-pre-wrap text-red-600 dark:text-red-400">{saving.error}</p>}
      <div className="flex gap-2">
        <button onClick={save} disabled={saving.busy} className={`px-3 py-2 text-sm ${MAIN_BUTTON}`}>
          Save
        </button>
        <button onClick={onCancel} className={`text-sm ${QUIET_BUTTON}`}>
          Cancel
        </button>
      </div>
    </div>
  );
}
