import MarkdownIt from 'markdown-it';
import { useMemo } from 'react';

import { highlight } from './highlight';

// CommonMark plus ==highlight== (docs/cards.md, "Text formatting"), and GFM
// tables, which notes use. Raw HTML in notes is shown as text, so a note can
// never run scripts in the page.
const md = new MarkdownIt('commonmark', { html: false }).enable('table').use(highlight);

// Open links in a new tab, so that following one doesn't end the session.
const defaultLinkOpen =
  md.renderer.rules.link_open ?? ((tokens, i, options, _env, self) => self.renderToken(tokens, i, options));
md.renderer.rules.link_open = (tokens, i, options, env, self) => {
  tokens[i].attrSet('target', '_blank');
  tokens[i].attrSet('rel', 'noreferrer');
  return defaultLinkOpen(tokens, i, options, env, self);
};

// Code blocks in the theme's colors: the typography plugin makes them dark in
// the light theme too.
const PRE =
  'prose-pre:border prose-pre:border-stone-200 prose-pre:bg-stone-50 prose-pre:text-stone-800 dark:prose-pre:border-stone-800 dark:prose-pre:bg-stone-950/60 dark:prose-pre:text-stone-200';

export function Markdown({ text, className = '' }: { text: string; className?: string }) {
  const html = useMemo(() => md.render(text), [text]);
  return (
    <div
      className={`prose prose-stone max-w-none dark:prose-invert ${PRE} ${className}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
