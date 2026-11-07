import { useEffect, useState, type ReactNode } from 'react';

import { api, type FoundCard, type LogEvent, type Rating } from './api';
import { Label, Loading, Skeleton } from './CardParts';
import { Modal } from './Modal';

// Everything srl says about a card, to trace it back to where it comes from
// and what happened to it: its IDs and kind, its collection and source,
// the note it names as its context, where it is in its schedule, and its
// history in the log. Study shows
// it in a modal (CardDetailsDialog), Browse above the chosen card.

const RATING_NAMES: Record<Rating, { name: string; color: string }> = {
  1: { name: 'Again', color: 'text-rose-600 dark:text-rose-400' },
  2: { name: 'Hard', color: 'text-amber-600 dark:text-amber-400' },
  3: { name: 'Good', color: 'text-emerald-600 dark:text-emerald-400' },
  4: { name: 'Easy', color: 'text-sky-600 dark:text-sky-400' },
};

// The details of CARD, as srl find --json lists it. Every kind is named here,
// a built-in one too, unlike on the card. Its history is read once
// READ_HISTORY is true: Browse waits until the card has stayed chosen.
export function CardDetails({ card, readHistory = true }: { card: FoundCard; readHistory?: boolean }) {
  return (
    <div className="flex flex-col gap-4 text-sm">
      {/* A card suspended without a reason says so, with no reason. */}
      {card.status === 'suspended' && (
        <div className="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 dark:border-amber-500/40 dark:bg-amber-400/10">
          {card.suspend_reason === null ? (
            <div className="font-medium text-amber-800 dark:text-amber-300">Suspended, with no reason given</div>
          ) : (
            <>
              <div className="font-medium text-amber-800 dark:text-amber-300">Suspended because</div>
              <p className="mt-1 whitespace-pre-wrap">{card.suspend_reason}</p>
            </>
          )}
        </div>
      )}
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 sm:grid-cols-[auto_minmax(0,1fr)_auto_minmax(0,1fr)]">
        <Entry name="Card" mono>
          {card.id}
        </Entry>
        <Entry name="Item" mono>
          {card.item}
        </Entry>
        <Entry name="Kind" mono>
          {card.kind}
        </Entry>
        <Entry name="Source" mono>
          {card.source}
        </Entry>
        <Entry name="Status">{card.status}</Entry>
        <Entry name="Schedule">{card.schedule}</Entry>
        <Entry name="Due" mono={card.due !== null}>
          {card.due ?? <None>not yet, new</None>}
        </Entry>
        <Entry name="Leech">
          {card.leech ? (
            <span
              className="text-amber-700 dark:text-amber-300"
              title="Forgotten again and again: the question likely needs rewriting"
            >
              yes
            </span>
          ) : (
            'no'
          )}
        </Entry>
        <Entry name="Collection" mono wide>
          {card.collection}
        </Entry>
        <Entry name="Context" mono={card.see !== null} wide>
          {card.see ?? <None>none</None>}
        </Entry>
      </dl>
      <History card={card} read={readHistory} />
    </div>
  );
}

// A name and its value, WIDE for a value such as a collection, which gets a
// row of its own. A long value is cut short, whole in its tooltip.
function Entry({ name, mono, wide, children }: { name: string; mono?: boolean; wide?: boolean; children: ReactNode }) {
  return (
    <>
      <dt className="text-stone-400">{name}</dt>
      <dd
        className={`min-w-0 truncate ${mono ? 'font-mono' : ''} ${wide ? 'sm:col-span-3' : ''}`}
        title={typeof children === 'string' ? children : undefined}
      >
        {children}
      </dd>
    </>
  );
}

function None({ children }: { children: ReactNode }) {
  return <span className="text-stone-400">{children}</span>;
}

// The card's log events, newest first, read once READ is true. The events of
// another card aren't shown while this one's are read; this card's, read
// before its row was found again, stay until the new ones come.
function History({ card, read }: { card: FoundCard; read: boolean }) {
  const [history, setHistory] = useState<{ id: string; events: LogEvent[] } | { id: string; error: string } | null>(
    null,
  );
  useEffect(() => {
    if (!read) return;
    const abort = new AbortController();
    api.log(card.id, abort.signal).then(
      (events) => setHistory({ id: card.id, events }),
      (error: Error) => {
        if (!abort.signal.aborted) setHistory({ id: card.id, error: error.message });
      },
    );
    return () => abort.abort();
  }, [card, read]);

  const shown = history?.id === card.id ? history : null;
  return (
    <section>
      <Label>History</Label>
      {shown === null ? (
        <Loading label="Reading the history" className="flex flex-col gap-2">
          <Skeleton className="h-4 w-2/3" />
          <Skeleton className="h-4 w-1/2" />
        </Loading>
      ) : 'error' in shown ? (
        <p className="whitespace-pre-wrap text-rose-600 dark:text-rose-400">{shown.error}</p>
      ) : shown.events.length === 0 ? (
        <p className="text-stone-400">Nothing in the log: never reviewed.</p>
      ) : (
        <ol className="flex max-h-60 flex-col gap-1.5 overflow-y-auto">
          {[...shown.events].reverse().map((event, i) => (
            <li key={i} className="flex flex-wrap items-baseline gap-x-3">
              <time dateTime={event.t} className="font-mono text-xs text-stone-400 tabular-nums">
                {event.t.replace('T', ' ')}
              </time>
              <span className="min-w-0">
                <EventText event={event} />
              </span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function EventText({ event }: { event: LogEvent }) {
  switch (event.type) {
    case 'review': {
      const { name, color } = RATING_NAMES[event.rating];
      return (
        <>
          <b className={`font-medium ${color}`}>{name}</b>
          {event.by === 'agent' && ', recorded by an agent'}
          {event.response !== null && <span className="whitespace-pre-wrap">: “{event.response}”</span>}{' '}
          <span className="font-mono text-xs text-stone-400" title="The hash of the question when it was reviewed">
            {event.hash}
          </span>
        </>
      );
    }
    case 'reset':
      return <>Reset: new again</>;
    case 'suspend':
      return event.reason === null ? (
        <>Suspended</>
      ) : (
        <span className="whitespace-pre-wrap">Suspended: {event.reason}</span>
      );
    case 'unsuspend':
      return <>Unsuspended</>;
    case 'move':
      return (
        <>
          History moved from <code className="font-mono">{event.from}</code>
        </>
      );
  }
}

// The shape of CardDetails while the card is found: its entries, in a grid.
export function CardDetailsSkeleton() {
  return (
    <Loading label="Finding the card" className="grid grid-cols-2 gap-x-4 gap-y-2.5 sm:grid-cols-4">
      {Array.from({ length: 8 }, (_, i) => (
        <Skeleton key={i} className={`h-4 ${i % 2 === 0 ? 'w-16' : 'w-24'}`} />
      ))}
    </Loading>
  );
}

// The details of card ID in a modal, as srl find gives them each time it is
// opened.
export function CardDetailsDialog({ id, open, onClose }: { id: string; open: boolean; onClose: () => void }) {
  const [found, setFound] = useState<{ id: string; card: FoundCard } | { id: string; error: string } | null>(null);
  useEffect(() => {
    if (!open) return;
    let current = true;
    api.find(`id:${id}`).then(
      (cards) => {
        const card = cards.find((c) => c.id === id);
        if (current) setFound(card ? { id, card } : { id, error: `srl find doesn't list the card ${id}.` });
      },
      (error: Error) => current && setFound({ id, error: error.message }),
    );
    return () => {
      current = false;
    };
  }, [open, id]);

  const shown = found?.id === id ? found : null;
  return (
    <Modal open={open} title="Card details" onClose={onClose}>
      {shown === null ? (
        <CardDetailsSkeleton />
      ) : 'error' in shown ? (
        <p className="text-sm whitespace-pre-wrap text-rose-600 dark:text-rose-400">{shown.error}</p>
      ) : (
        <CardDetails card={shown.card} />
      )}
    </Modal>
  );
}
