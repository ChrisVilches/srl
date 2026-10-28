import { useCallback, useEffect, useRef, useState } from 'react';

import { api, type FoundCard, type ShownCard } from './api';
import { CardFace, Counts, ErrorView, KindBadge, Key, Notice, QUIET_BUTTON, StateBadges, Waiting } from './CardParts';
import { useKeys, useStartWhenShown } from './hooks';
import { SelectorInput, TopBar } from './TopBar';

// The card browser: the cards of a selector as `srl find` lists them, and any
// one of them made and shown as studying shows it, without studying it. For
// looking at what a store holds, and at how a kind's cards look.

type List = { kind: 'loading' } | { kind: 'ready'; cards: FoundCard[] } | { kind: 'error'; message: string };

// A card being made, made (with how long it took), or that couldn't be.
type Made =
  | { kind: 'making'; since: number }
  | { kind: 'ready'; card: ShownCard; ms: number }
  | { kind: 'error'; message: string };

// How a made card is shown: as studying shows its prompt, as it shows it
// once revealed (with the context), or as srl cards --json prints it.
type Side = 'prompt' | 'revealed' | 'json';

const MAKE_DELAY = 300; // ms

// The keys that choose the card below or above.
const MOVES: Record<string, number> = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 };

export function Browse({ active }: { active: boolean }) {
  const [selector, setSelector] = useState<string | null>(null);
  const [list, setList] = useState<List>({ kind: 'loading' });
  const [selected, setSelected] = useState<string | null>(null);
  const [made, setMade] = useState<Record<string, Made>>({});
  const [side, setSide] = useState<Side>('prompt');
  // The card being made, to stop when another is chosen.
  const making = useRef<{ id: string; abort: AbortController } | null>(null);

  // Finding again forgets the cards made, so that edited items are made anew.
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

  // Starts when first shown, with the selector srl-web was started with.
  const start = useCallback(
    () =>
      api.selector().then(
        (sel) => {
          setSelector(sel);
          find(sel);
        },
        (error: Error) => setList({ kind: 'error', message: error.message }),
      ),
    [find],
  );
  useStartWhenShown(active, start);

  // Makes card ID with srl cards, stopping the card being made, if any.
  // Each card is made once, when first chosen, and again only when asked.
  const make = useCallback((id: string) => {
    making.current?.abort.abort();
    const abort = new AbortController();
    making.current = { id, abort };
    const since = performance.now();
    setMade((m) => ({ ...m, [id]: { kind: 'making', since } }));
    api.card(id, abort.signal).then(
      (card) => setMade((m) => ({ ...m, [id]: { kind: 'ready', card, ms: performance.now() - since } })),
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
  // list with the keys runs no kind program for the cards passed by.
  useEffect(() => {
    if (making.current && making.current.id !== selected) making.current.abort.abort();
    if (selected === null || made[selected] !== undefined) return;
    const timer = setTimeout(() => make(selected), MAKE_DELAY);
    return () => clearTimeout(timer);
  }, [selected, made, make]);

  const cards = list.kind === 'ready' ? list.cards : [];

  // Keys: ↑/↓ (or k/j) choose a card, space or Enter turns it, r makes it again.
  useKeys(active, (event) => {
    const at = cards.findIndex((c) => c.id === selected);
    const move = MOVES[event.key];
    if (move !== undefined) {
      event.preventDefault();
      if (cards.length > 0) setSelected(cards[Math.max(0, Math.min(at + move, cards.length - 1))].id);
    } else if (event.key === ' ' || event.key === 'Enter') {
      event.preventDefault();
      setSide((s) => (s === 'prompt' ? 'revealed' : 'prompt'));
    } else if (event.key === 'r' && selected !== null && made[selected]?.kind !== 'making') {
      make(selected);
    }
  });

  return (
    <div className="flex h-dvh flex-col overflow-hidden">
      <TopBar view="browse">
        <SelectorInput
          selector={selector}
          placeholder="selector, as for srl find (empty: every card)"
          onSelector={(sel) => {
            setSelector(sel);
            find(sel);
          }}
        />
        {list.kind === 'ready' && <Counts counts={[[cards.length, cards.length === 1 ? 'card' : 'cards']]} />}
      </TopBar>
      <main className="mx-auto grid min-h-0 w-full max-w-7xl flex-1 gap-4 grid-rows-[minmax(0,2fr)_minmax(0,3fr)] lg:grid-cols-[26rem_minmax(0,1fr)] lg:grid-rows-[minmax(0,1fr)]">
        <div className="overflow-y-auto border-b border-stone-200 [scrollbar-gutter:stable] lg:border-r lg:border-b-0 dark:border-stone-800">
          {list.kind === 'loading' && <p className="p-6 text-stone-400">Finding cards…</p>}
          {list.kind === 'error' && (
            <div className="p-6">
              <ErrorView message={list.message} onRetry={() => find(selector ?? '')} />
            </div>
          )}
          {list.kind === 'ready' && cards.length === 0 && <p className="p-6 text-stone-400">No card matches.</p>}
          {list.kind === 'ready' && (
            <CardList cards={cards} selected={selected} made={made} onChoose={setSelected} />
          )}
        </div>
        <div className="overflow-y-auto px-4 py-6 [scrollbar-gutter:stable] sm:px-6">
          <Preview
            found={cards.find((c) => c.id === selected) ?? null}
            made={selected === null ? undefined : made[selected]}
            side={side}
            onSide={setSide}
            onMake={() => selected !== null && make(selected)}
          />
        </div>
      </main>
    </div>
  );
}

// The cards in the order srl find gives them, under the name of each note.
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
  const selectedRow = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    selectedRow.current?.scrollIntoView({ block: 'nearest' });
  }, [selected]);

  return (
    <ul>
      {cards.map((card, i) => (
        <li key={card.id}>
          {(i === 0 || cards[i - 1].path !== card.path) && (
            <div className="sticky top-0 border-b border-stone-200 bg-stone-100/95 px-4 py-1.5 font-mono text-xs text-stone-500 backdrop-blur dark:border-stone-800 dark:bg-stone-900/95">
              {card.path}
            </div>
          )}
          <button
            ref={card.id === selected ? selectedRow : undefined}
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
              <span className="ml-auto font-mono">:{card.line}</span>
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
}: {
  found: FoundCard | null;
  made: Made | undefined;
  side: Side;
  onSide: (side: Side) => void;
  onMake: () => void;
}) {
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
  ];
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
        <span className="text-xs text-stone-400">
          <Key>space</Key> turns
        </span>
        <button
          onClick={onMake}
          disabled={made?.kind === 'making'}
          title="Make the card again: its kind program runs again"
          className={`py-1 ${QUIET_BUTTON}`}
        >
          Make again <Key>r</Key>
        </button>
        <span className="ml-auto font-mono text-xs text-stone-400">
          {found.path}:{found.line} · due {found.due ?? '—'}
        </span>
      </div>

      {made?.kind === 'making' && <Making kind={found.kind} since={made.since} />}
      {made?.kind === 'error' && <ErrorView message={made.message} onRetry={onMake} />}
      {made?.kind === 'ready' && (
        <>
          {side === 'json' ? (
            <pre className="overflow-x-auto rounded-2xl border border-stone-200 bg-white p-5 font-mono text-sm whitespace-pre-wrap dark:border-stone-800 dark:bg-stone-900">
              {JSON.stringify(
                {
                  id: made.card.id,
                  prompt: made.card.prompt,
                  prompt_revealed: made.card.prompt_revealed,
                  answer: made.card.answer,
                },
                null,
                2,
              )}
            </pre>
          ) : (
            <CardFace
              card={made.card}
              revealed={side === 'revealed'}
              badges={<StateBadges status={found.status} state={found.state} />}
            />
          )}
          <p className="text-center text-xs text-stone-400">
            made by srl cards in {(made.ms / 1000).toFixed(1)} s
          </p>
        </>
      )}
    </div>
  );
}

// A card being made, with how long it has taken so far: a kind program can
// take a while.
function Making({ kind, since }: { kind: string; since: number }) {
  return (
    <Waiting title="Making the card…" since={since}>
      {kind === 'qa' ? (
        'srl cards is making it.'
      ) : (
        <>
          srl cards is running the kind program <code className="font-mono">{kind}</code>.
        </>
      )}
    </Waiting>
  );
}
