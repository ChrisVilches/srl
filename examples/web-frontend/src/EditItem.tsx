import { useEffect, useRef, useState } from 'react';

import { api, fieldLabel, type KindDescription } from './api';
import { DraftCheckbox, ErrorView, Label, Loading, MAIN_BUTTON, QUIET_BUTTON, Skeleton } from './CardParts';
import { Modal, ResultBox } from './Modal';

// Editing an item of any source: its fields and whether it is a draft, as
// srl show --json gives them, written back whole with srl edit --stdin. The
// page knows no source: srl finds the item by its ID and its source writes
// it, so an item kept in a database is edited as a note's item is.

// What was read, with the ID of the item it was read for, so that the form
// never shows an item read for another ID.
type Loaded =
  | { kind: 'loading' }
  | { kind: 'ready'; id: string; item: Record<string, unknown> }
  | { kind: 'error'; id: string; message: string };

// What became of the last save: shown in a modal until it is closed. Save
// and Reset wait until then, so they stay dimmed under it.
type Saving =
  | { kind: 'idle' }
  | { kind: 'busy' }
  | { kind: 'saved' }
  | { kind: 'failed'; title: string; message: string };

// Field names are uppercase (docs/items.md, "The item object").
const isField = (key: string) => /^[A-Z][A-Z0-9_]*$/.test(key);

// ON_SAVED is called with the item's ID once srl has written it, to show its
// cards anew; the form stays, and what became of the save is shown in a
// modal, as Add shows what it gave. Reset puts back the item as
// srl last had it: as read, or as last saved.
//
// While ID is null, the item isn't known yet, as while Browse finds the
// cards: the form waits, and reads the item once given its ID. The skeleton
// stays the same one throughout, so that its pulse doesn't start over. Only
// the item read for the ID given is shown: a read for an earlier ID that
// ends later is dropped.
export function EditItem({ id, onSaved }: { id: string | null; onSaved: (id: string) => Promise<void> }) {
  const [loaded, setLoaded] = useState<Loaded>({ kind: 'loading' });
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [draft, setDraft] = useState(false);
  const [saving, setSaving] = useState<Saving>({ kind: 'idle' });
  // What each field of the item's kind is shown as (see fieldLabel). A field
  // not among them, or any field if srl couldn't describe the kinds, is shown
  // by its name: labels only inform, so they never stop the form.
  const [labels, setLabels] = useState<Record<string, string>>({});

  // Shows the fields of ITEM, as srl has it.
  const fill = (item: Record<string, unknown>) => {
    setTexts(Object.fromEntries(Object.entries(item).filter(([key]) => isField(key)).map(([key, value]) => [key, String(value)])));
    setDraft(item.draft === true);
  };

  // Counts the calls of load, so that a read is dropped once load has been
  // called again, for another ID or none.
  const reads = useRef(0);

  const load = () => {
    const read = ++reads.current;
    if (id === null) return;
    setLoaded({ kind: 'loading' });
    setSaving({ kind: 'idle' });
    Promise.all([api.item(id), api.kinds().catch((): KindDescription[] => [])]).then(
      ([item, kinds]) => {
        if (read !== reads.current) return;
        const fields = kinds.find((k) => k.name === item.kind)?.fields ?? [];
        setLabels(Object.fromEntries(fields.map((field) => [field.name, fieldLabel(field)])));
        setLoaded({ kind: 'ready', id, item });
        fill(item);
      },
      (error: Error) => {
        if (read === reads.current) setLoaded({ kind: 'error', id, message: error.message });
      },
    );
  };
  useEffect(load, [id]);

  if (id === null || loaded.kind === 'loading' || loaded.id !== id) return <ItemSkeleton />;
  if (loaded.kind === 'error') return <ErrorView message={loaded.message} onRetry={load} />;

  const save = () => {
    // The whole item: its entries as read, the fields as edited. The keys
    // only srl gives are ignored by srl edit.
    const item = { ...loaded.item, ...texts, draft };
    setSaving({ kind: 'busy' });
    api.edit(item).then(
      () => {
        setLoaded({ kind: 'ready', id, item });
        return onSaved(id).then(
          () => setSaving({ kind: 'saved' }),
          (error: Error) =>
            setSaving({ kind: 'failed', title: 'Saved, but srl could not find the cards again', message: error.message }),
        );
      },
      (error: Error) => setSaving({ kind: 'failed', title: 'srl did not save the item', message: error.message }),
    );
  };

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-stone-500">
        Item <span className="font-mono">{String(loaded.item.id)}</span> of <span className="font-mono">{String(loaded.item.address)}</span>{' '}
        (source {String(loaded.item.source)}).
      </p>
      {Object.entries(texts).map(([key, text]) => (
        <label key={key} className="flex flex-col gap-1">
          <Label>{labels[key] ?? key}</Label>
          <textarea
            value={text}
            onChange={(event) => setTexts((t) => ({ ...t, [key]: event.target.value }))}
            rows={Math.min(12, Math.max(2, text.split('\n').length + 1))}
            className="rounded-lg border border-stone-300 bg-white p-2 font-mono text-sm dark:border-stone-700 dark:bg-stone-900"
          />
        </label>
      ))}
      <DraftCheckbox checked={draft} onChange={setDraft} />
      <Modal
        open={saving.kind === 'saved' || saving.kind === 'failed'}
        title={saving.kind === 'failed' ? saving.title : 'Saved'}
        onClose={() => setSaving({ kind: 'idle' })}
      >
        {saving.kind === 'saved' && (
          <ResultBox tone="ok" mono>
            {String(loaded.item.id)}
          </ResultBox>
        )}
        {saving.kind === 'failed' && <ResultBox tone="error">{saving.message}</ResultBox>}
      </Modal>
      <div className="flex gap-2">
        <button onClick={save} disabled={saving.kind !== 'idle'} className={`px-3 py-2 text-sm ${MAIN_BUTTON}`}>
          Save
        </button>
        <button
          onClick={() => {
            fill(loaded.item);
            setSaving({ kind: 'idle' });
          }}
          disabled={saving.kind !== 'idle'}
          title="Put back the item as srl has it"
          className={`text-sm ${QUIET_BUTTON}`}
        >
          Reset
        </button>
      </div>
    </div>
  );
}

// The form while srl reads the item: the item's line, two fields, Draft
// and the buttons.
function ItemSkeleton() {
  return (
    <Loading label="Reading the item" className="flex flex-col gap-4">
      <Skeleton className="h-4 w-80 max-w-full" />
      {['h-14', 'h-24'].map((height) => (
        <div key={height}>
          <Skeleton className="mb-2 h-3 w-16" />
          <Skeleton className={height} />
        </div>
      ))}
      <Skeleton className="h-4 w-16" />
      <div className="flex gap-2">
        <Skeleton className="h-9 w-16" />
        <Skeleton className="h-9 w-16" />
      </div>
    </Loading>
  );
}
