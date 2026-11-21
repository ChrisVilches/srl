// The page's search: the filters, chosen with menus (src/Filters.tsx), and
// the text of the search box. The selector srl gets (docs/selectors.md) is
// made from them: one term for each filter set, then each word of the text,
// in double quotes, so that every word is text that the items must contain,
// whatever it looks like (in:x, -x). A double quote can't be in a quoted
// term, so it separates words, as a space does.
//
// A text of one word may also be an ID, of an item or a card, which the
// items don't contain as text: such a search is two selectors, the word as
// an ID (id:WORD) and as text, and finds the cards of either, those of the
// ID first (src/api.ts).

export type Menu = 'in' | 'source' | 'kind' | 'status' | 'schedule' | 'leech';

// Each menu's value, or null for any.
export type Filters = Record<Menu, string | null>;

export type Search = { filters: Filters; text: string };

// The menus, in the order of their terms.
export const MENUS: Menu[] = ['in', 'source', 'kind', 'status', 'schedule', 'leech'];

// The values of the menus of is:, which srl doesn't list: docs/selectors.md
// gives them.
export const IS_VALUES: Record<'status' | 'schedule' | 'leech', string[]> = {
  status: ['active', 'draft', 'suspended'],
  schedule: ['new', 'due', 'young', 'mature'],
  leech: ['leech'],
};

export const NO_FILTERS: Filters = { in: null, source: null, kind: null, status: null, schedule: null, leech: null };

// The key of MENU's term: is for the menus of is:.
export const keyOf = (menu: Menu) => (menu === 'in' || menu === 'source' || menu === 'kind' ? menu : 'is');

// The menu of is: whose value VALUE is, or null.
export const isMenuOf = (value: string) =>
  (['status', 'schedule', 'leech'] as const).find((menu) => IS_VALUES[menu].includes(value)) ?? null;

// Whether VALUE can be written as the value of a term: neither empty nor
// holding a space, a comma or a double quote, which would split it.
export const writable = (value: string) => /^[^\s,"]+$/.test(value);

// How many filters are set.
export const filterCount = (filters: Filters) => MENUS.filter((menu) => filters[menu] !== null).length;

// The selectors of SEARCH, in the order their cards come: with one word that
// can be written as a term, the word as an ID, then as text; otherwise the
// one selector of its words, empty, every card, when it has no filter and
// no word. Whether the word is an ID is srl's to say, which it does by
// refusing it (api.ts), so that no rule of IDs is written here.
export function selectorsOfSearch({ filters, text }: Search): string[] {
  const keys = MENUS.flatMap((menu) => (filters[menu] === null ? [] : [`${keyOf(menu)}:${filters[menu]}`]));
  const words = text.split(/[\s"]+/).filter((word) => word !== '');
  const asText = [...keys, ...words.map((word) => `"${word}"`)].join(' ');
  if (words.length === 1 && writable(words[0])) return [[...keys, `id:${words[0]}`].join(' '), asText];
  return [asText];
}
