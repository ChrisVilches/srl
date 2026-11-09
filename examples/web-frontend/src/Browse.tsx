import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router';

import { api, type FoundCard, type ShownCard } from './api';
import { CardDetails, CardDetailsSkeleton } from './CardDetails';
import { CardBadges, CardFace, Counts, ErrorView, KindBadge, Key, Loading, Notice, QUIET_BUTTON, Skeleton, Waiting, WITH_KEY } from './CardParts';
import { EditItem } from './EditItem';
import { JsonView } from './JsonView';
import { useView } from './App';
import { useKeys, useSettled, useSubmitted, type Submitted } from './hooks';
import { SelectorInput, TopBar } from './TopBar';

// The card browser: the cards of a selector as `srl find` lists them, and any
// one of them made and shown as studying shows it, without studying it. For
// looking at what a store holds, and at how a kind's cards look.

// While loading, the cards found before, if any: the chosen card stays shown
// as it was until the new cards say whether it is among them.
type List =
  | { kind: 'loading'; before: FoundCard[] }
  | { kind: 'ready'; cards: FoundCard[] }
  | { kind: 'error'; message: string };

// The cards found for a submission, as kept by TanStack Query (src/main.tsx).
const findKey = ({ selector, n }: Submitted) => ['find', selector, n];

// A card being made, made (with how long it took), or that couldn't be.
type Made =
  | { kind: 'making'; since: number }
  | { kind: 'ready'; card: ShownCard }
  | { kind: 'error'; message: string };

// How the chosen card is shown: as studying shows its prompt, as it shows it
// once revealed (with the context), as srl cards --json prints it, its item
// in a form to edit it, or its details (src/CardDetails.tsx).
type Side = 'prompt' | 'revealed' | 'json' | 'edit' | 'details';
const SIDES: Side[] = ['prompt', 'revealed', 'json', 'edit', 'details'];

// The sides that show the card made from the item, and so make it: Edit
// shows the item, and Details what srl find and srl log say about the card.
const MADE_SIDES: Side[] = ['prompt', 'revealed', 'json'];

// The chosen card and side are kept in the address's path, and only there,
// as /browse/CARD/SIDE (the route's :card and :side, src/main.tsx), with the
// side left out when it is the prompt: /browse, /browse/x7k2m9,
// /browse/x7k2m9/edit (see src/address.ts). An unknown side is the prompt.
function browsePath(card: string | null, side: Side): string {
  const parts = ['browse', ...(card === null ? [] : [card, ...(side === 'prompt' ? [] : [side])])];
  return '/' + parts.map(encodeURIComponent).join('/');
}

// How long a card stays chosen before it is made and its history read, so
// that moving through the list with the keys runs no kind program and no
// srl log for the cards passed by.
const CHOICE_DELAY = 300; // ms

// The keys that choose the card below or above.
const MOVES: Record<string, number> = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 };

export function Browse() {
  const { submitted, onSelector } = useView();
  const { selector } = submitted;
  const [made, setMade] = useState<Record<string, Made>>({});

  // The cards of the selector submitted: of this submission, so that a
  // selector submitted again finds its cards anew. The list is always that
  // of the selector shown, never of one submitted before, whatever order
  // srl answers in; the cards found before are only kept as `before`.
  const found = useQuery({ queryKey: findKey(submitted), queryFn: () => api.find(selector), placeholderData: keepPreviousData });
  const list: List =
    found.isSuccess && !found.isPlaceholderData
      ? { kind: 'ready', cards: found.data }
      : found.isError && !found.isFetching
        ? { kind: 'error', message: found.error.message }
        : { kind: 'loading', before: found.data ?? [] };
  const queryClient = useQueryClient();

  // Choosing a card replaces the address, so that moving through the list
  // stays out of the browser's history; turning to another side adds to
  // it, so back turns back.
  const params = useParams();
  const selected = params.card ?? null;
  const side = SIDES.find((s) => s === params.side) ?? 'prompt';
  const navigate = useNavigate();
  const { search } = useLocation();
  const setSelected = (card: string) => navigate({ pathname: browsePath(card, side), search }, { replace: true });
  const setSide = (s: Side) => {
    if (s !== side) navigate({ pathname: browsePath(selected, s), search });
  };
  // The card being made, to stop when another is chosen or Browse is left.
  const making = useRef<{ id: string; abort: AbortController } | null>(null);
  useEffect(() => () => making.current?.abort.abort(), []);

  // Each selector submitted forgets every card made, so that it makes its
  // cards anew.
  const forgetMade = useCallback(() => {
    making.current?.abort.abort();
    setMade({});
  }, []);
  useSubmitted(submitted, forgetMade);

  // After the item ITEM was saved, finds the cards again in place: the list
  // stays as it is until the new one replaces it, rows kept by card ID, so
  // its scroll stays; only the cards of ITEM (its ID, or its ID and a
  // suffix) are forgotten, to be made anew. Throws srl's error, if any.
  const refresh = useCallback(
    async (item: string) => {
      const ofItem = (id: string) => id === item || id.startsWith(`${item}.`);
      if (making.current && ofItem(making.current.id)) making.current.abort.abort();
      setMade((m) => Object.fromEntries(Object.entries(m).filter(([id]) => !ofItem(id))));
      queryClient.setQueryData(findKey(submitted), await api.find(selector));
    },
    [selector, submitted, queryClient],
  );

  // Makes card ID with srl cards, stopping the card being made, if any.
  // Each card is made once, when first chosen, and again only when asked.
  const make = useCallback((id: string) => {
    making.current?.abort.abort();
    const abort = new AbortController();
    making.current = { id, abort };
    const since = performance.now();
    setMade((m) => ({ ...m, [id]: { kind: 'making', since } }));
    api.card(id, abort.signal).then(
      (card) => setMade((m) => ({ ...m, [id]: { kind: 'ready', card } })),
      (error: Error) => {
        if (!abort.signal.aborted) {
          setMade((m) => ({ ...m, [id]: { kind: 'error', message: error.message } }));
        } else {
          // Forgotten, unless the card is already being made again.
          setMade(({ [id]: entry, ...rest }) =>
            entry?.kind === 'making' && entry.since !== since ? { ...rest, [id]: entry } : rest,
          );
        }
      },
    ).finally(() => {
      if (making.current?.abort === abort) making.current = null;
    });
  }, []);

  // Choosing another card stops the one being made. The chosen card is made
  // once it has stayed chosen for CHOICE_DELAY (settled). It is
  // made only on a side that shows it (MADE_SIDES), so choosing a card on
  // Edit or Details, or saving its item, makes nothing until another side is
  // shown. Only a card of the list is made: one named by the address that
  // isn't among the selector's cards is never asked for.
  const cards = list.kind === 'ready' ? list.cards : [];
  const listed = cards.some((c) => c.id === selected);
  const showsMade = MADE_SIDES.includes(side);
  const settled = useSettled(selected, CHOICE_DELAY);
  useEffect(() => {
    if (making.current && making.current.id !== selected) making.current.abort.abort();
    if (!showsMade || selected === null || settled !== selected || !listed || made[selected] !== undefined) return;
    make(selected);
  }, [showsMade, selected, settled, listed, made, make]);

  // Keys: ↑/↓ (or k/j) choose a card, r makes it again once it has been made
  // (on a side that shows it).
  useKeys((event) => {
    const at = cards.findIndex((c) => c.id === selected);
    const move = MOVES[event.key];
    if (move !== undefined) {
      event.preventDefault();
      if (cards.length > 0) setSelected(cards[Math.max(0, Math.min(at + move, cards.length - 1))].id);
    } else if (event.key === 'r' && showsMade && selected !== null && made[selected] !== undefined && made[selected].kind !== 'making') {
      make(selected);
    }
  });

  return (
    <div className="flex h-dvh flex-col overflow-hidden">
      <TopBar view="browse">
        <SelectorInput selector={selector} onSelector={onSelector} />
        {list.kind === 'loading' && <Skeleton className="h-4 w-16" />}
        {list.kind === 'ready' && <Counts counts={[[cards.length, cards.length === 1 ? 'card' : 'cards']]} />}
      </TopBar>
      <main className="mx-auto grid min-h-0 w-full max-w-7xl flex-1 gap-4 grid-rows-[minmax(0,2fr)_minmax(0,3fr)] lg:grid-cols-[26rem_minmax(0,1fr)] lg:grid-rows-[minmax(0,1fr)]">
        <div className="overflow-y-auto border-b border-stone-200 [scrollbar-gutter:stable] lg:border-r lg:border-b-0 dark:border-stone-800">
          {list.kind === 'loading' && <ListSkeleton />}
          {list.kind === 'error' && (
            <div className="p-6">
              <ErrorView message={list.message} onRetry={() => found.refetch()} />
            </div>
          )}
          {list.kind === 'ready' && cards.length === 0 && <p className="p-6 text-stone-400">No card matches.</p>}
          {list.kind === 'ready' && (
            <CardList cards={cards} selected={selected} made={made} onChoose={setSelected} />
          )}
        </div>
        <div className="overflow-y-auto px-4 py-6 [scrollbar-gutter:stable] sm:px-6">
          {/* Until the cards are found, a card the address names may or may
              not be among them: the preview waits, with its sides but not
              the card's details. A card chosen among the cards found before
              stays as it was meanwhile, so that its Edit form isn't read
              again, nor its edits lost, when another selector is submitted. */}
          {(selected === null || list.kind !== 'error') && (
            <Preview
              loading={selected !== null && list.kind === 'loading'}
              missing={selected !== null && list.kind === 'ready' && !listed ? selected : null}
              found={(list.kind === 'loading' ? list.before : cards).find((c) => c.id === selected) ?? null}
              made={selected === null ? undefined : made[selected]}
              settled={selected !== null && settled === selected}
              side={side}
              onSide={setSide}
              onMake={() => selected !== null && make(selected)}
              onSaved={refresh}
            />
          )}
        </div>
      </main>
    </div>
  );
}

// The widths of the excerpts in the list's skeleton, varied so it looks
// like a list.
const EXCERPT_WIDTHS = ['w-3/4', 'w-1/2', 'w-2/3', 'w-5/6', 'w-2/5', 'w-3/5', 'w-3/4', 'w-1/2'];

// The list while srl finds the cards: rows of an ID and an excerpt. No bar
// for a collection, whose background would stop short of the
// gutter kept for the scrollbar, leaving a gap.
function ListSkeleton() {
  return (
    <Loading label="Finding cards">
      {EXCERPT_WIDTHS.map((width, i) => (
        <div key={i} className="border-b border-stone-100 px-4 py-2.5 dark:border-stone-900">
          <Skeleton className="h-3 w-14" />
          <Skeleton className={`mt-2 h-4 ${width}`} />
        </div>
      ))}
    </Loading>
  );
}

// Centers the chosen card's row in the list when it is out of sight, as the
// callback ref of that row only. React calls it when the row's element
// appears: when the cards come, as after the page opened on a card of the
// address, and when the choice moves to another row. A row clicked is in
// sight, so clicking never scrolls; a key choosing a row past the edge
// does. A refresh that keeps the rows (by card ID) calls it no more, so the
// list stays where it was. A function outside the component, so that it is
// the same one at every render.
function scrollToRow(row: HTMLButtonElement | null) {
  const list = row?.closest('.overflow-y-auto');
  if (!row || !list) return;
  const r = row.getBoundingClientRect();
  const l = list.getBoundingClientRect();
  if (r.bottom <= l.top || r.top >= l.bottom) row.scrollIntoView({ block: 'center' });
}

// The cards in the order srl find gives them, under the name of each collection.
function CardList({
  cards,
  selected,
  made,
  onChoose,
}: {
  cards: FoundCard[];
  selected: string | null;
  made: Record<string, Made>;
  onChoose: (id: string) => void;
}) {
  return (
    <ul>
      {cards.map((card, i) => (
        <li key={card.id}>
          {(i === 0 || cards[i - 1].collection !== card.collection) && (
            <div className="sticky top-0 border-b border-stone-200 bg-stone-100/95 px-4 py-1.5 font-mono text-xs text-stone-500 backdrop-blur dark:border-stone-800 dark:bg-stone-900/95">
              {card.collection}
            </div>
          )}
          <button
            ref={card.id === selected ? scrollToRow : undefined}
            onClick={() => onChoose(card.id)}
            className={`block w-full border-b border-stone-100 px-4 py-2.5 text-left dark:border-stone-900 ${
              card.id === selected
                ? 'bg-amber-50 dark:bg-amber-400/10'
                : 'hover:bg-stone-100 dark:hover:bg-stone-900'
            }`}
          >
            <div className="flex items-center gap-2 text-xs text-stone-400">
              <span className="font-mono text-stone-600 dark:text-stone-300">{card.id}</span>
              <KindBadge kind={card.kind} />
              <CardBadges status={card.status} schedule={card.schedule} leech={card.leech} />
              {made[card.id]?.kind === 'making' && <span className="text-amber-600">making…</span>}
            </div>
            <div className="mt-1 truncate text-sm">{card.excerpt}</div>
          </button>
        </li>
      ))}
    </ul>
  );
}

// The chosen card, on the chosen side. While LOADING, the cards being found,
// the sides are shown, with a skeleton for the form on Edit and for the
// details on Details; the other sides show nothing until the card is made,
// which Making shows.
// TODO: Prompt, Revealed and JSON are blank under the sides until Making
// shows, while the cards are found and for CHOICE_DELAY after; they should
// show something from the start. See the README's TODO.
function Preview({
  loading,
  found,
  made,
  settled,
  side,
  onSide,
  onMake,
  onSaved,
  missing,
}: {
  loading: boolean;
  found: FoundCard | null;
  // The card the address names when the selector's cards don't include it.
  missing: string | null;
  made: Made | undefined;
  // Whether the card has stayed chosen for CHOICE_DELAY: its history is
  // read only then.
  settled: boolean;
  side: Side;
  onSide: (side: Side) => void;
  onMake: () => void;
  // Called with the item's ID once it is saved, staying on the edit side.
  onSaved: (item: string) => Promise<void>;
}) {
  if (missing !== null) {
    return (
      <Notice>
        No card <code className="font-mono">{missing}</code> is among the cards of this selector. Choose another one.{' '}
        <Key>↑</Key>
        <Key>↓</Key>
      </Notice>
    );
  }
  if (found === null && !loading) {
    return (
      <Notice>
        Choose a card to make it and see it as studying shows it. <Key>↑</Key>
        <Key>↓</Key>
      </Notice>
    );
  }
  const sides: { side: Side; name: string }[] = [
    { side: 'prompt', name: 'Prompt' },
    { side: 'revealed', name: 'Revealed' },
    { side: 'json', name: 'JSON' },
    { side: 'edit', name: 'Edit' },
    { side: 'details', name: 'Details' },
  ];
  const showsMade = MADE_SIDES.includes(side);
  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
        <div className="flex rounded-lg border border-stone-300 p-0.5 dark:border-stone-700">
          {sides.map(({ side: s, name }) => (
            <button
              key={s}
              onClick={() => onSide(s)}
              className={`rounded-md px-3 py-1 ${
                side === s ? 'bg-stone-200 font-medium dark:bg-stone-800' :'text-stone-500 hover:text-stone-900 dark:hover:text-stone-100'
              }`}
            >
              {name}
            </button>
          ))}
        </div>
        {/* Edit and Details don't show the card made from the item. The
            button stays disabled until the card has been made once, rather
            than turning on while the card waits for CHOICE_DELAY and off as
            it is made. */}
        {showsMade && (
          <button
            onClick={onMake}
            disabled={found === null || made === undefined || made.kind === 'making'}
            title="Make the card again: its kind program runs again"
            className={`py-1 ${WITH_KEY} ${QUIET_BUTTON}`}
          >
            Make again <Key>r</Key>
          </button>
        )}
      </div>

      {side === 'details' && (
        <section className="rounded-2xl border border-stone-200 px-5 py-4 dark:border-stone-800">
          {found === null ? <CardDetailsSkeleton /> : <CardDetails card={found} readHistory={settled} />}
        </section>
      )}
      {/* Until the cards are found, the form waits for the card's item, as
          one skeleton from the page's opening to the form. */}
      {side === 'edit' && <EditItem id={found === null ? null : found.item} onSaved={onSaved} />}
      {found !== null && showsMade && made?.kind === 'making' && <Making kind={found.kind} since={made.since} />}
      {showsMade && made?.kind === 'error' && <ErrorView message={made.message} onRetry={onMake} />}
      {found !== null &&
        showsMade &&
        made?.kind === 'ready' &&
        (side === 'json' ? (
          <JsonView
            value={{
              id: made.card.id,
              prompt: made.card.prompt,
              prompt_revealed: made.card.prompt_revealed,
              answer: made.card.answer,
            }}
          />
        ) : (
          <CardFace
            card={made.card}
            revealed={side === 'revealed'}
            badges={<CardBadges status={found.status} schedule={found.schedule} leech={found.leech} />}
          />
        ))}
    </div>
  );
}

// A card being made, with how long it has taken so far: a kind program can
// take a while. A card of a built-in kind gets no line under it.
function Making({ kind, since }: { kind: string; since: number }) {
  return (
    <Waiting title="Making the card…" since={since}>
      {kind !== 'qa' && (
        <>
          Being made by the program <code className="font-mono">{kind}</code>.
        </>
      )}
    </Waiting>
  );
}
