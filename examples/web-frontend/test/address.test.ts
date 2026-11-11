// The selector kept in the address (src/address.ts): written as one query
// parameter per key term and the text terms together in q, and read back as
// the same selector.
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
  ['in:net/routers', 'in=net%2Frouters'],
  ['id:k3m9q1.2', 'id=k3m9q1.2'],
  ['word', 'q=word'],
  // Words, each searched for on its own, stay words, and a quoted term
  // keeps its quotes, so that it stays one.
  ['hello how are you', 'q=hello+how+are+you'],
  ['"two words"', 'q=%22two+words%22'],
  ['-"two words" -x', 'q=-%22two+words%22+-x'],
  ['"std::move"', 'q=%22std::move%22'],
  ['"in:a"', 'q=%22in:a%22'],
  ['ものともせず', 'q=%E3%82%82%E3%81%AE%E3%81%A8%E3%82%82%E3%81%9B%E3%81%9A'],
  ['a&b=c+d', 'q=a%26b%3Dc%2Bd'],
  ['in:a,b -kind:qa in:c hello "two words"', 'in=a,b&-kind=qa&in=c&q=hello+%22two+words%22'],
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
  assert.equal(queryOfSelector(' hello   you '), 'q=hello+you');
});

test('text goes after the key terms', () => {
  assert.equal(queryOfSelector('hello in:a "two words" kind:qa'), 'in=a&kind=qa&q=hello+%22two+words%22');
});

test('the terms come back in the order of the parameters', () => {
  assert.equal(selectorOfQuery('kind=qa&in=b&in=a'), 'kind:qa in:b in:a');
  assert.equal(selectorOfQuery('q=x&in=a'), 'x in:a');
});

test('a query written by hand is read too', () => {
  assert.equal(selectorOfQuery('in=a%2Cb&q=two%20words'), 'in:a,b two words');
  assert.equal(selectorOfQuery('q=&in=a'), 'in:a');
});

test('any other parameter is a key, for srl to say is wrong', () => {
  assert.equal(selectorOfQuery('text=hello'), 'text:hello');
});
