import { useQuery } from '@tanstack/react-query';

import { Activity } from './Activity';
import { type Schedule } from './api';
import { useView } from './App';
import { Label, Loading, Skeleton } from './CardParts';
import { FilterButton } from './Filters';
import { queries } from './queries';
import { TopBar, VIEW_COLUMN, VIEW_SCROLL } from './TopBar';

// What srl says about studying, shown and never changed: the scheduling
// settings in use, of the whole store, and the calendar of the search's
// cards.
export function Stats() {
  const { search, submitted, onSearch } = useView();
  return (
    <div className="flex h-dvh flex-col">
      <TopBar view="stats">
        {/* The search, for the calendar, with its text in the filters. */}
        <FilterButton search={search} onSearch={onSearch} className="max-md:ml-auto" />
      </TopBar>
      <main className={VIEW_SCROLL}>
        <div className={`${VIEW_COLUMN} gap-10`}>
          <section>
            <Label>Scheduling</Label>
            <ScheduleTable />
          </section>
          <section>
            <Label>Activity</Label>
            <Activity selector={submitted.selector} />
          </section>
        </div>
      </main>
    </div>
  );
}

// Each setting of [schedule], in the order of docs/store.md: its key in
// config.toml, what it is, and its value as shown.
const SETTINGS: { key: keyof Schedule; name: string; value: (schedule: Schedule) => string }[] = [
  {
    key: 'desired_retention',
    name: 'The chance of recalling a card when it is due, which FSRS schedules for',
    value: (s) => `${Number((s.desired_retention * 100).toFixed(2))}%`,
  },
  {
    key: 'day_starts_at',
    name: 'The hour a new study day starts, local time',
    value: (s) => `${s.day_starts_at}:00`,
  },
  { key: 'new_per_day', name: 'New cards a day, across every search', value: (s) => String(s.new_per_day) },
  { key: 'max_skip', name: 'Cards skipped at random, at most, to pick the next', value: (s) => String(s.max_skip) },
  { key: 'leech_lapses', name: 'Lapses that make a card a leech', value: (s) => String(s.leech_lapses) },
  {
    key: 'mature_days',
    name: 'Days of stability that make a card mature',
    value: (s) => `${s.mature_days} days`,
  },
  { key: 'weights', name: 'FSRS weights, the 21 in use', value: (s) => s.weights.join(', ') },
];

// The settings srl schedules with, those config.toml leaves out at their
// defaults: srl config gives every one, so the page knows no default.
function ScheduleTable() {
  const schedule = useQuery(queries.schedule());
  if (schedule.isError && !schedule.isFetching) {
    return <p className="text-sm whitespace-pre-wrap text-rose-600 dark:text-rose-400">{schedule.error.message}</p>;
  }
  return (
    <>
      <p className="mb-3 text-sm text-stone-500">
        <code className="font-mono">[schedule]</code> of <code className="font-mono">.srl/config.toml</code>, with
        the default of each it leaves out.
      </p>
      {schedule.isSuccess ? (
        <table className="w-full text-sm">
          <tbody>
            {SETTINGS.map(({ key, name, value }) => (
              <tr key={key} className="border-t border-stone-200 align-baseline dark:border-stone-800">
                <th scope="row" colSpan={key === 'weights' ? 2 : 1} className="py-2 pr-4 text-left font-normal">
                  <div>{name}</div>
                  <code className="font-mono text-xs text-stone-400">{key}</code>
                  {/* The weights, a long list, go under their name, the
                      whole width of the table. */}
                  {key === 'weights' && (
                    <div className="mt-1 font-mono text-xs break-words text-stone-500 tabular-nums">
                      {value(schedule.data)}
                    </div>
                  )}
                </th>
                {key !== 'weights' && (
                  <td className="py-2 text-right whitespace-nowrap tabular-nums">{value(schedule.data)}</td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <ScheduleSkeleton />
      )}
    </>
  );
}

// The table's shape while srl reads the configuration: a row per setting,
// its name and key at the left, its value at the right, or under them for
// the weights.
function ScheduleSkeleton() {
  return (
    <Loading label="Reading the configuration" className="flex flex-col">
      {SETTINGS.map(({ key }) => (
        <div key={key} className="flex justify-between gap-4 border-t border-stone-200 py-2 dark:border-stone-800">
          <div className="flex min-w-0 flex-1 flex-col">
            <div className="flex h-5 items-center">
              <Skeleton className="h-4 w-64 max-w-full" />
            </div>
            <div className="flex h-4 items-center">
              <Skeleton className="h-3 w-24" />
            </div>
            {key === 'weights' && <Skeleton className="mt-1 h-3 w-full" />}
          </div>
          {key !== 'weights' && (
            <div className="flex h-5 items-center">
              <Skeleton className="h-4 w-12" />
            </div>
          )}
        </div>
      ))}
    </Loading>
  );
}
