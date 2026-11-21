// The selectors srl gets for a search (src/filters.ts): a term per filter,
// then each word of the text in double quotes, and, for one word, the word
// as an ID first.
//
// Run with npm test.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { filterCount, NO_FILTERS, selectorsOfSearch, type Filters } from '../src/filters.ts';

const selectors = (filters: Partial<Filters>, text = '') =>
  selectorsOfSearch({ filters: { ...NO_FILTERS, ...filters }, text });

test('no filter and no text is every card', () => {
  assert.deepEqual(selectors({}), ['']);
  assert.deepEqual(selectors({}, '  '), ['']);
});

test('each filter is a term, in the order of the menus', () => {
  assert.deepEqual(
    selectors({ leech: 'leech', in: 'grammar', schedule: 'new', status: 'active', kind: 'qa', source: 'items' }),
    ['in:grammar source:items kind:qa is:active is:new is:leech'],
  );
});

test('each word of several is text, whatever it looks like', () => {
  assert.deepEqual(selectors({}, 'hello  you'), ['"hello" "you"']);
  assert.deepEqual(selectors({}, 'in:x -x std::move'), ['"in:x" "-x" "std::move"']);
  assert.deepEqual(selectors({ in: 'a' }, 'two words'), ['in:a "two" "words"']);
});

test('one word is an ID, then text, with the filters', () => {
  assert.deepEqual(selectors({}, 'k3m9q1'), ['id:k3m9q1', '"k3m9q1"']);
  assert.deepEqual(selectors({}, ' K3m9q1.2 '), ['id:K3m9q1.2', '"K3m9q1.2"']);
  assert.deepEqual(selectors({ in: 'a', status: 'active' }, 'verb'), ['in:a is:active id:verb', 'in:a is:active "verb"']);
});

test('one word that a term can\'t hold is text only', () => {
  assert.deepEqual(selectors({}, 'a,b'), ['"a,b"']);
});

test('a double quote separates words', () => {
  assert.deepEqual(selectors({}, '"two words"'), ['"two" "words"']);
  assert.deepEqual(selectors({}, 'a"b'), ['"a" "b"']);
});

test('counts the filters set', () => {
  assert.equal(filterCount(NO_FILTERS), 0);
  assert.equal(filterCount({ ...NO_FILTERS, in: 'a', leech: 'leech' }), 2);
});
