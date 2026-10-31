// The server's API (server/server.js).

// A card (docs/cards.md, "Format") with its item's kind, note and context:
// what /api/card gives the card browser.
export type ShownCard = {
  id: string;
  prompt: string;
  prompt_revealed: string | null;
  answer: string | null;
  kind: string;
  path: string;
  see: string | null;
  context: string | null;
  context_format: string | null;
};

// What `srl next --json` prints (docs/frontends.md, "What a frontend gets"):
// the same, with the card's state.
export type Card = ShownCard & { state: 'new' | 'due' };

// A card as `srl find --json` lists it (docs/cli.md, "Items and cards"),
// with its item's kind and the first line of its first field.
export type FoundCard = {
  id: string;
  item: string;
  path: string;
  line: number;
  status: 'active' | 'draft' | 'suspended';
  state: string;
  due: string | null;
  kind: string;
  excerpt: string;
};

export type Rating = 1 | 2 | 3 | 4;

// A kind as `srl kinds --json` describes it (docs/cards.md, "Describing a
// kind"): what it is for, and its fields.
export type KindDescription = {
  name: string;
  description: string;
  fields: { name: string; description: string }[];
};

// An item object for `srl add --stdin` (docs/notes.md, "The item object"): its header
// entries kind, see and draft, and its fields.
export type NewItem = Record<string, string | boolean>;

// How the --on-change program's last run went: error is what it printed to
// stderr, or null when it succeeded. busy is true until every change so far
// has had its run.
export type OnChangeStatus = { busy: boolean; last: { seq: number; error: string | null } | null };

// Called after each change the page makes to the store.
const changeListeners = new Set<() => void>();

// Calls LISTENER after each change the page makes to the store: a review, a
// suspension, an item added. Returns a function that stops it.
export function onStoreChange(listener: () => void) {
  changeListeners.add(listener);
  return () => {
    changeListeners.delete(listener);
  };
}

// Calls the route, then the change listeners.
async function change<T>(path: string, body: object): Promise<T> {
  const value = await call<T>(path, body);
  changeListeners.forEach((listener) => listener());
  return value;
}

async function call<T>(path: string, body?: object, signal?: AbortSignal): Promise<T> {
  const response = await fetch(
    path,
    body === undefined
      ? { signal }
      : {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(body),
          signal,
        },
  );
  const value = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(value.error ?? `${path} failed with status ${response.status}`);
  }
  return value as T;
}

export const api = {
  // The selector srl-web was started with.
  selector: async () => (await call<{ selector: string }>('/api/config')).selector,
  // Whether srl-web was started with --on-change.
  hasOnChange: async () => (await call<{ on_change: boolean }>('/api/config')).on_change,
  onChangeStatus: () => call<OnChangeStatus>('/api/on-change'),
  next: async (selector: string) =>
    (await call<{ card: Card | null }>(`/api/next?${new URLSearchParams({ sel: selector })}`)).card,
  review: (card: string, rating: Rating) => change<object>('/api/review', { card, rating }),
  suspend: (card: string) => change<object>('/api/suspend', { card }),
  find: async (selector: string) =>
    (await call<{ cards: FoundCard[] }>(`/api/find?${new URLSearchParams({ sel: selector })}`)).cards,
  // Makes the card, running its kind program. Aborting SIGNAL stops srl.
  card: async (id: string, signal: AbortSignal) =>
    (await call<{ card: ShownCard }>(`/api/card?${new URLSearchParams({ id })}`, undefined, signal)).card,
  kinds: async () => (await call<{ kinds: KindDescription[] }>('/api/kinds')).kinds,
  notes: async () => (await call<{ notes: string[] }>('/api/notes')).notes,
  // The item as srl would write it to NOTE, writing nothing.
  check: async (note: string, item: NewItem) =>
    (await call<{ text: string }>('/api/add', { note, item, dry_run: true })).text,
  // Writes the item to the end of NOTE, and gives its new ID.
  add: async (note: string, item: NewItem) => (await change<{ ids: string[] }>('/api/add', { note, item })).ids,
};
