// The server's API (server/server.js): each call runs one srl command.

// What `srl next --json` prints (docs/cli.md, "Studying"): the card, its
// state, and its item's note and context.
export type Card = {
  id: string;
  prompt: string;
  answer: string;
  state: 'new' | 'due';
  path: string;
  see: string | null;
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
  review: (card: string, rating: Rating) => call<object>('/api/review', { card, rating }),
  suspend: (card: string) => call<{ suspended: boolean }>('/api/suspend', { card }),
};
