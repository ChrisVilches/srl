// The search kept in the address (src/address.ts): one parameter per
// filter and the text in q, and read back as the same search.
//
// Run with npm test. It is outside src/ so that the page's typecheck, which
// has no Node types, leaves it out.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { queryOfSearch, searchOfQuery } from '../src/address.ts';
import { NO_FILTERS, type Filters, type Search } from '../src/filters.ts';

const search = (filters: Partial<Filters>, text = ''): Search => ({ filters: { ...NO_FILTERS, ...filters }, text });

// Each search, its query, and back.
const ROUND_TRIPS: [Search, string][] = [
  [search({}), ''],
  [search({ in: 'grammar' }), 'in=grammar'],
  [search({ in: 'grammar', source: 'items', kind: 'ja_word' }), 'in=grammar&source=items&kind=ja_word'],
  [search({ in: 'grammar', id: 'k3m9q1.2' }, 'verb'), 'in=grammar&id=k3m9q1.2&q=verb'],
  [search({ status: 'active', schedule: 'due', leech: 'leech' }), 'is=active&is=due&is=leech'],
  [search({ in: 'v1.2' }, 'hello you'), 'in=v1.2&q=hello+you'],
  [search({}, '"std::move" -x'), 'q=%22std::move%22+-x'],
  [search({}, 'ものともせず'), 'q=%E3%82%82%E3%81%AE%E3%81%A8%E3%82%82%E3%81%9B%E3%81%9A'],
  [search({}, 'a&b=c+d'), 'q=a%26b%3Dc%2Bd'],
];

for (const [searched, query] of ROUND_TRIPS) {
  test(`${JSON.stringify(searched)} is ?${query} and back`, () => {
    assert.equal(queryOfSearch(searched), query);
    assert.deepEqual(searchOfQuery(query), searched);
    assert.deepEqual(searchOfQuery(`?${query}`), searched);
  });
}

test('the text is trimmed', () => {
  assert.equal(queryOfSearch(search({}, '  hello  ')), 'q=hello');
  assert.equal(queryOfSearch(search({}, '   ')), '');
});

test('the parameters are read in any order, each is to its menu', () => {
  assert.deepEqual(searchOfQuery('q=x&is=leech&kind=qa&is=new&in=a'), search({ in: 'a', kind: 'qa', schedule: 'new', leech: 'leech' }, 'x'));
});

test('a menu takes the first value it can, and the rest is left out', () => {
  assert.deepEqual(searchOfQuery('in=a&in=b&is=new&is=due'), search({ in: 'a', schedule: 'new' }));
  assert.deepEqual(searchOfQuery('in=a,b&in=my+notes&in=c'), search({ in: 'c' }));
  assert.deepEqual(searchOfQuery('-in=a&id=a,b&is=sleeping&text=x'), search({}));
  assert.deepEqual(searchOfQuery('id=a+b&id=k3m9q1'), search({ id: 'k3m9q1' }));
});
