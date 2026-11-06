// The server's API (server/server.js).

// A card (docs/cards.md, "Format") with its item's kind, address and context:
// what /api/card gives the card browser.
export type ShownCard = {
  id: string;
  prompt: string;
  prompt_revealed: string | null;
  answer: string | null;
  kind: string;
  address: string;
  // The note the item names as its context, and the note's text: null when
  // it is not UTF-8 or holds nothing but blank space.
  see: string | null;
  context: string | null;
};

// What `srl next --json` prints (docs/frontends.md, "What a frontend gets"):
// the same, with where the card is in its schedule and whether it is a leech.
export type Card = ShownCard & { schedule: 'new' | 'due'; leech: boolean };

// A card as `srl find --json` lists it (docs/cli.md, "Items and cards"),
// with its item's kind and see, and the first line of its first field.
export type FoundCard = {
  id: string;
  item: string;
  address: string;
  source: string;
  status: 'active' | 'draft' | 'suspended';
  schedule: 'new' | 'due' | 'young' | 'mature';
  leech: boolean;
  due: string | null;
  // Why the card is suspended, if it was suspended with a reason.
  suspend_reason: string | null;
  kind: string;
  see: string | null;
  excerpt: string;
};

export type Rating = 1 | 2 | 3 | 4;

// An event of the log, as stored (docs/log.md, "Events"). A move belongs to
// the card it moved history to.
export type LogEvent = { t: string } & (
  | { type: 'review'; card: string; rating: Rating; response: string | null; by: 'human' | 'agent'; hash: string }
  | { type: 'reset'; card: string }
  | { type: 'suspend'; card: string; reason: string | null }
  | { type: 'unsuspend'; card: string }
  | { type: 'move'; from: string; to: string }
);

// A kind as `srl kinds --json` describes it (docs/cards.md, "Describing a
// kind"): what it is for, and its fields, each with its label for people
// if the kind gives one.
export type KindDescription = {
  name: string;
  description: string;
  fields: FieldDescription[];
};

export type FieldDescription = { name: string; label: string | null; description: string };

// What a field is shown as: its label, or its name when it has none.
export const fieldLabel = (field: FieldDescription) => field.label ?? field.name;

// A source of the store, as `srl sources --json` describes it
// (docs/sources.md): `srl add` may create a collection whose address a
// glob of `include` matches and none of `exclude` does.
export type SourceDescription = {
  name: string;
  driver: string;
  description: string;
  creates: { include: string[]; exclude: string[] };
};

// An item object for `srl add --stdin` (docs/items.md, "The item object"): its header
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
  // Whether srl-web was started with --on-change.
  hasOnChange: async () => (await call<{ on_change: boolean }>('/api/config')).on_change,
  onChangeStatus: () => call<OnChangeStatus>('/api/on-change'),
  next: async (selector: string, signal: AbortSignal) =>
    (await call<{ card: Card | null }>(`/api/next?${new URLSearchParams({ sel: selector })}`, undefined, signal)).card,
  review: (card: string, rating: Rating) => change<object>('/api/review', { card, rating }),
  // An empty REASON is no reason.
  suspend: (card: string, reason: string) => change<object>('/api/suspend', { card, reason }),
  find: async (selector: string) =>
    (await call<{ cards: FoundCard[] }>(`/api/find?${new URLSearchParams({ sel: selector })}`)).cards,
  // Makes the card, running its kind program. Aborting SIGNAL stops srl.
  card: async (id: string, signal: AbortSignal) =>
    (await call<{ card: ShownCard }>(`/api/card?${new URLSearchParams({ id })}`, undefined, signal)).card,
  // The card's log events, in log order. Aborting SIGNAL stops srl.
  log: async (id: string, signal: AbortSignal) =>
    (await call<{ events: LogEvent[] }>(`/api/log?${new URLSearchParams({ id })}`, undefined, signal)).events,
  kinds: async () => (await call<{ kinds: KindDescription[] }>('/api/kinds')).kinds,
  sources: async () => (await call<{ sources: SourceDescription[] }>('/api/sources')).sources,
  collections: async () => (await call<{ collections: string[] }>('/api/collections')).collections,
  // The notes: the files an item may name with `see`, as its context.
  notes: async () => (await call<{ notes: string[] }>('/api/notes')).notes,
  // The item as its source would add it to the collection at ADDRESS,
  // adding nothing.
  check: async (address: string, item: NewItem) =>
    (await call<{ text: string }>('/api/add', { address, item, dry_run: true })).text,
  // Adds the item to the collection at ADDRESS, and gives its new IDs.
  add: async (address: string, item: NewItem) =>
    (await change<{ ids: string[] }>('/api/add', { address, item })).ids,
  // The item of ID as srl show --json prints it: its entries, fields and the
  // keys only srl gives, which srl edit ignores.
  item: async (id: string) =>
    (await call<{ item: Record<string, unknown> }>(`/api/item?${new URLSearchParams({ id })}`)).item,
  // Replaces the item of the object's id with it, whatever its source.
  edit: async (item: Record<string, unknown>) => (await change<{ ids: string[] }>('/api/edit', { item })).ids,
};
