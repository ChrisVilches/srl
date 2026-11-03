// The page's address: its path names the view, its query the selector.
//
//   /                     study
//   /browse[/CARD[/SIDE]] the card browser, with Browse's own parts (src/Browse.tsx)
//   /add                  adding items
//
// The query holds the selector last submitted, one parameter per term, so
// that reloading the page keeps it: in:a,b -kind:qa "two words" is
// ?in=a,b&-kind=qa&text=two%20words. Switching views keeps the query.

// The views: studying (the page's start), the card browser and adding items.
export type View = 'study' | 'browse' | 'add';

// The view is the path's first part; any other path is study.
export function viewOfPath(): View {
  const view = window.location.pathname.split('/')[1];
  return view === 'browse' || view === 'add' ? view : 'study';
}

// Shows PATH, keeping the query, as a new entry of the browser's history.
// App follows the address on popstate, which pushState doesn't fire.
export function go(path: string) {
  window.history.pushState(null, '', path + window.location.search);
  window.dispatchEvent(new PopStateEvent('popstate'));
}

// A term as a parameter: key:value is key=value, and text, quoted or not, is
// text=TEXT without its quotes. A term negated with - gives its parameter
// name the -.
function termParam(term: string): [string, string] {
  const neg = term.length > 1 && term.startsWith('-') ? '-' : '';
  const body = term.slice(neg.length);
  if (body.startsWith('"')) return [`${neg}text`, body.slice(1).replace(/"$/, '')];
  const keyed = /^([a-z]+):(.*)$/s.exec(body);
  return keyed ? [neg + keyed[1], keyed[2]] : [`${neg}text`, body];
}

// A parameter as a term, the other way. Text is quoted when it would
// otherwise be read as something else, or split. Every other parameter is a
// key, even one srl doesn't know, which srl then says is wrong.
function paramTerm(name: string, value: string): string {
  const neg = name.startsWith('-') ? '-' : '';
  const key = name.slice(neg.length);
  if (key !== 'text') return `${neg}${key}:${value}`;
  return neg + (/^$|\s|^[-"]|^[a-z]+:/.test(value) ? `"${value}"` : value);
}

// The terms of a selector, as docs/selectors.md splits them: by spaces, but a
// quoted term, which runs to the next ", spaces included.
const terms = (selector: string) => selector.match(/-?"[^"]*"?|\S+/g) ?? [];

// Commas and colons are left as they are, so that the address reads as the
// selector does.
const encode = (s: string) => encodeURIComponent(s).replace(/%2C/gi, ',').replace(/%3A/gi, ':');

// The selector in the address, its terms in the order of the parameters;
// empty, every card, if there is none.
export function urlSelector(): string {
  return [...new URLSearchParams(window.location.search)].map(([name, value]) => paramTerm(name, value)).join(' ');
}

// Writes SELECTOR in the address, in place of the one there, without adding
// to the browser's history.
export function setUrlSelector(selector: string) {
  const url = new URL(window.location.href);
  url.search = terms(selector)
    .map(termParam)
    .map(([name, value]) => `${encode(name)}=${encode(value)}`)
    .join('&');
  window.history.replaceState(null, '', url);
}
