// The page's address: its path names the view, its query the selector.
// Each view is a route (src/main.tsx):
//
//   /                     study, as is any path not below
//   /browse[/CARD[/SIDE]] the card browser, with Browse's own parts (src/Browse.tsx)
//   /add                  adding items
//
// The query holds the selector last submitted, so that reloading the page
// keeps it: one parameter per key term, and the text terms together in q, as
// written. in:a,b -kind:qa hello "two words" is
// ?in=a,b&-kind=qa&q=hello+%22two+words%22. Switching views keeps the query.
//
// The address is the only place this state is kept: the page reads it from
// the address on every render, through the router (react-router), and
// changes it by going to another address. So back and forward go through
// all of it, and the page shows whatever address they come to. Follow the
// rules of README.md, "Where the page is: the address, through the router",
// when adding to it.

// The views: studying (the page's start), the card browser and adding items.
export type View = 'study' | 'browse' | 'add';

// The path of each view, which its tab links to.
export const VIEW_PATHS: Record<View, string> = { study: '/', browse: '/browse', add: '/add' };

// The key term KEY:VALUE, negated or not, as the parameter KEY=VALUE, the -
// going to its name; null for a text term (docs/selectors.md: a term is a
// key term when it starts with lowercase letters and a colon).
function keyParam(term: string): [string, string] | null {
  const keyed = /^(-?[a-z]+):(.*)$/s.exec(term);
  return keyed ? [keyed[1], keyed[2]] : null;
}

// The terms of a selector, as docs/selectors.md splits them: by spaces, but a
// quoted term, which runs to the next ", spaces included.
const terms = (selector: string) => selector.match(/-?"[^"]*"?|\S+/g) ?? [];

// Commas and colons are left as they are, and spaces are +, so that the
// address reads as the selector does.
const encode = (s: string) =>
  encodeURIComponent(s).replace(/%2C/gi, ',').replace(/%3A/gi, ':').replace(/%20/g, '+');

// The selector of the query QUERY, its terms in the order of the
// parameters; empty, every card, if there is none. q is selector text, read
// as written. Every other parameter is a key, even one srl doesn't know,
// which srl then says is wrong.
export function selectorOfQuery(query: string): string {
  return [...new URLSearchParams(query)]
    .map(([name, value]) => (name === 'q' ? value.trim() : `${name}:${value}`))
    .filter((term) => term !== '')
    .join(' ');
}

// The query of SELECTOR, without its ?: empty for the empty selector. Its
// key terms, in order, then its text terms together in q, as written.
export function queryOfSelector(selector: string): string {
  const all = terms(selector);
  const keys = all.map(keyParam).filter((param) => param !== null);
  const text = all.filter((term) => keyParam(term) === null).join(' ');
  return [...keys, ...(text === '' ? [] : [['q', text]])]
    .map(([name, value]) => `${encode(name)}=${encode(value)}`)
    .join('&');
}
