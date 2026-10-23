import MarkdownIt from 'markdown-it';
import { useMemo } from 'react';

import { highlight } from './highlight';

// CommonMark plus ==highlight== (docs/cards.md, "Text formatting"). Raw HTML
// in notes is shown as text, so a note can never run scripts in the page.
const md = new MarkdownIt('commonmark', { html: false }).use(highlight);

// Open links in a new tab, so that following one doesn't end the session.
const defaultLinkOpen =
  md.renderer.rules.link_open ?? ((tokens, i, options, _env, self) => self.renderToken(tokens, i, options));
md.renderer.rules.link_open = (tokens, i, options, env, self) => {
  tokens[i].attrSet('target', '_blank');
  tokens[i].attrSet('rel', 'noreferrer');
  return defaultLinkOpen(tokens, i, options, env, self);
};

export function Markdown({ text, className = '' }: { text: string; className?: string }) {
  const html = useMemo(() => md.render(text), [text]);
  return (
    <div
      className={`prose max-w-none dark:prose-invert ${className}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
