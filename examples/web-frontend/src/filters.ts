// With its extension, for Node to find it in npm test.
import { terms } from './address.ts';

// The filters: a selector written with menus, one value each, for those who
// would rather not type one (src/Filters.tsx). They are a part of the
// selector, not another one: each menu reads and writes one term of the
// selector in the box, and every other term is left as written.
//
// A menu owns the first term of its key that is neither negated nor a list,
// key:value with one value: in:, source: and kind: their own, and the three
// menus of is: (docs/selectors.md, "Status, schedule and leeches") the
// first is: whose value is one of theirs. -in:a, in:a,b, is:draft,due, a
// second in: and every other term belong to no menu.

export type Menu = 'in' | 'source' | 'kind' | 'status' | 'schedule' | 'leech';

// Each menu's value, or null for any.
export type Filters = Record<Menu, string | null>;

// The menus, in the order a term they add goes after the others.
export const MENUS: Menu[] = ['in', 'source', 'kind', 'status', 'schedule', 'leech'];

// The values of the menus of is:, which srl doesn't list: docs/selectors.md
// gives them.
export const IS_VALUES: Record<'status' | 'schedule' | 'leech', string[]> = {
  status: ['active', 'draft', 'suspended'],
  schedule: ['new', 'due', 'young', 'mature'],
  leech: ['leech'],
};

export const NO_FILTERS: Filters = { in: null, source: null, kind: null, status: null, schedule: null, leech: null };

// The key of MENU's term.
const keyOf = (menu: Menu) => (menu === 'in' || menu === 'source' || menu === 'kind' ? menu : 'is');

// Whether VALUE can be written as the value of a term: neither empty nor
// holding a space, a comma or a double quote, which would split it.
export const writable = (value: string) => /^[^\s,"]+$/.test(value);

// The menu TERM may belong to, with its value, or null.
function owner(term: string): [Menu, string] | null {
  const keyed = /^([a-z]+):(.*)$/s.exec(term);
  if (!keyed || !writable(keyed[2])) return null;
  const [, key, value] = keyed;
  if (key === 'in' || key === 'source' || key === 'kind') return [key, value];
  if (key !== 'is') return null;
  const menu = (['status', 'schedule', 'leech'] as const).find((m) => IS_VALUES[m].includes(value));
  return menu ? [menu, value] : null;
}

// The value of each menu in SELECTOR.
export function readFilters(selector: string): Filters {
  const filters = { ...NO_FILTERS };
  const seen = new Set<Menu>();
  for (const term of terms(selector)) {
    const owned = owner(term);
    if (!owned || seen.has(owned[0])) continue;
    seen.add(owned[0]);
    filters[owned[0]] = owned[1];
  }
  return filters;
}

// SELECTOR with each menu's term set to FILTERS: replaced where it is, taken
// out for any, or, for a menu that had none, added after the rest.
export function writeFilters(selector: string, filters: Filters): string {
  const seen = new Set<Menu>();
  const written: string[] = [];
  for (const term of terms(selector)) {
    const owned = owner(term);
    if (!owned || seen.has(owned[0])) {
      written.push(term);
      continue;
    }
    const menu = owned[0];
    seen.add(menu);
    const value = filters[menu];
    if (value !== null) written.push(`${keyOf(menu)}:${value}`);
  }
  for (const menu of MENUS) {
    const value = filters[menu];
    if (!seen.has(menu) && value !== null) written.push(`${keyOf(menu)}:${value}`);
  }
  return written.join(' ');
}
