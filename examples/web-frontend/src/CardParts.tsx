import type { ReactNode } from 'react';

import type { ShownCard } from './api';
import { Markdown } from './Markdown';

// A card as studying shows it: its ID and kind, and the prompt, or, once
// revealed, the revealed prompt and the answer. BADGES go after the kind.
export function CardFace({ card, revealed, badges }: { card: ShownCard; revealed: boolean; badges?: ReactNode }) {
  return (
    <article className="rounded-2xl border border-stone-200 bg-white shadow-sm dark:border-stone-800 dark:bg-stone-900">
      <div className="flex items-center justify-between px-6 pt-4 text-xs text-stone-400">
        <span className="font-mono">{card.id}</span>
        <span className="flex gap-2">
          <KindBadge kind={card.kind} />
          {badges}
        </span>
      </div>
      <section className="px-6 pt-2 pb-6">
        <Label>Question</Label>
        <Markdown text={revealed ? (card.prompt_revealed ?? card.prompt) : card.prompt} className="prose-lg" />
      </section>
      {revealed && card.answer !== null && (
        <section className="border-t border-dashed border-stone-200 px-6 py-6 dark:border-stone-700">
          <Label>Answer</Label>
          <Markdown text={card.answer} className="prose-lg" />
        </section>
      )}
    </article>
  );
}

// The kind that made a card, in violet when it isn't qa.
export function KindBadge({ kind }: { kind: string }) {
  return (
    <span
      title="The kind that made this card"
      className={
        kind === 'qa'
          ? 'rounded-full px-2 py-0.5 font-mono'
          : 'rounded-full bg-violet-100 px-2 py-0.5 font-mono font-medium text-violet-700 dark:bg-violet-950 dark:text-violet-300'
      }
    >
      {kind}
    </span>
  );
}

// The item's context: the note it belongs to, or the note its see: names.
export function ContextView({ card }: { card: ShownCard }) {
  if (card.context === null) {
    return <p className="text-center text-sm text-stone-400">No context.</p>;
  }
  return (
    <section className="rounded-2xl bg-stone-100 px-6 py-5 dark:bg-stone-900/50">
      <div className="mb-3 font-mono text-xs text-stone-500">{card.see ?? card.path}</div>
      <Markdown text={card.context} className="prose-sm prose-stone" />
    </section>
  );
}

export function ErrorView({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="m-auto flex w-full flex-col gap-4">
      <h1 className="font-semibold text-rose-600 dark:text-rose-400">srl reported an error</h1>
      <pre className="overflow-x-auto rounded-lg bg-rose-50 p-4 text-sm whitespace-pre-wrap text-rose-950 dark:bg-rose-950/40 dark:text-rose-200">
        {message}
      </pre>
      <button
        onClick={onRetry}
        className="self-start rounded-lg border border-stone-300 px-4 py-2 text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
      >
        Try again
      </button>
    </div>
  );
}

export function Label({ children }: { children: ReactNode }) {
  return <div className="mb-2 text-xs font-semibold tracking-widest text-stone-400 uppercase">{children}</div>;
}

export function Key({ children }: { children: ReactNode }) {
  return <kbd className="ml-1 rounded bg-black/15 px-1.5 py-0.5 font-mono text-xs opacity-80">{children}</kbd>;
}

export function Notice({ children }: { children: ReactNode }) {
  return <p className="m-auto text-stone-400">{children}</p>;
}
