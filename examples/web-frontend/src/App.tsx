import { useCallback, useEffect, useState, type ComponentType } from 'react';

import { Add } from './Add';
import { api, type Card, type Rating } from './api';
import { Browse } from './Browse';
import { CardFace, Counts, ErrorView, Key, MAIN_BUTTON, Notice, OUTLINE_BUTTON, QUIET_BUTTON, StateBadges } from './CardParts';
import { useKeys, useStartWhenShown } from './hooks';
import { SelectorInput, TopBar, type View } from './TopBar';

const VIEWS: Record<View, ComponentType<{ active: boolean }>> = {
  study: Study,
  browse: Browse,
  add: Add,
};

const viewOfHash = (): View => {
  const hash = window.location.hash.slice(1);
  return hash === 'browse' || hash === 'add' ? hash : 'study';
};

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

// The phase after getting the next card: its prompt, or done if none is left.
async function nextPhase(selector: string): Promise<Phase> {
  const card = await api.next(selector);
  return card ? { kind: 'prompt', card } : { kind: 'done' };
}

// Every view stays mounted, so that switching keeps the session, the
// browser and the item being written where they were; only the one shown
// takes keys.
export default function App() {
  const [view, setView] = useState<View>(viewOfHash);
  useEffect(() => {
    const onHash = () => setView(viewOfHash());
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);

  return (
    <div className="min-h-screen bg-stone-50 text-stone-900 dark:bg-stone-950 dark:text-stone-100">
      {(Object.keys(VIEWS) as View[]).map((v) => {
        const Pane = VIEWS[v];
        return (
          <div key={v} className={view === v ? 'contents' : 'hidden'}>
            <Pane active={view === v} />
          </div>
        );
      })}
    </div>
  );
}

function Study({ active }: { active: boolean }) {
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

  // Asks for the selector srl-web was started with, then for the first card.
  const start = useCallback(
    () =>
      step(async () => {
        const sel = await api.selector();
        setSelector(sel);
        setPhase(await nextPhase(sel));
      }),
    [step],
  );

  const loadNext = useCallback((sel: string) => step(async () => setPhase(await nextPhase(sel))), [step]);

  // Starts when first shown: a page opened on the browser makes no card.
  useStartWhenShown(active, start);

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
        setPhase(await nextPhase(selector));
      });
    },
    [phase, selector, step],
  );

  const suspend = useCallback(() => {
    if ((phase.kind !== 'prompt' && phase.kind !== 'answer') || selector === null) return;
    const { card } = phase;
    step(async () => {
      await api.suspend(card.id);
      setPhase(await nextPhase(selector));
    });
  }, [phase, selector, step]);

  // Keys: space or Enter shows the answer, 1–4 rate, s suspends.
  useKeys(active, (event) => {
    if (busy) return;
    if (event.key === ' ' || event.key === 'Enter') {
      if (phase.kind !== 'prompt') return;
      event.preventDefault();
      reveal();
    } else if (['1', '2', '3', '4'].includes(event.key)) {
      rate(Number(event.key) as Rating);
    } else if (event.key === 's') {
      suspend();
    }
  });

  return (
    <div className="flex min-h-screen flex-col">
      <TopBar view="study">
        <SelectorInput
          selector={selector}
          placeholder="selector, e.g. in:grammar (empty: every card)"
          onSelector={(sel) => {
            setSelector(sel);
            loadNext(sel);
          }}
        />
        <StatsLine stats={stats} />
      </TopBar>
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 py-8 sm:px-6">
        {phase.kind === 'loading' && <Notice>Loading…</Notice>}
        {phase.kind === 'error' && (
          <ErrorView message={phase.message} onRetry={() => (selector === null ? start() : loadNext(selector))} />
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

function StatsLine({ stats }: { stats: Stats }) {
  return (
    <Counts
      counts={[
        [stats.reviewed, 'reviewed'],
        [stats.new, 'new', 'text-sky-600 dark:text-sky-400'],
        [stats.again, 'again', 'text-rose-600 dark:text-rose-400'],
      ]}
    />
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
      <CardFace card={card} revealed={phase.kind === 'answer'} badges={<StateBadges state={card.state} />} />

      <div className="sticky bottom-0 -mx-4 mt-auto border-t border-stone-200 bg-stone-50/90 px-4 py-4 backdrop-blur sm:-mx-6 sm:px-6 dark:border-stone-800 dark:bg-stone-950/90">
        <div className="flex flex-wrap items-center gap-2">
          {phase.kind === 'prompt' ? (
            <button
              onClick={onReveal}
              disabled={busy}
              className={`flex-1 px-4 py-3 ${MAIN_BUTTON}`}
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
            className={`py-3 text-sm ${QUIET_BUTTON}`}
          >
            Suspend <Key>s</Key>
          </button>
        </div>
      </div>
    </div>
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
      <button onClick={onCheck} className={OUTLINE_BUTTON}>
        Check again
      </button>
    </div>
  );
}
