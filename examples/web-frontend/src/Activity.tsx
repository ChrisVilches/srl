import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import { type DayCounts } from './api';
import { Loading, Skeleton } from './CardParts';
import { queries } from './queries';

// The study of the last DAYS study days, as a calendar like GitHub's: a
// column per week, Sunday at the top, a square per day, darker for more
// reviews. srl counts them (srl stats --days, docs/cli.md, "Analysis"),
// so a day is a study day, which starts at srl's day_starts_at, and srl
// alone says which day is today: the page lays out the days it is given.
const DAYS = 63;

// The shades of a day by how many reviews it had, from none to the most of
// the days shown: one hue, light to dark, and darker to lighter in the dark
// theme.
const SHADES = [
  'bg-stone-200 dark:bg-stone-800',
  'bg-emerald-200 dark:bg-emerald-900',
  'bg-emerald-400 dark:bg-emerald-700',
  'bg-emerald-600 dark:bg-emerald-500',
  'bg-emerald-800 dark:bg-emerald-300',
];

// The shade of REVIEWS out of MOST: none for none, else one of four by
// its share of the most.
const shadeOf = (reviews: number, most: number) =>
  SHADES[reviews === 0 ? 0 : Math.ceil(((SHADES.length - 1) * reviews) / most)];

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

// The weekday of DAY, "2026-10-27", 0 for Sunday: of the date itself,
// whatever the browser's time zone.
const weekdayOf = (day: string) => new Date(`${day}T00:00:00Z`).getUTCDay();

const monthOf = (day: string) =>
  new Date(`${day}T00:00:00Z`).toLocaleString(undefined, { month: 'short', timeZone: 'UTC' });

// The calendar of the cards of a search's SELECTORS, read again after each
// change to the store (src/main.tsx). Its skeleton has the calendar's
// shape, so what is under it doesn't move as the calendar replaces it.
export function Activity({ selectors }: { selectors: string[] }) {
  const days = useQuery(queries.days(selectors, DAYS));
  const searched = selectors[0] !== '';
  return (
    <div className="flex flex-col gap-4 text-sm">
      <p className="text-stone-500">
        Reviews of each of the last {DAYS / 7} weeks, {searched ? 'of the cards of this search' : 'of every card'}.
      </p>
      {days.isSuccess ? (
        days.data === null ? (
          <p className="text-stone-400">{searched ? 'No card matches this search.' : 'The store has no card.'}</p>
        ) : (
          <Calendar days={days.data} />
        )
      ) : days.isError && !days.isFetching ? (
        <p className="whitespace-pre-wrap text-rose-600 dark:text-rose-400">{days.error.message}</p>
      ) : (
        <CalendarSkeleton />
      )}
    </div>
  );
}

// The grid of the calendar: the weekdays' names, then a column per week,
// as wide as the room allows up to WEEK, with its month's name above the
// first week of a month. There are always WEEKS columns, the most DAYS can
// take, starting on any weekday, so that the calendar and its skeleton are
// the same size wherever the days start.
const GRID = 'grid grid-flow-col justify-start grid-rows-[auto_repeat(7,minmax(0,1fr))] gap-1';
const WEEK = 'minmax(0, 1.75rem)';
const WEEKS = Math.ceil((DAYS + 6) / 7);
const COLUMNS = { gridTemplateColumns: `auto repeat(${WEEKS}, ${WEEK})` };

// A line of text-sm, which a skeleton's bar of text takes the height of.
const LINE = 'flex h-5 items-center';

// The day last pointed at, focused or tapped is told below the calendar,
// today until then, so that a phone, which shows no tooltips, tells it too.
function Calendar({ days }: { days: DayCounts[] }) {
  const today = days[days.length - 1];
  // Kept by its date, so that the counts read again after a review show.
  const [pointed, setPointed] = useState<string | null>(null);
  const shown = days.find((day) => day.day === pointed) ?? today;
  const most = Math.max(1, ...days.map((day) => day.reviews));
  // Blanks before the first day, so that each day is on its weekday's row,
  // and after the last, up to WEEKS weeks.
  const cells: (DayCounts | null)[] = [...Array<null>(weekdayOf(days[0].day)).fill(null), ...days];
  cells.push(...Array<null>(WEEKS * 7 - cells.length).fill(null));
  const weeks = Array.from({ length: WEEKS }, (_, i) => cells.slice(i * 7, i * 7 + 7));
  const total = days.reduce((sum, day) => sum + day.reviews, 0);
  const studied = days.filter((day) => day.reviews > 0).length;
  return (
    <>
      <div className={GRID} style={COLUMNS}>
        <span />
        {WEEKDAYS.map((name, i) => (
          <span key={name} className="self-center pr-1 text-xs leading-none text-stone-400">
            {i % 2 === 1 ? name : ''}
          </span>
        ))}
        {weeks.map((week, i) => {
          // The month's name over its first week, and over the first week shown.
          const first = week.find((day) => day !== null);
          const monthStart = week.find((day) => day?.day.endsWith('-01'));
          const month = monthStart ? monthOf(monthStart.day) : i === 0 && first ? monthOf(first.day) : '';
          return [
            <span key={`month-${i}`} className="h-4 text-xs whitespace-nowrap text-stone-400">
              {month}
            </span>,
            ...week.map((day, j) =>
              day === null ? (
                <span key={`blank-${i}-${j}`} />
              ) : (
                <button
                  key={day.day}
                  type="button"
                  title={describe(day)}
                  aria-label={describe(day)}
                  onPointerEnter={() => setPointed(day.day)}
                  onFocus={() => setPointed(day.day)}
                  onClick={() => setPointed(day.day)}
                  className={`aspect-square rounded-sm ${shadeOf(day.reviews, most)} ${day === shown ? 'ring-2 ring-stone-500 ring-offset-1 ring-offset-white dark:ring-stone-400 dark:ring-offset-stone-900' : ''}`}
                />
              ),
            ),
          ];
        })}
      </div>
      <p className="text-stone-500 tabular-nums">
        {shown === today ? 'Today, ' : ''}
        {describe(shown)}
      </p>
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 text-stone-500">
        <span className="tabular-nums">
          <b className="text-stone-800 dark:text-stone-200">{total}</b> reviews ·{' '}
          <b className="text-stone-800 dark:text-stone-200">{studied}</b> of {days.length} days studied
        </span>
        <span className="flex items-center gap-1 text-xs text-stone-400" aria-hidden>
          Less
          {SHADES.map((shade) => (
            <span key={shade} className={`size-3 rounded-sm ${shade}`} />
          ))}
          More
        </span>
      </div>
    </>
  );
}

// What a day's square says.
const describe = ({ day, reviews, new: fresh, again }: DayCounts) =>
  reviews === 0 ? `${day}: no reviews` : `${day}: ${reviews} reviews, ${fresh} new, ${again} again`;

// The calendar's shape while srl counts, the same size: its grid, the
// day's line, and the total with the legend, which wraps as they do.
function CalendarSkeleton() {
  return (
    <Loading label="Counting the reviews" className="flex flex-col gap-4">
      <div className={GRID} style={COLUMNS}>
        <span />
        {WEEKDAYS.map((name, i) => (
          <span key={name} className="self-center pr-1 text-xs leading-none text-transparent">
            {i % 2 === 1 ? name : ''}
          </span>
        ))}
        {Array.from({ length: WEEKS }, (_, i) => [
          <span key={`month-${i}`} className="h-4" />,
          ...Array.from({ length: 7 }, (_, j) => <Skeleton key={j} className="aspect-square rounded-sm" />),
        ])}
      </div>
      <div className={LINE}>
        <Skeleton className="h-4 w-52 max-w-full" />
      </div>
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <div className={LINE}>
          <Skeleton className="h-4 w-56" />
        </div>
        <div className="flex h-4 items-center">
          <Skeleton className="h-3 w-36" />
        </div>
      </div>
    </Loading>
  );
}
