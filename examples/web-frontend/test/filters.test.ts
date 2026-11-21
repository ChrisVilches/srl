// The selector srl gets for a search (src/filters.ts): a term per filter,
// the ID among them, then each word of the text in double quotes.
//
// Run with npm test.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { collectionNames, filterCount, NO_FILTERS, selectorOfSearch, type Filters } from '../src/filters.ts';

const selector = (filters: Partial<Filters>, text = '') =>
  selectorOfSearch({ filters: { ...NO_FILTERS, ...filters }, text });

test('no filter and no text is every card', () => {
  assert.equal(selector({}), '');
  assert.equal(selector({}, '  '), '');
});

test('each filter is a term, in the order of the menus, the ID last', () => {
  assert.equal(
    selector({ id: 'k3m9q1', leech: 'leech', collection: 'grammar', schedule: 'new', status: 'active', kind: 'qa', source: 'items' }),
    'collection:grammar source:items kind:qa is:active is:new is:leech id:k3m9q1',
  );
});

test('each word is text, whatever it looks like', () => {
  assert.equal(selector({}, 'hello  you'), '"hello" "you"');
  assert.equal(selector({}, 'in:x -x std::move'), '"in:x" "-x" "std::move"');
  assert.equal(selector({ collection: 'a' }, 'two words'), 'collection:a "two" "words"');
});

test('one word is text, even one that looks like an ID', () => {
  assert.equal(selector({}, 'k3m9q1'), '"k3m9q1"');
  assert.equal(selector({ id: 'k3m9q1' }, 'verb'), 'id:k3m9q1 "verb"');
});

test('a double quote separates words', () => {
  assert.equal(selector({}, '"two words"'), '"two" "words"');
  assert.equal(selector({}, 'a"b'), '"a" "b"');
});

test('counts the filters set, the ID among them', () => {
  assert.equal(filterCount(NO_FILTERS), 0);
  assert.equal(filterCount({ ...NO_FILTERS, collection: 'a', leech: 'leech', id: 'k3m9q1' }), 3);
});

test('the Collection menu offers every collection by its whole name, in the order srl lists them', () => {
  assert.deepEqual(
    collectionNames([{ name: 'b/questions.jsonl' }, { name: 'a/questions.jsonl' }, { name: 'words' }]),
    ['b/questions.jsonl', 'a/questions.jsonl', 'words'],
  );
});

test('a collection name a term cannot hold is not offered', () => {
  assert.deepEqual(collectionNames([{ name: 'my notes/words.jsonl' }, { name: 'a,b' }, { name: 'ok.jsonl' }]), ['ok.jsonl']);
});

test('a collection filter is its whole name', () => {
  assert.equal(selector({ collection: 'a/questions.jsonl' }), 'collection:a/questions.jsonl');
});
