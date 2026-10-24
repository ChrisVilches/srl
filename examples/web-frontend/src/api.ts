// The server's API (server/server.js).

// What `srl next --json` prints (docs/frontends.md, "What a frontend gets"):
// the card, its state, and its item's note and context.
export type Card = {
  id: string;
  prompt: string;
  prompt_revealed: string | null;
  answer: string | null;
  state: 'new' | 'due';
  path: string;
  see: string | null;
  context: string | null;
};

export type Rating = 1 | 2 | 3 | 4;

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
};
