import { InformationCircleIcon } from '@heroicons/react/24/outline';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Outlet, useLocation, useNavigate, useOutletContext } from 'react-router';

import { queryOfSelector, selectorOfQuery } from './address';
import { api, type Card, type Rating } from './api';
import { CardDetailsDialog } from './CardDetails';
import { CardBadges, CardFace, Counts, DANGER_BUTTON, ErrorView, Key, MAIN_BUTTON, OUTLINE_BUTTON, QUIET_BUTTON, Waiting, WITH_KEY } from './CardParts';
import { useKeys, useSubmitted, type Submitted } from './hooks';
import { Modal } from './Modal';
import { OnChangeError } from './OnChange';
import { SelectorInput, TopBar } from './TopBar';

// What App gives the view of its route: the page's selector, one for every
// view, with how to submit another.
export type ViewContext = {
  submitted: Submitted;
  onSelector: (selector: string) => void;
};

export const useView = () => useOutletContext<ViewContext>();

// Where the session is. It runs the frontend loop of docs/frontends.md:
// get a card, show the prompt, show the answer and the context, rate it.
type Phase =
  | { kind: 'loading'; since: number }
  | { kind: 'prompt'; card: Card }
  | { kind: 'answer'; card: Card }
  | { kind: 'done' }
  | { kind: 'error'; message: string };

type Stats = { reviewed: number; new: number; again: number };

const RATINGS: { rating: Rating; name: string; style: string }[] = [
  { rating: 1, name: 'Again', style: 'bg-rose-600 enabled:hover:bg-rose-500' },
  { rating: 2, name: 'Hard', style: 'bg-amber-600 enabled:hover:bg-amber-500' },
  { rating: 3, name: 'Good', style: 'bg-emerald-600 enabled:hover:bg-emerald-500' },
  { rating: 4, name: 'Easy', style: 'bg-sky-600 enabled:hover:bg-sky-500' },
];

// How long the card shown stays while srl next makes the next one, before
// the page says it is waiting: most cards come sooner, and swap at once.
const WAIT_SHOWN_AFTER_MS = 300;

// The phase after getting the next card: its prompt, or done if none is left.
async function nextPhase(selector: string, signal: AbortSignal): Promise<Phase> {
  const card = await api.next(selector, signal);
  return card ? { kind: 'prompt', card } : { kind: 'done' };
}

// The page: the view of the route (src/main.tsx), which is mounted while
// its route is shown, and the error of the --on-change program.
export default function App() {
  const { pathname, search } = useLocation();

  // The selector in the address, or else none, which is every card, with
  // the number of selectors submitted, so that submitting the same one
  // again finds its cards again.
  const selector = selectorOfQuery(search);
  const [n, setN] = useState(0);
  const submitted = useMemo<Submitted>(() => ({ selector, n }), [selector, n]);
  // Submitting another selector goes to its address, a new entry of the
  // browser's history; submitting the one there again stays on it.
  const navigate = useNavigate();
  const onSelector = useCallback(
    (next: string) => {
      const search = queryOfSelector(next);
      navigate({ pathname, search }, { replace: search === queryOfSelector(selector) });
      setN((n) => n + 1);
    },
    [navigate, pathname, selector],
  );

  return (
    <div className="min-h-screen bg-stone-50 text-stone-900 dark:bg-stone-950 dark:text-stone-100">
      <Outlet context={{ submitted, onSelector } satisfies ViewContext} />
      <OnChangeError />
    </div>
  );
}

export function Study() {
  const { submitted, onSelector } = useView();
  const { selector } = submitted;
  const [phase, setPhase] = useState<Phase>(() => ({ kind: 'loading', since: performance.now() }));
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

  // The request for the next card, stopped when another is asked for or
  // Study is left: the server then stops srl, and srl the kind program
  // making the card.
  const asking = useRef<AbortController | null>(null);
  useEffect(() => () => asking.current?.abort(), []);

  // Shows the next card, or, if srl takes a while to make it, that it waits.
  // A request stopped shows nothing.
  const toNext = useCallback(async (sel: string) => {
    asking.current?.abort();
    const abort = new AbortController();
    asking.current = abort;
    const since = performance.now();
    const timer = setTimeout(() => setPhase({ kind: 'loading', since }), WAIT_SHOWN_AFTER_MS);
    try {
      setPhase(await nextPhase(sel, abort.signal));
    } catch (error) {
      if (!abort.signal.aborted) throw error;
    } finally {
      clearTimeout(timer);
    }
  }, []);

  const loadNext = useCallback((sel: string) => step(() => toNext(sel)), [step, toNext]);

  // Asks for a card when shown, and with each selector submitted.
  useSubmitted(submitted, loadNext);

  const reveal = useCallback(() => {
    if (phase.kind !== 'prompt') return;
    setPhase({ kind: 'answer', card: phase.card });
  }, [phase]);

  const rate = useCallback(
    (rating: Rating) => {
      if (phase.kind !== 'answer') return;
      const { card } = phase;
      step(async () => {
        await api.review(card.id, rating);
        setStats((s) => ({
          reviewed: s.reviewed + 1,
          new: s.new + (card.schedule === 'new' ? 1 : 0),
          again: s.again + (rating === 1 ? 1 : 0),
        }));
        await toNext(selector);
      });
    },
    [phase, selector, step, toNext],
  );

  // Suspending first asks why, in a dialog; the reason may be left empty.
  const [askingWhy, setAskingWhy] = useState(false);
  const askWhy = useCallback(() => {
    if (phase.kind !== 'prompt' && phase.kind !== 'answer') return;
    setAskingWhy(true);
  }, [phase]);

  const suspend = useCallback(
    (reason: string) => {
      setAskingWhy(false);
      if (phase.kind !== 'prompt' && phase.kind !== 'answer') return;
      const { card } = phase;
      step(async () => {
        await api.suspend(card.id, reason.trim());
        await toNext(selector);
      });
    },
    [phase, selector, step, toNext],
  );

  // The card's details, in a dialog.
  const [showingDetails, setShowingDetails] = useState(false);
  const showDetails = useCallback(() => {
    if (phase.kind !== 'prompt' && phase.kind !== 'answer') return;
    setShowingDetails(true);
  }, [phase]);

  // Keys: space or Enter shows the answer, 1–4 rate, s suspends, i shows the
  // card's details.
  useKeys((event) => {
    if (event.key === 'i') {
      showDetails();
      return;
    }
    if (busy) return;
    if (event.key === ' ' || event.key === 'Enter') {
      if (phase.kind !== 'prompt') return;
      event.preventDefault();
      reveal();
    } else if (['1', '2', '3', '4'].includes(event.key)) {
      rate(Number(event.key) as Rating);
    } else if (event.key === 's') {
      event.preventDefault();
      askWhy();
    }
  });

  return (
    <div className="flex min-h-screen flex-col">
      <TopBar view="study">
        <SelectorInput selector={selector} onSelector={onSelector} />
        <StatsLine stats={stats} />
      </TopBar>
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 py-8 sm:px-6">
        {phase.kind === 'loading' && (
          <Waiting title="Loading…" since={phase.since} />
        )}
        {phase.kind === 'error' && (
          <ErrorView message={phase.message} onRetry={() => loadNext(selector)} />
        )}
        {phase.kind === 'done' && (
          <DoneView selector={selector} stats={stats} onCheck={() => loadNext(selector)} />
        )}
        {(phase.kind === 'prompt' || phase.kind === 'answer') && (
          <>
            <CardView
              phase={phase}
              busy={busy}
              onReveal={reveal}
              onRate={rate}
              onSuspend={askWhy}
              onDetails={showDetails}
            />
            <CardDetailsDialog id={phase.card.id} open={showingDetails} onClose={() => setShowingDetails(false)} />
          </>
        )}
        <SuspendDialog open={askingWhy} onSuspend={suspend} onClose={() => setAskingWhy(false)} />
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
  onDetails,
}: {
  phase: Extract<Phase, { kind: 'prompt' | 'answer' }>;
  busy: boolean;
  onReveal: () => void;
  onRate: (rating: Rating) => void;
  onSuspend: () => void;
  onDetails: () => void;
}) {
  const { card } = phase;
  return (
    <div className="flex flex-1 flex-col gap-6">
      {/* A studied card is always new or due, so only new gets a label, and a
          leech, whose question likely needs rewriting. */}
      <CardFace
        card={card}
        revealed={phase.kind === 'answer'}
        scrollToAnswer
        badges={(card.schedule === 'new' || card.leech) && <CardBadges schedule={card.schedule} leech={card.leech} />}
        start={
          <button
            onClick={onDetails}
            title="Everything srl says about this card, and its history"
            className={`-ml-3 py-1 text-xs ${WITH_KEY} ${QUIET_BUTTON}`}
          >
            <InformationCircleIcon aria-hidden className="h-4 w-4" />
            Details <Key>i</Key>
          </button>
        }
      />

      <div className="sticky bottom-0 -mx-4 mt-auto border-t border-stone-200 bg-stone-50/90 px-4 py-4 backdrop-blur sm:-mx-6 sm:px-6 dark:border-stone-800 dark:bg-stone-950/90">
        <div className="flex flex-wrap items-center gap-2">
          {phase.kind === 'prompt' ? (
            <button
              onClick={onReveal}
              disabled={busy}
              className={`flex-1 px-4 py-3 ${WITH_KEY} ${MAIN_BUTTON}`}
            >
              Show answer <Key>space</Key>
            </button>
          ) : (
            RATINGS.map(({ rating, name, style }) => (
              <button
                key={rating}
                onClick={() => onRate(rating)}
                disabled={busy}
                className={`min-w-20 flex-1 rounded-lg px-3 py-3 ${WITH_KEY} font-medium text-white disabled:opacity-50 ${style}`}
              >
                {name} <Key>{rating}</Key>
              </button>
            ))
          )}
          <button
            onClick={onSuspend}
            disabled={busy}
            title="Suspend this card without recording a review"
            className={`py-3 text-sm ${WITH_KEY} ${QUIET_BUTTON}`}
          >
            Suspend <Key>s</Key>
          </button>
        </div>
      </div>
    </div>
  );
}

// Asks why the card is suspended, so that it can be fixed later: srl keeps
// the reason, and srl find --json gives it. Enter starts a new line, as in
// any text box, Ctrl+Enter (or ⌘+Enter) suspends, Escape cancels.
function SuspendDialog({
  open,
  onSuspend,
  onClose,
}: {
  open: boolean;
  onSuspend: (reason: string) => void;
  onClose: () => void;
}) {
  const [reason, setReason] = useState('');
  useEffect(() => {
    if (open) setReason('');
  }, [open]);

  return (
    <Modal open={open} title="Suspend this card" onClose={onClose}>
      <label className="flex flex-col gap-2.5 text-sm text-stone-500">
        Why? What to fix before studying it again (optional)
        <textarea
          autoFocus
          value={reason}
          onChange={(event) => setReason(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
              event.preventDefault();
              onSuspend(reason);
            }
          }}
          rows={3}
          placeholder="e.g. too hard for now; split it in two"
          className="rounded-lg border border-stone-300 bg-white p-2 text-base text-stone-900 dark:border-stone-700 dark:bg-stone-900 dark:text-stone-100"
        />
      </label>
      <button onClick={() => onSuspend(reason)} className={`self-start px-5 py-2.5 ${WITH_KEY} ${DANGER_BUTTON}`}>
        Suspend <Key>ctrl+enter</Key>
      </button>
    </Modal>
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
