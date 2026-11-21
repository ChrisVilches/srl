// The page's search: the filters, chosen in the filters' dialog
// (src/Filters.tsx), and the text of the search box. The selector srl gets
// (docs/selectors.md) is made from them: one term for each filter set, then
// each word of the text, in double quotes, so that every word is text that
// the items must contain, whatever it looks like (in:x, -x, an ID). A double
// quote can't be in a quoted term, so it separates words, as a space does.
// An ID, of an item or a card, is a filter, typed in the dialog: the box
// only ever searches text.

export type Menu = 'collection' | 'source' | 'kind' | 'status' | 'schedule' | 'leech' | 'id';

// Each filter's value, or null for any.
export type Filters = Record<Menu, string | null>;

export type Search = { filters: Filters; text: string };

// The filters, in the order of their terms.
export const MENUS: Menu[] = ['collection', 'source', 'kind', 'status', 'schedule', 'leech', 'id'];

// The values of the menus of is:, which srl doesn't list: docs/selectors.md
// gives them.
export const IS_VALUES: Record<'status' | 'schedule' | 'leech', string[]> = {
  status: ['active', 'draft', 'suspended'],
  schedule: ['new', 'due', 'young', 'mature'],
  leech: ['leech'],
};

export const NO_FILTERS: Filters = {
  collection: null,
  source: null,
  kind: null,
  status: null,
  schedule: null,
  leech: null,
  id: null,
};

// The key of MENU's term: is for the menus of is:.
export const keyOf = (menu: Menu) =>
  menu === 'collection' || menu === 'source' || menu === 'kind' || menu === 'id' ? menu : 'is';

// The menu of is: whose value VALUE is, or null.
export const isMenuOf = (value: string) =>
  (['status', 'schedule', 'leech'] as const).find((menu) => IS_VALUES[menu].includes(value)) ?? null;

// Whether VALUE can be written as the value of a term: neither empty nor
// holding a space, a comma or a double quote, which would split it.
export const writable = (value: string) => /^[^\s,"]+$/.test(value);

// How many filters are set.
export const filterCount = (filters: Filters) => MENUS.filter((menu) => filters[menu] !== null).length;

// The selector of SEARCH: its filters' terms and its words, empty, every
// card, when it has neither. Whether an ID is one is srl's to say, which it
// does by refusing it, so that no rule of IDs is written here.
export function selectorOfSearch({ filters, text }: Search): string {
  const keys = MENUS.flatMap((menu) => (filters[menu] === null ? [] : [`${keyOf(menu)}:${filters[menu]}`]));
  const words = text.split(/[\s"]+/).filter((word) => word !== '');
  return [...keys, ...words.map((word) => `"${word}"`)].join(' ');
}
