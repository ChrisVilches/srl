import { useCallback, useEffect, useRef, useState } from 'react';

import { api, type FoundCard, type ShownCard } from './api';
import { CardFace, Counts, ErrorView, KindBadge, Key, Loading, Notice, QUIET_BUTTON, Skeleton, StateBadges, Waiting, WITH_KEY } from './CardParts';
import { EditItem } from './EditItem';
import { JsonView } from './JsonView';
import type { ViewProps } from './App';
import { useKeys, useSubmitted } from './hooks';
import { SelectorInput, TopBar } from './TopBar';

// The card browser: the cards of a selector as `srl find` lists them, and any
// one of them made and shown as studying shows it, without studying it. For
// looking at what a store holds, and at how a kind's cards look.

type List = { kind: 'loading' } | { kind: 'ready'; cards: FoundCard[] } | { kind: 'error'; message: string };

// A card being made, made (with how long it took), or that couldn't be.
type Made =
  | { kind: 'making'; since: number }
  | { kind: 'ready'; card: ShownCard }
  | { kind: 'error'; message: string };

// How the chosen card is shown: as studying shows its prompt, as it shows it
// once revealed (with the context), as srl cards --json prints it, or its
// item in a form to edit it.
type Side = 'prompt' | 'revealed' | 'json' | 'edit';
const SIDES: Side[] = ['prompt', 'revealed', 'json', 'edit'];

// The chosen card and side are kept in the address, as #browse/CARD/SIDE,
// with the side left out when it is the prompt: #browse, #browse/x7k2m9,
// #browse/x7k2m9/edit. They are read when the page opens, and written
// whenever they change while Browse is shown, so reloading the page comes
// back to them. An unknown side is the prompt.
function browseHash(): { card: string | null; side: Side } {
  const [, card, side] = window.location.hash.slice(1).split('/').map(decodeURIComponent);
  return { card: card || null, side: SIDES.find((s) => s === side) ?? 'prompt' };
}

function setBrowseHash(card: string | null, side: Side) {
  const parts = ['browse', ...(card === null ? [] : [card, ...(side === 'prompt' ? [] : [side])])];
  const url = new URL(window.location.href);
  url.hash = parts.map(encodeURIComponent).join('/');
  // replaceState fires no hashchange, and keeps moving through the list
  // out of the browser's history.
  window.history.replaceState(null, '', url);
}

const MAKE_DELAY = 300; // ms

// The keys that choose the card below or above.
const MOVES: Record<string, number> = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 };

export function Browse({ active, submitted, onSelector }: ViewProps) {
  const { selector } = submitted;
  const [list, setList] = useState<List>({ kind: 'loading' });
  const [selected, setSelected] = useState<string | null>(() => browseHash().card);
  const [made, setMade] = useState<Record<string, Made>>({});
  const [side, setSide] = useState<Side>(() => browseHash().side);
  // Written again when Browse is shown, since switching to it by its tab
  // sets the address to #browse alone.
  useEffect(() => {
    if (active) setBrowseHash(selected, side);
  }, [active, selected, side]);
  // The card being made, to stop when another is chosen.
  const making = useRef<{ id: string; abort: AbortController } | null>(null);

  // Finding forgets every card made, so that a selector submitted again
  // makes its cards anew.
  const find = useCallback(async (sel: string) => {
    making.current?.abort.abort();
    setMade({});
    setList({ kind: 'loading' });
    try {
      setList({ kind: 'ready', cards: await api.find(sel) });
    } catch (error) {
      setList({ kind: 'error', message: (error as Error).message });
    }
  }, []);

  // Finds the cards of each selector submitted, once shown.
  useSubmitted(active, submitted, find);

  // After the item ITEM was saved, finds the cards again in place: the list
  // stays as it is until the new one replaces it, rows kept by card ID, so
  // its scroll stays; only the cards of ITEM (its ID, or its ID and a
  // suffix) are forgotten, to be made anew. Throws srl's error, if any.
  const refresh = useCallback(
    async (item: string) => {
      const ofItem = (id: string) => id === item || id.startsWith(`${item}.`);
      if (making.current && ofItem(making.current.id)) making.current.abort.abort();
      setMade((m) => Object.fromEntries(Object.entries(m).filter(([id]) => !ofItem(id))));
      setList({ kind: 'ready', cards: await api.find(selector) });
    },
    [selector],
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
  // once it has stayed chosen for MAKE_DELAY, so that moving through the
  // list with the keys runs no kind program for the cards passed by. It is
  // made only on a side that shows it: Edit shows the item, so choosing a
  // card or saving its item there makes nothing until another side is shown.
  // Only a card of the list is made: one named by the address that isn't
  // among the selector's cards is never asked for.
  const cards = list.kind === 'ready' ? list.cards : [];
  const listed = cards.some((c) => c.id === selected);
  useEffect(() => {
    if (making.current && making.current.id !== selected) making.current.abort.abort();
    if (side === 'edit' || selected === null || !listed || made[selected] !== undefined) return;
    const timer = setTimeout(() => make(selected), MAKE_DELAY);
    return () => clearTimeout(timer);
  }, [side, selected, listed, made, make]);

  // Keys: ↑/↓ (or k/j) choose a card, r makes it again (but on the edit side).
  useKeys(active, (event) => {
    const at = cards.findIndex((c) => c.id === selected);
    const move = MOVES[event.key];
    if (move !== undefined) {
      event.preventDefault();
      if (cards.length > 0) setSelected(cards[Math.max(0, Math.min(at + move, cards.length - 1))].id);
    } else if (event.key === 'r' && side !== 'edit' && selected !== null && made[selected]?.kind !== 'making') {
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
              <ErrorView message={list.message} onRetry={() => find(selector)} />
            </div>
          )}
          {list.kind === 'ready' && cards.length === 0 && <p className="p-6 text-stone-400">No card matches.</p>}
          {list.kind === 'ready' && (
            <CardList cards={cards} selected={selected} made={made} onChoose={setSelected} />
          )}
        </div>
        <div className="overflow-y-auto px-4 py-6 [scrollbar-gutter:stable] sm:px-6">
          {/* Until the cards are found, a card the address names may or may
              not be among them: the preview waits, in the shape of a card. */}
          {selected !== null && list.kind === 'loading' && <PreviewSkeleton />}
          {(selected === null || list.kind === 'ready') && (
            <Preview
              missing={selected !== null && !listed ? selected : null}
              found={cards.find((c) => c.id === selected) ?? null}
              made={selected === null ? undefined : made[selected]}
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

// The list while srl finds the cards: a collection's address, then rows of
// an ID and an excerpt.
function ListSkeleton() {
  return (
    <Loading label="Finding cards">
      <div className="border-b border-stone-200 bg-stone-100/95 px-4 py-2 dark:border-stone-800 dark:bg-stone-900/95">
        <Skeleton className="h-3 w-40" />
      </div>
      {EXCERPT_WIDTHS.map((width, i) => (
        <div key={i} className="border-b border-stone-100 px-4 py-2.5 dark:border-stone-900">
          <Skeleton className="h-3 w-14" />
          <Skeleton className={`mt-2 h-4 ${width}`} />
        </div>
      ))}
    </Loading>
  );
}

// The card the address names, while srl finds the cards: the sides, then a
// card with its ID and a question.
function PreviewSkeleton() {
  return (
    <Loading label="Finding the card" className="flex flex-col gap-6">
      <Skeleton className="h-8 w-72 max-w-full" />
      <div className="rounded-2xl border border-stone-200 bg-white px-6 pt-4 pb-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
        <Skeleton className="h-3 w-16" />
        <Skeleton className="mt-4 h-3 w-20" />
        <Skeleton className="mt-3 h-6 w-3/4" />
        <Skeleton className="mt-2 h-6 w-1/2" />
      </div>
    </Loading>
  );
}

// Scrolls the list to the chosen card's row, as the callback ref of that row
// only. React calls it when the row's element appears: when the cards come,
// as after the page opened on a card of the address, and when the choice
// moves to another row. A refresh that keeps the rows (by card ID) calls it
// no more, so the list stays where it was. A function outside the
// component, so that it is the same one at every render.
function scrollToRow(row: HTMLButtonElement | null) {
  row?.scrollIntoView({ block: 'center' });
}

// The cards in the order srl find gives them, under the address of each collection.
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
          {(i === 0 || cards[i - 1].address !== card.address) && (
            <div className="sticky top-0 border-b border-stone-200 bg-stone-100/95 px-4 py-1.5 font-mono text-xs text-stone-500 backdrop-blur dark:border-stone-800 dark:bg-stone-900/95">
              {card.address}
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
              <StateBadges status={card.status} state={card.state} />
              {made[card.id]?.kind === 'making' && <span className="text-amber-600">making…</span>}
              {card.line !== null && <span className="ml-auto font-mono">:{card.line}</span>}
            </div>
            <div className="mt-1 truncate text-sm">{card.excerpt}</div>
          </button>
        </li>
      ))}
    </ul>
  );
}

function Preview({
  found,
  made,
  side,
  onSide,
  onMake,
  onSaved,
  missing,
}: {
  found: FoundCard | null;
  // The card the address names when the selector's cards don't include it.
  missing: string | null;
  made: Made | undefined;
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
  if (found === null) {
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
  ];
  const editing = side === 'edit';
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
        {/* Edit shows the item, not the card made from it. */}
        {!editing && (
          <button
            onClick={onMake}
            disabled={made?.kind === 'making'}
            title="Make the card again: its kind program runs again"
            className={`py-1 ${WITH_KEY} ${QUIET_BUTTON}`}
          >
            Make again <Key>r</Key>
          </button>
        )}
        <span className="ml-auto min-w-0 truncate font-mono text-xs text-stone-400" title={found.address}>
          {found.address}
          {found.line === null ? '' : `:${found.line}`} · due {found.due ?? '—'}
        </span>
      </div>

      {found.suspend_reason !== null && (
        <div className="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm dark:border-amber-500/40 dark:bg-amber-400/10">
          <div className="font-medium text-amber-800 dark:text-amber-300">Suspended because</div>
          <p className="mt-1 whitespace-pre-wrap">{found.suspend_reason}</p>
        </div>
      )}
      {editing && <EditItem id={found.item} onSaved={() => onSaved(found.item)} />}
      {!editing && made?.kind === 'making' && <Making kind={found.kind} since={made.since} />}
      {!editing && made?.kind === 'error' && <ErrorView message={made.message} onRetry={onMake} />}
      {!editing &&
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
            badges={<StateBadges status={found.status} state={found.state} />}
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
