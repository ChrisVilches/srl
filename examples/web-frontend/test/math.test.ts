// Math in card text (src/math.ts): what is math and what stays text, and
// highlights inside a formula.
//
// Run with npm test.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import MarkdownIt from 'markdown-it';

import { highlight } from '../src/highlight.ts';
import { math } from '../src/math.ts';

const md = new MarkdownIt('commonmark', { html: false }).use(highlight).use(math);
const isMath = (text: string) => md.renderInline(text).includes('class="katex');

test('$…$ and $$…$$ are math', () => {
  assert.ok(isMath('$x^2$'));
  assert.ok(isMath('so $a + b$ is'));
  assert.ok(isMath('$$\\int_0^1 x\\,dx$$'));
  assert.ok(md.render('$$\n\\frac{a}{b}\n$$').includes('katex-display'));
});

test('dollars that are money stay text', () => {
  for (const text of ['$5 and $10', 'it costs $ 5', 'from $5 to $ 6$', '\\$x$', '$x $', '$$']) {
    assert.ok(!isMath(text), text);
  }
});

test('code keeps its dollars', () => {
  assert.ok(!isMath('`echo $HOME $PATH`'));
});

test('a highlight in a formula is marked, and so is a formula in a highlight', () => {
  assert.ok(md.renderInline('$E = ==mc^2==$').includes('srl-mark'));
  assert.ok(md.renderInline('$x = ==[...]==$').includes('srl-mark'));
  const around = md.renderInline('==$x^2$==');
  assert.ok(around.startsWith('<mark>') && around.includes('katex'));
});

test('a formula KaTeX can\'t read is shown, not thrown', () => {
  assert.ok(md.renderInline('$\\frac{a$').includes('katex-error'));
});
