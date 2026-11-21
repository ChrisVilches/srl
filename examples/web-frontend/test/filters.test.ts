// The filters (src/filters.ts): which terms of a selector the menus read,
// and the selector they write back, every other term left as written.
//
// Run with npm test.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { NO_FILTERS, readFilters, writeFilters, type Filters } from '../src/filters.ts';

const filters = (set: Partial<Filters>): Filters => ({ ...NO_FILTERS, ...set });

test('reads the first plain term of each menu', () => {
  assert.deepEqual(readFilters(''), NO_FILTERS);
  assert.deepEqual(
    readFilters('in:grammar source:items kind:qa is:due is:suspended is:leech'),
    filters({ in: 'grammar', source: 'items', kind: 'qa', status: 'suspended', schedule: 'due', leech: 'leech' }),
  );
  assert.deepEqual(readFilters('in:a in:b'), filters({ in: 'a' }));
});

test('leaves out terms no menu shows', () => {
  assert.deepEqual(readFilters('-in:x in:a,b is:draft,due id:k3m9q1 hello "in:a" -is:new'), NO_FILTERS);
  // A value of is: none of the menus has is left to srl.
  assert.deepEqual(readFilters('is:sleeping'), NO_FILTERS);
});

test('replaces a term where it is, and takes it out for any', () => {
  assert.equal(writeFilters('-in:x hello in:a', filters({ in: 'b' })), '-in:x hello in:b');
  assert.equal(writeFilters('hello in:a is:due', filters({ in: 'a' })), 'hello in:a');
});

test('adds a term a menu had none of after the rest, in the order of the menus', () => {
  assert.equal(
    writeFilters('hello', filters({ leech: 'leech', in: 'grammar', schedule: 'new', status: 'active', kind: 'qa' })),
    'hello in:grammar kind:qa is:active is:new is:leech',
  );
});

test('keeps every term no menu owns, in its place', () => {
  const kept = '-in:x in:a,b is:draft,due id:k3m9q1 "two words" -kind:qa';
  assert.equal(writeFilters(kept, NO_FILTERS), kept);
  assert.equal(writeFilters(`${kept} in:c`, filters({ in: 'd' })), `${kept} in:d`);
  // A second term of a menu's key is not the menu's.
  assert.equal(writeFilters('in:a in:b', NO_FILTERS), 'in:b');
});

test('reads back what it writes', () => {
  const set = filters({ in: 'grammar', source: 'items', status: 'draft', leech: 'leech' });
  assert.deepEqual(readFilters(writeFilters('verb -in:n5', set)), set);
});
