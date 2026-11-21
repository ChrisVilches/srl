// The page's address: its path names the view, its query the search.
// Each view is a route (src/main.tsx):
//
//   /                     study, as is any path not below
//   /browse[/CARD[/SIDE]] the card browser, with Browse's own parts (src/Browse.tsx)
//   /add                  adding items
//   /stats                the store's scheduling settings and a calendar of study
//
// The query holds the search last submitted (src/filters.ts), so that
// reloading the page keeps it: one parameter per filter, named by its key,
// in the order of the menus (the ID last), then the text, as typed, in q. Collection
// grammar, status active, schedule due and the text "hello you" are
// ?in=grammar&is=active&is=due&q=hello+you. Switching views keeps the query.
//
// The address is the only place this state is kept: the page reads it from
// the address on every render, through the router (react-router), and
// changes it by going to another address. So back and forward go through
// all of it, and the page shows whatever address they come to. Follow the
// rules of README.md, "Where the page is: the address, through the router",
// when adding to it.

// With its extension, for Node to find it in npm test.
import { isMenuOf, keyOf, MENUS, NO_FILTERS, writable, type Search } from './filters.ts';

// The views: studying (the page's start), the card browser, adding items,
// and the statistics.
export type View = 'study' | 'browse' | 'add' | 'stats';

// The path of each view, which its tab links to.
export const VIEW_PATHS: Record<View, string> = { study: '/', browse: '/browse', add: '/add', stats: '/stats' };

// Commas and colons are left as they are, and spaces are +, so that the
// address reads as the search does.
const encode = (s: string) =>
  encodeURIComponent(s).replace(/%2C/gi, ',').replace(/%3A/gi, ':').replace(/%20/g, '+');

// The search of the query QUERY: no filter and no text if it has none. A
// menu takes the first parameter of its key whose value it can take, and
// the menus of is: the first is of one of their values; every other
// parameter is left out.
export function searchOfQuery(query: string): Search {
  const filters = { ...NO_FILTERS };
  let text = '';
  for (const [name, value] of new URLSearchParams(query)) {
    if (name === 'q') {
      if (text === '') text = value.trim();
      continue;
    }
    const menu = name === 'is' ? isMenuOf(value) : MENUS.find((m) => keyOf(m) === name);
    if (menu && filters[menu] === null && writable(value)) filters[menu] = value;
  }
  return { filters, text };
}

// The query of SEARCH, without its ?: empty for no filter and no text.
export function queryOfSearch({ filters, text }: Search): string {
  const params: [string, string][] = MENUS.flatMap((menu) =>
    filters[menu] === null ? [] : [[keyOf(menu), filters[menu]] as [string, string]],
  );
  if (text.trim() !== '') params.push(['q', text.trim()]);
  return params.map(([name, value]) => `${encode(name)}=${encode(value)}`).join('&');
}
