// The server's API (server/server.js): each call runs one srl command.

// A card, as in docs/cards.md.
export type Card = { id: string; prompt: string; answer: string };

// A card as `srl find --json` lists it (docs/cli.md).
export type FoundCard = {
  id: string;
  item: string;
  path: string;
  line: number;
  status: 'active' | 'draft' | 'suspended';
  state: 'new' | 'due' | 'young' | 'mature' | 'leech';
  due: string | null;
};

// An item, as `srl show --json` prints it (docs/notes.md, "Items as JSON").
export type Item = {
  id: string;
  path: string;
  line: number;
  kind: string;
  see: string | null;
  draft: boolean;
  fields: Record<string, string>;
  cards: string[];
  context: string | null;
};

export type Rating = 1 | 2 | 3 | 4;

export type Config = { selector: string; dir: string | null };

async function call<T>(path: string, body?: object): Promise<T> {
  const response = await fetch(
    path,
    body === undefined
      ? undefined
      : {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(body),
        },
  );
  const value = await response.json().catch(() => ({ error: `${path}: ${response.statusText}` }));
  if (!response.ok) {
    throw new Error(value.error ?? `${path} failed with status ${response.status}`);
  }
  return value as T;
}

const query = (params: Record<string, string>) => new URLSearchParams(params).toString();

export const api = {
  config: () => call<Config>('/api/config'),
  next: async (selector: string) =>
    (await call<{ card: Card | null }>(`/api/next?${query({ sel: selector })}`)).card,
  find: async (selector: string) =>
    (await call<{ cards: FoundCard[] }>(`/api/find?${query({ sel: selector })}`)).cards,
  item: async (id: string) => (await call<{ item: Item }>(`/api/item?${query({ id })}`)).item,
  review: (card: string, rating: Rating) => call<object>('/api/review', { card, rating }),
  suspend: (card: string) => call<{ suspended: boolean }>('/api/suspend', { card }),
};
