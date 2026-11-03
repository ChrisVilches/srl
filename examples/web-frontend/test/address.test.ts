// The selector kept in the address (src/address.ts): written as one query
// parameter per term, and read back as the same selector.
//
// Run with npm test. It is outside src/ so that the page's typecheck, which
// has no Node types, leaves it out.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { queryOfSelector, selectorOfQuery } from '../src/address.ts';

// Each selector, its query, and back.
const ROUND_TRIPS: [string, string][] = [
  ['', ''],
  ['in:grammar', 'in=grammar'],
  ['in:a,b,c kind:qa', 'in=a,b,c&kind=qa'],
  // Two terms of one key are two parameters, not one with a comma: and,
  // not or.
  ['in:a in:b', 'in=a&in=b'],
  ['in:a,b in:c', 'in=a,b&in=c'],
  ['in:a kind:qa in:b', 'in=a&kind=qa&in=b'],
  ['-in:n5', '-in=n5'],
  ['in:a -in:a', 'in=a&-in=a'],
  ['is:new is:active', 'is=new&is=active'],
  ['path:japanese/**', 'path=japanese%2F**'],
  ['id:k3m9q1.2', 'id=k3m9q1.2'],
  ['word', 'text=word'],
  ['"two words"', 'text=two%20words'],
  ['-"two words"', '-text=two%20words'],
  ['"std::move"', 'text=std::move'],
  ['-"-x"', '-text=-x'],
  ['ものともせず', 'text=%E3%82%82%E3%81%AE%E3%81%A8%E3%82%82%E3%81%9B%E3%81%9A'],
  ['a&b=c', 'text=a%26b%3Dc'],
  ['in:a,b -kind:qa "two words" in:c', 'in=a,b&-kind=qa&text=two%20words&in=c'],
];

for (const [selector, query] of ROUND_TRIPS) {
  test(`${JSON.stringify(selector)} is ?${query} and back`, () => {
    assert.equal(queryOfSelector(selector), query);
    assert.equal(selectorOfQuery(query), selector);
    assert.equal(selectorOfQuery(`?${query}`), selector);
  });
}

test('spaces between terms are not kept', () => {
  assert.equal(queryOfSelector('  in:a   in:b '), 'in=a&in=b');
});

test('the terms come back in the order of the parameters', () => {
  assert.equal(selectorOfQuery('kind=qa&in=b&in=a'), 'kind:qa in:b in:a');
});

test('a query written by hand is read too', () => {
  assert.equal(selectorOfQuery('in=a%2Cb&text=two+words'), 'in:a,b "two words"');
});

test('text is quoted only when it needs to be', () => {
  assert.equal(queryOfSelector('"word"'), 'text=word');
  assert.equal(selectorOfQuery('text=word'), 'word');
});

test('text that would be read otherwise is quoted', () => {
  assert.equal(selectorOfQuery('text=-x'), '"-x"');
  assert.equal(selectorOfQuery('text=in:a'), '"in:a"');
  assert.equal(selectorOfQuery('text='), '""');
});

test('any other parameter is a key, for srl to say is wrong', () => {
  assert.equal(selectorOfQuery('selector=in:grammar'), 'selector:in:grammar');
});
