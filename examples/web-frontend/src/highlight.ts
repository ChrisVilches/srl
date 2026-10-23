import type { MarkdownIt } from 'markdown-it';

// ==highlight== (docs/cards.md, "Text formatting"): each `==` is paired with
// the next `==` after it, and the text between them is marked.
//
// Plugins such as markdown-it-mark treat `==` like `*`, with CommonMark's
// flanking rules, which fail on text without spaces: in the cloze prompt
// `==[...]==を==[...]==`, the `==` between `]` and `を` can only open a
// highlight, so the marks come out nested. Pairing them in order, as the
// terminal frontends do, gets every card right.
export function highlight(md: MarkdownIt) {
  md.inline.ruler.before('emphasis', 'srl_highlight', (state, silent) => {
    const start = state.pos;
    if (!state.src.startsWith('==', start)) return false;
    const end = state.src.indexOf('==', start + 2);
    if (end === -1 || end === start + 2 || end + 2 > state.posMax) return false;
    if (!silent) {
      const max = state.posMax;
      state.push('mark_open', 'mark', 1);
      state.pos = start + 2;
      state.posMax = end;
      state.md.inline.tokenize(state);
      state.push('mark_close', 'mark', -1);
      state.posMax = max;
    }
    state.pos = end + 2;
    return true;
  });
}
