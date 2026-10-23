import { useCallback, useEffect, useRef, useState } from 'react';

import { api, type Card, type Rating } from './api';
import { Markdown } from './Markdown';

// Where the session is. It runs the frontend loop of docs/frontends.md:
// get a card, show the prompt, show the answer and the context, rate it.
type Phase =
  | { kind: 'loading' }
  | { kind: 'prompt'; card: Card }
  | { kind: 'answer'; card: Card }
  | { kind: 'done' }
  | { kind: 'error'; message: string };

type Stats = { reviewed: number; new: number; again: number };

const RATINGS: { rating: Rating; name: string; style: string }[] = [
  { rating: 1, name: 'Again', style: 'bg-rose-600 hover:bg-rose-500' },
  { rating: 2, name: 'Hard', style: 'bg-amber-600 hover:bg-amber-500' },
  { rating: 3, name: 'Good', style: 'bg-emerald-600 hover:bg-emerald-500' },
  { rating: 4, name: 'Easy', style: 'bg-sky-600 hover:bg-sky-500' },
];

export default function App() {
  const [selector, setSelector] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: 'loading' });
  const [stats, setStats] = useState<Stats>({ reviewed: 0, new: 0, again: 0 });
  const [busy, setBusy] = useState(false);

  // Runs one step of the loop, showing any error from srl instead of the card.
  const step = useCallback(async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      setPhase({ kind: 'error', message: (error as Error).message });
    } finally {
      setBusy(false);
    }
  }, []);

  const loadNext = useCallback(
    (sel: string) =>
      step(async () => {
        const card = await api.next(sel);
        if (!card) {
          setPhase({ kind: 'done' });
          return;
        }
        setPhase({ kind: 'prompt', card });
      }),
    [step],
  );

  useEffect(() => {
    step(async () => {
      const config = await api.config();
      setSelector(config.selector);
      await loadNext(config.selector);
    });
  }, [step, loadNext]);

  const reveal = useCallback(() => {
    if (phase.kind !== 'prompt') return;
    setPhase({ kind: 'answer', card: phase.card });
  }, [phase]);

  const rate = useCallback(
    (rating: Rating) => {
      if (phase.kind !== 'answer' || selector === null) return;
      const { card } = phase;
      step(async () => {
        await api.review(card.id, rating);
        setStats((s) => ({
          reviewed: s.reviewed + 1,
          new: s.new + (card.state === 'new' ? 1 : 0),
          again: s.again + (rating === 1 ? 1 : 0),
        }));
        await loadNext(selector);
      });
    },
    [phase, selector, step, loadNext],
  );

  const suspend = useCallback(() => {
    if ((phase.kind !== 'prompt' && phase.kind !== 'answer') || selector === null) return;
    const { card } = phase;
    step(async () => {
      await api.suspend(card.id);
      await loadNext(selector);
    });
  }, [phase, selector, step, loadNext]);

  // Keys: space or Enter shows the answer, 1–4 rate, s suspends.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (busy || event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.target instanceof HTMLInputElement) return;
      if (event.key === ' ' || event.key === 'Enter') {
        if (phase.kind !== 'prompt') return;
        event.preventDefault();
        reveal();
      } else if (['1', '2', '3', '4'].includes(event.key)) {
        rate(Number(event.key) as Rating);
      } else if (event.key === 's') {
        suspend();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [busy, phase, reveal, rate, suspend]);

  return (
    <div className="flex min-h-screen flex-col bg-stone-50 text-stone-900 dark:bg-stone-950 dark:text-stone-100">
      <Header
        selector={selector}
        stats={stats}
        onSelector={(sel) => {
          setSelector(sel);
          loadNext(sel);
        }}
      />
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 py-8 sm:px-6">
        {phase.kind === 'loading' && <Notice>Loading…</Notice>}
        {phase.kind === 'error' && (
          <ErrorView message={phase.message} onRetry={() => selector !== null && loadNext(selector)} />
        )}
        {phase.kind === 'done' && (
          <DoneView selector={selector ?? ''} stats={stats} onCheck={() => loadNext(selector ?? '')} />
        )}
        {(phase.kind === 'prompt' || phase.kind === 'answer') && (
          <CardView phase={phase} busy={busy} onReveal={reveal} onRate={rate} onSuspend={suspend} />
        )}
      </main>
    </div>
  );
}

function Header({
  selector,
  stats,
  onSelector,
}: {
  selector: string | null;
  stats: Stats;
  onSelector: (selector: string) => void;
}) {
  const [draft, setDraft] = useState(selector ?? '');
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => setDraft(selector ?? ''), [selector]);

  return (
    <header className="border-b border-stone-200 bg-white/70 backdrop-blur dark:border-stone-800 dark:bg-stone-900/70">
      <div className="mx-auto flex max-w-3xl flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 sm:px-6">
        <span className="font-mono text-lg font-bold tracking-tight">srl</span>
        <form
          className="min-w-48 flex-1"
          onSubmit={(event) => {
            event.preventDefault();
            input.current?.blur();
            onSelector(draft.trim());
          }}
        >
          <input
            ref={input}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="selector, e.g. in:grammar (empty: every card)"
            spellCheck={false}
            className="w-full rounded-md border border-stone-300 bg-transparent px-3 py-1.5 font-mono text-sm outline-none placeholder:text-stone-400 focus:border-stone-500 dark:border-stone-700 dark:focus:border-stone-400"
          />
        </form>
        <StatsLine stats={stats} />
      </div>
    </header>
  );
}

function StatsLine({ stats }: { stats: Stats }) {
  return (
    <span className="text-sm text-stone-500 tabular-nums dark:text-stone-400">
      <b className="text-stone-800 dark:text-stone-200">{stats.reviewed}</b> reviewed ·{' '}
      <b className="text-sky-600 dark:text-sky-400">{stats.new}</b> new ·{' '}
      <b className="text-rose-600 dark:text-rose-400">{stats.again}</b> Again
    </span>
  );
}

function CardView({
  phase,
  busy,
  onReveal,
  onRate,
  onSuspend,
}: {
  phase: Extract<Phase, { kind: 'prompt' | 'answer' }>;
  busy: boolean;
  onReveal: () => void;
  onRate: (rating: Rating) => void;
  onSuspend: () => void;
}) {
  const { card } = phase;
  return (
    <div className="flex flex-1 flex-col gap-6">
      <article className="rounded-2xl border border-stone-200 bg-white shadow-sm dark:border-stone-800 dark:bg-stone-900">
        <div className="flex items-center justify-between px-6 pt-4 text-xs text-stone-400">
          <span className="font-mono">{card.id}</span>
          {card.state === 'new' && (
            <span className="rounded-full bg-sky-100 px-2 py-0.5 font-medium text-sky-700 dark:bg-sky-950 dark:text-sky-300">
              new
            </span>
          )}
        </div>
        <section className="px-6 pt-2 pb-6">
          <Label>Question</Label>
          <Markdown text={card.prompt} className="prose-lg" />
        </section>
        {phase.kind === 'answer' && (
          <section className="border-t border-dashed border-stone-200 px-6 py-6 dark:border-stone-700">
            <Label>Answer</Label>
            <Markdown text={card.answer} className="prose-lg" />
          </section>
        )}
      </article>

      {phase.kind === 'answer' && <ContextView card={card} />}

      <div className="sticky bottom-0 -mx-4 mt-auto border-t border-stone-200 bg-stone-50/90 px-4 py-4 backdrop-blur sm:-mx-6 sm:px-6 dark:border-stone-800 dark:bg-stone-950/90">
        <div className="flex flex-wrap items-center gap-2">
          {phase.kind === 'prompt' ? (
            <button
              onClick={onReveal}
              disabled={busy}
              className="flex-1 rounded-lg bg-stone-900 px-4 py-3 font-medium text-white hover:bg-stone-700 disabled:opacity-50 dark:bg-stone-100 dark:text-stone-900 dark:hover:bg-stone-300"
            >
              Show answer <Key>space</Key>
            </button>
          ) : (
            RATINGS.map(({ rating, name, style }) => (
              <button
                key={rating}
                onClick={() => onRate(rating)}
                disabled={busy}
                className={`min-w-20 flex-1 rounded-lg px-3 py-3 font-medium text-white disabled:opacity-50 ${style}`}
              >
                {name} <Key>{rating}</Key>
              </button>
            ))
          )}
          <button
            onClick={onSuspend}
            disabled={busy}
            title="Suspend this card without recording a review"
            className="rounded-lg px-3 py-3 text-sm text-stone-500 hover:bg-stone-200 disabled:opacity-50 dark:hover:bg-stone-800"
          >
            Suspend <Key>s</Key>
          </button>
        </div>
      </div>
    </div>
  );
}

// The item's context: the note it belongs to, or the note its see: names.
function ContextView({ card }: { card: Card }) {
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

function DoneView({ selector, stats, onCheck }: { selector: string; stats: Stats; onCheck: () => void }) {
  return (
    <div className="m-auto flex flex-col items-center gap-4 text-center">
      <div className="text-4xl">✓</div>
      <h1 className="text-xl font-semibold">Nothing left to study</h1>
      <p className="text-stone-500">
        {selector ? (
          <>
            No card is due for <code className="font-mono">{selector}</code>.
          </>
        ) : (
          'No card is due.'
        )}
      </p>
      <StatsLine stats={stats} />
      <button
        onClick={onCheck}
        className="rounded-lg border border-stone-300 px-4 py-2 text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
      >
        Check again
      </button>
    </div>
  );
}

function ErrorView({ message, onRetry }: { message: string; onRetry: () => void }) {
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

function Label({ children }: { children: React.ReactNode }) {
  return <div className="mb-2 text-xs font-semibold tracking-widest text-stone-400 uppercase">{children}</div>;
}

function Key({ children }: { children: React.ReactNode }) {
  return <kbd className="ml-1 rounded bg-black/15 px-1.5 py-0.5 font-mono text-xs opacity-80">{children}</kbd>;
}

function Notice({ children }: { children: React.ReactNode }) {
  return <p className="m-auto text-stone-400">{children}</p>;
}
