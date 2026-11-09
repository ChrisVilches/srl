import { queryOptions } from '@tanstack/react-query';

import { api } from './api';
import type { Submitted } from './hooks';

// What the views read from srl, for TanStack Query's useQuery (src/main.tsx
// sets how long it is kept): each read's key, what is asked for, and how to
// read it, in one place, so that two views asking the same thing share it.
// A view reads with useQuery(queries.X(…)) and never sets what it shows from
// an answer itself (README.md, "What srl answers").
export const queries = {
  // The cards of a selector, for one submission of it: submitting it again
  // finds them anew.
  find: ({ selector, n }: Submitted) =>
    queryOptions({ queryKey: ['find', selector, n], queryFn: () => api.find(selector) }),

  // Card ID made from its item, by its kind program, which may take long or
  // cost something (an LLM): made only when asked, so not again after a
  // change to the store (ONLY_WHEN_ASKED). Of one submission of the
  // selector, as the cards found. Stopped when no view shows it any more.
  made: (n: number, id: string) =>
    queryOptions({
      queryKey: ['made', n, id],
      queryFn: ({ signal }) => api.card(id, signal),
      staleTime: Infinity,
      meta: { [ONLY_WHEN_ASKED]: true },
    }),

  // Card ID as srl find lists it.
  details: (id: string) =>
    queryOptions({
      queryKey: ['details', id],
      queryFn: async () => {
        const card = (await api.find(`id:${id}`)).find((c) => c.id === id);
        if (!card) throw new Error(`srl find doesn't list the card ${id}.`);
        return card;
      },
    }),

  // The log events of card ID, in log order.
  log: (id: string) => queryOptions({ queryKey: ['log', id], queryFn: ({ signal }) => api.log(id, signal) }),

  // Item ID as srl show --json gives it.
  item: (id: string) => queryOptions({ queryKey: ['item', id], queryFn: () => api.item(id) }),

  kinds: () => queryOptions({ queryKey: ['kinds'], queryFn: api.kinds }),
  collections: () => queryOptions({ queryKey: ['collections'], queryFn: api.collections }),
  notes: () => queryOptions({ queryKey: ['notes'], queryFn: api.notes }),
  sources: () => queryOptions({ queryKey: ['sources'], queryFn: api.sources }),

  // The counts of today's study day.
  today: () => queryOptions({ queryKey: ['today'], queryFn: api.today }),

  // Whether srl-web was started with --on-change: fixed while it runs.
  hasOnChange: () => queryOptions({ queryKey: ['has-on-change'], queryFn: api.hasOnChange, staleTime: Infinity }),

  // How the --on-change program is doing, read every POLL_MS while it is
  // busy.
  onChangeStatus: (pollMs: number) =>
    queryOptions({
      queryKey: ['on-change'],
      queryFn: api.onChangeStatus,
      refetchInterval: (query) => (query.state.data?.busy ? pollMs : false),
    }),
};

// The meta of a read made only when asked: a change to the store doesn't
// make it out of date (src/main.tsx).
export const ONLY_WHEN_ASKED = 'onlyWhenAsked';
