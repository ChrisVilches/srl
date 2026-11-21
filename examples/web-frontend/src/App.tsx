import { CheckIcon, InformationCircleIcon } from '@heroicons/react/24/outline';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Outlet, useLocation, useNavigate, useOutletContext } from 'react-router';

import { queryOfSearch, searchOfQuery } from './address';
import { api, type Card, type Rating, type Today } from './api';
import { CardDetailsDialog } from './CardDetails';
import { CardBadges, CardFace, CardSkeleton, Counts, DANGER_BUTTON, ErrorView, Key, MAIN_BUTTON, OUTLINE_BUTTON, QUIET_BUTTON, Skeleton, WITH_KEY } from './CardParts';
import { selectorOfSearch, type Search } from './filters';
import { FilterButton } from './Filters';
import { useKeys, useSubmitted, type Submitted } from './hooks';
import { Modal } from './Modal';
import { OnChangeError } from './OnChange';
import { queries } from './queries';
import { TopBar, VIEW_COLUMN, VIEW_SCROLL } from './TopBar';

// What App gives the view of its route: the page's search, one for every
// view, with how to submit another, and the selector srl gets for it.
export type ViewContext = {
  search: Search;
  submitted: Submitted;
  onSearch: (search: Search) => void;
};

export const useView = () => useOutletContext<ViewContext>();

// Where the session is. It runs the frontend loop of docs/frontends.md:
// get a card, show the prompt, show the answer and the context, rate it.
type Phase =
  | { kind: 'loading'; since: number }
  | { kind: 'prompt'; card: Card }
  | { kind: 'answer'; card: Card; ratable: boolean }
  | { kind: 'done' }
  | { kind: 'error'; message: string };

const RATINGS: { rating: Rating; name: string; style: string }[] = [
  { rating: 1, name: 'Again', style: 'bg-rose-600 enabled:hover:bg-rose-500' },
  { rating: 2, name: 'Hard', style: 'bg-amber-600 enabled:hover:bg-amber-500' },
  { rating: 3, name: 'Good', style: 'bg-emerald-600 enabled:hover:bg-emerald-500' },
  { rating: 4, name: 'Easy', style: 'bg-sky-600 enabled:hover:bg-sky-500' },
];

// How long the card shown stays while srl next makes the next one, before
// the page says it is waiting: most cards come sooner, and swap at once.
const WAIT_SHOWN_AFTER_MS = 300;

// How long the ratings wait after the answer shows before they can be
// pressed, so that a key or a tap pressed twice in a row doesn't rate the
// card before its answer is read.
const RATE_AFTER_MS = 1000;

// The phase after getting the next card: its prompt, or done if none is left.
async function nextPhase(selector: string, signal: AbortSignal): Promise<Phase> {
  const card = await api.next(selector, signal);
  return card ? { kind: 'prompt', card } : { kind: 'done' };
}

// The page: the view of the route (src/main.tsx), which is mounted while
// its route is shown, and the error of the --on-change program.
export default function App() {
  const { pathname, search: query } = useLocation();

  // The search in the address, or else none, which is every card, and its
  // selector, with the number of times a search was submitted again, so
  // that submitting the same one again finds its cards again.
  const search = useMemo(() => searchOfQuery(query), [query]);
  const selector = useMemo(() => selectorOfSearch(search), [search]);
  const [n, setN] = useState(0);
  const submitted = useMemo<Submitted>(() => ({ selector, n }), [selector, n]);
  // Submitting another search goes to its address, a new entry of the
  // browser's history, which is a new submission by itself; submitting the
  // one there again stays on it, and counts. Only one of the two changes,
  // never both: the router changes the address a render after React
  // changes N, so changing both would make a render with the old selector
  // and the new N, which would read the old selector's cards again.
  const navigate = useNavigate();
  const onSearch = useCallback(
    (next: Search) => {
      const nextQuery = queryOfSearch(next);
      if (nextQuery === queryOfSearch(search)) {
        setN((n) => n + 1);
      } else {
        navigate({ pathname, search: nextQuery });
      }
    },
    [navigate, pathname, search],
  );

  return (
    // No height of its own: the view's, h-dvh (src/TopBar.tsx). min-h-screen,
    // 100vh, is the window's height with a phone's address bar hidden, so
    // while the bar shows, the window could scroll by its height.
    <div className="bg-stone-50 text-stone-900 dark:bg-stone-950 dark:text-stone-100">
      <Outlet context={{ search, submitted, onSearch } satisfies ViewContext} />
      <OnChangeError />
    </div>
  );
}

export function Study() {
  const { search, submitted, onSearch } = useView();
  const { selector } = submitted;
  const [phase, setPhase] = useState<Phase>(() => ({ kind: 'loading', since: performance.now() }));
  // The counts are the study day's, from srl: they include reviews recorded
  // elsewhere, a reload shows them as they were, and each review reads them
  // again (src/main.tsx).
  const today = useQuery(queries.today()).data ?? null;
  const [busy, setBusy] = useState(false);

  // Runs one step of the loop, showing any error from srl instead of the card.
  const step = useCallback(async (action: () => Promise<unknown>) => {
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
    setPhase({ kind: 'answer', card: phase.card, ratable: false });
  }, [phase]);

  // The answer shown becomes ratable after RATE_AFTER_MS.
  useEffect(() => {
    if (phase.kind !== 'answer' || phase.ratable) return;
    const timer = setTimeout(() => setPhase({ ...phase, ratable: true }), RATE_AFTER_MS);
    return () => clearTimeout(timer);
  }, [phase]);

  const rate = useCallback(
    (rating: Rating) => {
      if (phase.kind !== 'answer' || !phase.ratable) return;
      const { card } = phase;
      step(async () => {
        await api.review(card.id, rating);
        await toNext(selector);
      });
    },
    [phase, selector, step, toNext],
  );

  // Suspending first asks why and for how long, in a dialog; both may be
  // left empty.
  const [askingWhy, setAskingWhy] = useState(false);
  const askWhy = useCallback(() => {
    if (phase.kind !== 'prompt' && phase.kind !== 'answer') return;
    setAskingWhy(true);
  }, [phase]);

  const suspend = useCallback(
    (reason: string, days: number | null) => {
      setAskingWhy(false);
      if (phase.kind !== 'prompt' && phase.kind !== 'answer') return;
      const { card } = phase;
      step(async () => {
        await api.suspend(card.id, reason.trim(), days);
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
    <div className="flex h-dvh flex-col">
      <TopBar view="study">
        {today && <StatsLine stats={today} />}
        {/* The text is in the filters. On a phone the button is at the
            right of the bar's second row, under the theme button. */}
        <FilterButton search={search} onSearch={onSearch} className="max-md:ml-auto" />
      </TopBar>
      <main className={VIEW_SCROLL}>
        <div className={VIEW_COLUMN}>
          {phase.kind === 'loading' && <NextCardSkeleton since={phase.since} />}
          {phase.kind === 'error' && (
            <ErrorView message={phase.message} onRetry={() => loadNext(selector)} />
          )}
          {phase.kind === 'done' && (
            <DoneView searched={selector !== ''} today={today} onCheck={() => loadNext(selector)} />
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
        </div>
      </main>
    </div>
  );
}

function StatsLine({ stats }: { stats: Today }) {
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

// The next card while srl makes it, or finds none is left: CardView's
// shape, the card and the bar of buttons. SINCE, when the card was asked
// for, tells one wait from the next. Worth knowing: the seconds count from
// when the skeleton shows, WAIT_SHOWN_AFTER_MS after SINCE, so they are
// that much under srl's time, and first show that much later.
function NextCardSkeleton({ since }: { since: number }) {
  return (
    <CardSkeleton of={since}>
      <div className="-mx-4 mt-auto -mb-8 border-t border-stone-200 px-4 py-4 sm:-mx-6 sm:px-6 dark:border-stone-800">
        <div className="flex items-center gap-2">
          <Skeleton className="h-12 flex-1" />
          <Skeleton className="h-12 w-24" />
        </div>
      </div>
    </CardSkeleton>
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

      {/* The bar takes the column's padding at its sides and bottom
          (VIEW_COLUMN), so it rests at the bottom of the view once
          scrolled to the end, where it sticks before. */}
      <div className="sticky bottom-0 -mx-4 mt-auto -mb-8 border-t border-stone-200 bg-stone-50/90 px-4 py-4 backdrop-blur sm:-mx-6 sm:px-6 dark:border-stone-800 dark:bg-stone-950/90">
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
                disabled={busy || !phase.ratable}
                className={`min-w-20 flex-1 rounded-lg px-3 py-3 ${WITH_KEY} font-medium text-white transition-opacity disabled:opacity-50 ${style}`}
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
// any text box, Escape cancels. It also asks for how many days, after which
// srl makes the card active again by itself; left empty, the card stays
// suspended until it is unsuspended. Suspending has no key, as a red button
// has none: it takes a click, and not while the days are not a whole number
// from 1 to 36500.
function SuspendDialog({
  open,
  onSuspend,
  onClose,
}: {
  open: boolean;
  onSuspend: (reason: string, days: number | null) => void;
  onClose: () => void;
}) {
  const [reason, setReason] = useState('');
  const [days, setDays] = useState('');
  useEffect(() => {
    if (open) {
      setReason('');
      setDays('');
    }
  }, [open]);
  const dayCount = days.trim() === '' ? null : Number(days);
  const daysValid = dayCount === null || (Number.isInteger(dayCount) && dayCount >= 1 && dayCount <= 36500);

  return (
    <Modal
      open={open}
      title="Suspend this card"
      closeLabel="Cancel"
      actions={
        <button
          onClick={() => onSuspend(reason, dayCount)}
          disabled={!daysValid}
          className={`px-4 py-2 text-sm ${DANGER_BUTTON}`}
        >
          Suspend
        </button>
      }
      onClose={onClose}
    >
      <label className="flex flex-col gap-4 text-sm text-stone-500">
        Why? What to fix before studying it again (optional)
        <textarea
          autoFocus
          value={reason}
          onChange={(event) => setReason(event.target.value)}
          rows={3}
          placeholder="e.g. too hard for now; split it in two"
          className="rounded-lg border border-stone-300 bg-white p-2 text-base text-stone-900 dark:border-stone-700 dark:bg-stone-900 dark:text-stone-100"
        />
      </label>
      <label className="flex flex-col gap-4 text-sm text-stone-500">
        For how many days? Empty: until you unsuspend it
        <input
          type="number"
          inputMode="numeric"
          min={1}
          max={36500}
          step={1}
          value={days}
          onChange={(event) => setDays(event.target.value)}
          placeholder="e.g. 7"
          aria-invalid={!daysValid}
          className="w-32 rounded-lg border border-stone-300 bg-white p-2 text-base text-stone-900 aria-[invalid=true]:border-rose-500 dark:border-stone-700 dark:bg-stone-900 dark:text-stone-100"
        />
      </label>
    </Modal>
  );
}

function DoneView({ searched, today, onCheck }: { searched: boolean; today: Today | null; onCheck: () => void }) {
  return (
    <div className="m-auto flex flex-col items-center gap-4 text-center">
      <CheckIcon aria-hidden className="h-10 w-10" />
      <h1 className="text-xl font-semibold">Nothing left to study</h1>
      <p className="text-stone-500">{searched ? 'No card is due for this search.' : 'No card is due.'}</p>
      {today && <StatsLine stats={today} />}
      <button onClick={onCheck} className={OUTLINE_BUTTON}>
        Check again
      </button>
    </div>
  );
}
