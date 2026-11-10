// The cloze deletions the qa question's shortcut writes (src/qaCloze.ts):
// numbered after the highest one, never checked.
//
// Run with npm test.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { clozeHint, clozeShortcut, deletion } from '../src/qaCloze.ts';

test('the first deletion is c1', () => {
  assert.equal(deletion('The capital of France is Paris.', 'Paris'), '{{c1::Paris}}');
});

test('a deletion takes the number after the highest one', () => {
  assert.equal(deletion('{{c1::a}} {{c3::b::hint}} {{c2::c}} d', 'd'), '{{c4::d}}');
});

test('nothing selected is an empty deletion, left for srl to refuse', () => {
  assert.equal(deletion('', ''), '{{c1::}}');
});

test('only an opening with a number counts', () => {
  assert.equal(deletion('format!("{{}}") {{c::x}} {{c2:y}}', 'x'), '{{c1::x}}');
});

test("only qa's question gets the shortcut", () => {
  assert.notEqual(clozeShortcut('qa', 'Q'), undefined);
  for (const [kind, field] of [
    ['qa', 'A'],
    ['ja-words', 'Q'],
    ['other', 'WORD'],
  ]) {
    assert.equal(clozeShortcut(kind, field), undefined);
  }
});

test('only qa gets the hint', () => {
  assert.notEqual(clozeHint('qa'), undefined);
  assert.equal(clozeHint('ja-words'), undefined);
});
