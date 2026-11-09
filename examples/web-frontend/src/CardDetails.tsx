import { useQuery } from '@tanstack/react-query';
import { Fragment, type ReactNode } from 'react';

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

// The card's log events, newest first, read once READ is true, and again
// after each change to the store (src/main.tsx). The events of another
// card aren't shown while this one's are read; this card's, read before,
// stay until the new ones come.
function History({ card, read }: { card: FoundCard; read: boolean }) {
  const log = useQuery({
    queryKey: ['log', card.id],
    queryFn: ({ signal }) => api.log(card.id, signal),
    enabled: read,
  });
  const shown: { events: LogEvent[] } | { error: string } | null = log.isSuccess
    ? { events: log.data }
    : log.isError && !log.isFetching
      ? { error: log.error.message }
      : null;
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

// The widths of the values of CardDetails' entries, in their order, and
// whether each is wide.
const SKELETON_VALUES: { width: string; wide?: boolean }[] = [
  { width: 'w-14' },
  { width: 'w-14' },
  { width: 'w-10' },
  { width: 'w-20' },
  { width: 'w-14' },
  { width: 'w-12' },
  { width: 'w-24' },
  { width: 'w-8' },
  { width: 'w-72', wide: true },
  { width: 'w-60', wide: true },
];

// The shape of CardDetails while the card is found: its entries, in the
// same grid, each a line of text tall. A suspended card's reason, above
// them, isn't shown: most cards have none.
export function CardDetailsSkeleton() {
  return (
    <Loading
      label="Finding the card"
      className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 sm:grid-cols-[auto_minmax(0,1fr)_auto_minmax(0,1fr)]"
    >
      {SKELETON_VALUES.map(({ width, wide }, i) => (
        <Fragment key={i}>
          <Skeleton className="my-0.5 h-4 w-16" />
          <Skeleton className={`my-0.5 h-4 max-w-full ${width} ${wide ? 'sm:col-span-3' : ''}`} />
        </Fragment>
      ))}
    </Loading>
  );
}

// The details of card ID in a modal, as srl find gives them, read when it is
// opened and kept as other reads are (src/main.tsx). The modal is tall, so
// it doesn't move as the card, then its history, come in.
export function CardDetailsDialog({ id, open, onClose }: { id: string; open: boolean; onClose: () => void }) {
  const found = useQuery({
    queryKey: ['card', id],
    queryFn: async () => {
      const card = (await api.find(`id:${id}`)).find((c) => c.id === id);
      if (!card) throw new Error(`srl find doesn't list the card ${id}.`);
      return card;
    },
    enabled: open,
  });
  const shown: { card: FoundCard } | { error: string } | null = found.isSuccess
    ? { card: found.data }
    : found.isError && !found.isFetching
      ? { error: found.error.message }
      : null;
  return (
    <Modal open={open} title="Card details" tall onClose={onClose}>
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
