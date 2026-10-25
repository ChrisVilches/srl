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
  next: async (selector: string) =>
    (await call<{ card: Card | null }>(`/api/next?${new URLSearchParams({ sel: selector })}`)).card,
  review: (card: string, rating: Rating) => call<object>('/api/review', { card, rating }),
  suspend: (card: string) => call<object>('/api/suspend', { card }),
  find: async (selector: string) =>
    (await call<{ cards: FoundCard[] }>(`/api/find?${new URLSearchParams({ sel: selector })}`)).cards,
  // Makes the card, running its kind program. Aborting SIGNAL stops srl.
  card: async (id: string, signal: AbortSignal) =>
    (await call<{ card: ShownCard }>(`/api/card?${new URLSearchParams({ id })}`, undefined, signal)).card,
};
