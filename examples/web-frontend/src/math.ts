import katex from 'katex';
import type { MarkdownIt } from 'markdown-it';

// Math in card text and notes, rendered with KaTeX: $…$ inline and $$…$$ on
// a line of its own, as most Markdown editors write it. docs/cards.md asks
// for no math: raw text is always an acceptable rendering, and srl-tui shows
// it as written.
//
// A $ opens math when it isn't followed by a space, and the next $ closes it
// when it isn't after a space or before a digit, so "$5 and $10" stays text
// (pandoc's rule). \$ is a dollar sign.
//
// A cloze in a formula, $E = {{c1::mc^2}}$, comes as ==[...]== or ==mc^2==
// inside it: each ==…== is marked as the highlights around it are
// (src/highlight.ts), with the class srl-mark, the only one KaTeX may set.
// A deletion ends at the first }}, as in Anki, so one that ends in a brace
// of the formula needs a space before its own: {{c1::\sqrt{\pi} }}.
export function math(md: MarkdownIt) {
  md.inline.ruler.before('srl_highlight', 'srl_math', (state, silent) => {
    const src = state.src;
    const start = state.pos;
    if (src[start] !== '$') return false;
    const display = src.startsWith('$$', start);
    const open = start + (display ? 2 : 1);
    const end = display ? src.indexOf('$$', open) : closing(src, open);
    const after = end + (display ? 2 : 1);
    if (end === -1 || end === open || after > state.posMax) return false;
    if (!display && /\s/.test(src[open])) return false;
    if (!silent) {
      const token = state.push('srl_math', 'math', 0);
      token.content = src.slice(open, end);
      token.markup = display ? '$$' : '$';
    }
    state.pos = after;
    return true;
  });
  md.renderer.rules.srl_math = (tokens, i) =>
    katex.renderToString(marked(tokens[i].content), {
      displayMode: tokens[i].markup === '$$',
      throwOnError: false,
      trust: (context) => context.command === '\\htmlClass' && context.class === 'srl-mark',
      // \htmlClass isn't LaTeX: KaTeX would warn on every highlight.
      strict: (code: string) => (code === 'htmlExtension' ? 'ignore' : 'warn'),
    });
}

// Where the $ after FROM that closes inline math is, or -1.
function closing(src: string, from: number): number {
  for (let at = src.indexOf('$', from); at !== -1; at = src.indexOf('$', at + 1)) {
    if (src[at - 1] === '\\') continue;
    return /\s/.test(src[at - 1]) || /\d/.test(src[at + 1] ?? '') ? -1 : at;
  }
  return -1;
}

// TEX with each ==…== as a highlight, paired in order as src/highlight.ts
// pairs them.
const marked = (tex: string) => tex.replace(/==(.+?)==/gs, '\\htmlClass{srl-mark}{$1}');
