import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';

import { api, fieldLabel } from './api';
import { DraftCheckbox, ErrorView, Label, Loading, MAIN_BUTTON, QUIET_BUTTON, Skeleton } from './CardParts';
import { Modal, ResultBox, UnsavedGuard } from './Modal';
import { clozeShortcut } from './qaCloze';
import { queries } from './queries';

// Editing an item of any source: its fields and whether it is a draft, as
// srl show --json gives them, written back whole with srl edit --stdin. The
// page knows no source: srl finds the item by its ID and its source writes
// it, so an item kept in a database is edited as one kept in a file.

// The form's fields and Draft as edited, with the ID of the item they are
// edits of, so that they are never shown for another item.
type Edits = { id: string; texts: Record<string, string>; draft: boolean };

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
// stays the same one throughout, so that its pulse doesn't start over.
//
// The item is read as other reads are (src/queries.ts), so it is read again
// after a change to the store; the form shows it until it is edited, and
// from then on the edits, which a new read never changes.
export function EditItem({ id, onSaved }: { id: string | null; onSaved: (id: string) => Promise<void> }) {
  const queryClient = useQueryClient();
  const read = useQuery({ ...queries.item(id ?? ''), enabled: id !== null });
  // What each field of the item's kind is shown as (see fieldLabel). A field
  // not among them, or any field if srl couldn't describe the kinds, is shown
  // by its name: labels only inform, so they never stop the form.
  const kinds = useQuery(queries.kinds());
  const [edits, setEdits] = useState<Edits | null>(null);
  const [saving, setSaving] = useState<Saving>({ kind: 'idle' });

  if (id === null || kinds.isPending || (read.data === undefined && !(read.isError && !read.isFetching))) {
    return <ItemSkeleton />;
  }
  if (read.data === undefined) {
    return <ErrorView message={read.error?.message ?? ''} onRetry={() => read.refetch()} />;
  }
  const item = read.data;
  const fields = kinds.data?.find((k) => k.name === item.kind)?.fields ?? [];
  const labels = Object.fromEntries(fields.map((field) => [field.name, fieldLabel(field)]));

  // The fields and Draft shown: as edited, or else as srl has them.
  const { texts, draft } =
    edits?.id === id
      ? edits
      : {
          texts: Object.fromEntries(
            Object.entries(item).filter(([key]) => isField(key)).map(([key, value]) => [key, String(value)]),
          ),
          draft: item.draft === true,
        };
  const edit = (change: Partial<Omit<Edits, 'id'>>) => setEdits({ id, texts, draft, ...change });

  // A field or Draft differs from the item as srl has it: leaving would lose
  // the edits.
  const unsaved =
    Object.entries(texts).some(([key, text]) => text !== String(item[key])) || draft !== (item.draft === true);

  const save = () => {
    // The whole item: its entries as read, the fields as edited. The keys
    // only srl gives are ignored by srl edit.
    const saved = { ...item, ...texts, draft };
    setSaving({ kind: 'busy' });
    api.edit(saved).then(
      () => {
        // srl has it as saved, until it is read again: the form shows it.
        queryClient.setQueryData(queries.item(id).queryKey, saved);
        setEdits(null);
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
      <UnsavedGuard unsaved={unsaved} what="The edits to this item aren't saved, and leaving its Edit side loses them." />
      <p className="text-sm text-stone-500">
        Item <span className="font-mono">{String(item.id)}</span> of <span className="font-mono">{String(item.collection)}</span>{' '}
        (source {String(item.source)}).
      </p>
      {Object.entries(texts).map(([key, text]) => (
        <label key={key} className="flex flex-col gap-1">
          <Label>{labels[key] ?? key}</Label>
          <textarea
            value={text}
            onChange={(event) => edit({ texts: { ...texts, [key]: event.target.value } })}
            onKeyDown={clozeShortcut(String(item.kind), key)}
            rows={Math.min(12, Math.max(2, text.split('\n').length + 1))}
            className="rounded-lg border border-stone-300 bg-white p-2 font-mono text-sm dark:border-stone-700 dark:bg-stone-900"
          />
        </label>
      ))}
      <DraftCheckbox checked={draft} onChange={(checked) => edit({ draft: checked })} />
      <Modal
        open={saving.kind === 'saved' || saving.kind === 'failed'}
        title={saving.kind === 'failed' ? saving.title : 'Saved'}
        onClose={() => setSaving({ kind: 'idle' })}
      >
        {saving.kind === 'saved' && (
          <ResultBox tone="ok" mono>
            {String(item.id)}
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
            setEdits(null);
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
