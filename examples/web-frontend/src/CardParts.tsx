import { useEffect, useRef, useState, type ReactNode } from 'react';

import type { FoundCard, ShownCard } from './api';
import { Markdown } from './Markdown';

// Every button changes color on hover only while it is enabled (`enabled:hover:`),
// so one disabled while srl works, or until its form is complete, doesn't look
// clickable.

// The look of a view's main action: a raised surface in the theme's colors,
// like the card.
export const MAIN_BUTTON =
  'rounded-lg border border-stone-300 bg-white font-medium text-stone-900 shadow-sm enabled:hover:bg-stone-100 disabled:opacity-50 dark:border-stone-700 dark:bg-stone-800 dark:text-stone-100 dark:enabled:hover:bg-stone-700';
// The look of a button that confirms losing something, takes a card out
// of study or clears the filters: a red border and name, and a faint red on hover.
// It has no Key: what it does takes a click.
export const DANGER_BUTTON =
  'rounded-lg border border-rose-400 font-medium text-rose-700 enabled:hover:bg-rose-50 enabled:active:bg-rose-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rose-500 disabled:opacity-50 dark:border-rose-500/70 dark:text-rose-400 dark:enabled:hover:bg-rose-950/40 dark:enabled:active:bg-rose-950/70';
// The looks of the buttons that aren't a view's main action.
export const OUTLINE_BUTTON =
  'rounded-lg border border-stone-300 px-4 py-2 text-sm enabled:hover:bg-stone-100 dark:border-stone-700 disabled:opacity-50 dark:enabled:hover:bg-stone-900';
export const QUIET_BUTTON = 'rounded-lg px-3 text-stone-500 enabled:hover:bg-stone-200 disabled:opacity-50 dark:enabled:hover:bg-stone-800';
// The layout of a button with a Key after its name, which centers the key
// on the name's line: inline, the smaller key sits on the baseline, low.
export const WITH_KEY = 'inline-flex items-center justify-center gap-1';

// A card as studying shows it: its kind, and the prompt, or, once revealed,
// the revealed prompt, the answer and the context. BADGES go after the kind,
// and START before it, at the left. Its ID and the rest are in CardDetails.
// With SCROLL_TO_ANSWER, as when studying, revealing the card scrolls
// smoothly to put the answer in the middle of the view under the top bar
// (its <main>, src/TopBar.tsx), or its top at the view's top when it is
// taller than the view, so that its start shows.
export function CardFace({
  card,
  revealed,
  badges,
  start,
  scrollToAnswer = false,
}: {
  card: ShownCard;
  revealed: boolean;
  badges?: ReactNode;
  start?: ReactNode;
  scrollToAnswer?: boolean;
}) {
  const answer = useRef<HTMLElement>(null);
  useEffect(() => {
    const shown = answer.current;
    if (!scrollToAnswer || !revealed || !shown) return;
    const view = shown.closest('main') ?? document.documentElement;
    const fits = shown.offsetHeight <= view.clientHeight;
    shown.scrollIntoView({ behavior: 'smooth', block: fits ? 'center' : 'start' });
  }, [scrollToAnswer, revealed, card]);

  return (
    <>
      <article className="rounded-2xl border border-stone-200 bg-white shadow-sm dark:border-stone-800 dark:bg-stone-900">
        <div className="flex items-center justify-between gap-3 px-6 pt-4 text-xs text-stone-400">
          <span className="min-w-0">{start}</span>
          <span className="flex shrink-0 flex-wrap items-center justify-end gap-2">
            <KindBadge kind={card.kind} />
            {badges}
          </span>
        </div>
        <section className="px-6 pt-2 pb-6">
          <Label>Question</Label>
          <Markdown text={revealed ? (card.prompt_revealed ?? card.prompt) : card.prompt} className="prose-lg" />
        </section>
        {revealed && card.answer !== null && (
          <section ref={answer} className="border-t border-dashed border-stone-200 px-6 py-6 dark:border-stone-700">
            <Label>Answer</Label>
            <Markdown text={card.answer} className="prose-lg" />
          </section>
        )}
      </article>
      {revealed && <ContextView card={card} />}
    </>
  );
}

const PILL = 'rounded-full px-2 py-0.5';

// The kinds built into srl (docs/cards.md, "Kinds"). srl doesn't say which
// kinds are built in, so a new one must be added here.
const BUILT_IN_KINDS = ['qa'];

// The kind that made a card, in violet, or nothing for a built-in kind: only
// a kind program is worth pointing out. A long name is cut short, whole in
// the tooltip.
export function KindBadge({ kind }: { kind: string }) {
  if (BUILT_IN_KINDS.includes(kind)) return null;
  return (
    <span
      title={`The kind that made this card: ${kind}`}
      className={`${PILL} max-w-48 truncate bg-violet-100 font-mono font-medium text-violet-700 dark:bg-violet-950 dark:text-violet-300`}
    >
      {kind}
    </span>
  );
}

// The card's status when it isn't active (srl next gives only active
// cards), where it is in its schedule, in sky blue when new, and, in amber,
// whether it is a leech.
export function CardBadges({
  status,
  schedule,
  leech,
}: {
  status?: FoundCard['status'];
  schedule: FoundCard['schedule'];
  leech: boolean;
}) {
  return (
    <>
      {status !== undefined && status !== 'active' && (
        <span className={`${PILL} bg-stone-200 font-medium text-stone-600 dark:bg-stone-800 dark:text-stone-300`}>
          {status}
        </span>
      )}
      <span
        className={`${PILL} ${schedule === 'new' ? 'bg-sky-100 font-medium text-sky-700 dark:bg-sky-950 dark:text-sky-300' : ''}`}
      >
        {schedule}
      </span>
      {leech && (
        <span
          className={`${PILL} bg-amber-100 font-medium text-amber-800 dark:bg-amber-400/10 dark:text-amber-300`}
          title="Forgotten again and again: the question likely needs rewriting"
        >
          leech
        </span>
      )}
    </>
  );
}

// The item's context: the note its see: names, under its path. Nothing
// when the item has none; only the path when srl gives no text for the
// note, such as a PDF or an image.
function ContextView({ card }: { card: ShownCard }) {
  if (card.see === null) {
    return null;
  }
  return (
    <section className="rounded-2xl bg-stone-100 px-6 py-5 dark:bg-stone-900/50">
      <div className="truncate font-mono text-xs text-stone-500" title={card.see}>
        {card.see}
      </div>
      {card.context === null ? (
        <p className="mt-3 text-sm text-stone-500">No text to show: open the note to see it.</p>
      ) : (
        <Markdown text={contextMarkdown(card.context, card.see)} className="mt-3 prose-sm prose-stone" />
      )}
    </section>
  );
}

const MARKDOWN_EXTENSIONS = ['.md', '.markdown', '.mdx', '.qmd'];

// The context as Markdown to render, by the extension of the note's path
// (see). A note in anything but Markdown, such as a text file, goes in a
// code block as plain text, json for JSON and JSONL, xml for XML, with a
// fence longer than any run of backticks in it.
function contextMarkdown(context: string, path: string | null): string {
  const lower = (path ?? '').toLowerCase();
  if (MARKDOWN_EXTENSIONS.some((extension) => lower.endsWith(extension))) {
    return context;
  }
  const language = /\.jsonl?$/.test(lower) ? 'json' : lower.endsWith('.xml') ? 'xml' : '';
  const longest = Math.max(0, ...(context.match(/`+/g) ?? []).map((run) => run.length));
  const fence = '`'.repeat(Math.max(3, longest + 1));
  return `${fence}${language}\n${context.replace(/\n+$/, '')}\n${fence}\n`;
}

export function ErrorView({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="m-auto flex w-full flex-col gap-4">
      <h1 className="font-semibold text-rose-600 dark:text-rose-400">srl reported an error</h1>
      <pre className="overflow-x-auto rounded-lg bg-rose-50 p-4 text-sm whitespace-pre-wrap text-rose-950 dark:bg-rose-950/40 dark:text-rose-200">
        {message}
      </pre>
      <button onClick={onRetry} className={`self-start ${OUTLINE_BUTTON}`}>
        Try again
      </button>
    </div>
  );
}

// Numbers for the top bar, each with its name and, optionally, its color.
export function Counts({ counts }: { counts: [number, string, string?][] }) {
  return (
    <span className="text-sm text-stone-500 tabular-nums dark:text-stone-400">
      {counts.map(([n, name, color], i) => (
        <span key={name}>
          {i > 0 && ' · '}
          <b className={color ?? 'text-stone-800 dark:text-stone-200'}>{n}</b> {name}
        </span>
      ))}
    </span>
  );
}

// Whether an item is a draft, in every form that writes one.
export function DraftCheckbox({ checked, onChange }: { checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      Draft
    </label>
  );
}

export function Label({ children }: { children: ReactNode }) {
  return <div className="mb-2 text-xs font-semibold tracking-widest text-stone-400 uppercase">{children}</div>;
}

// The key that does what a button does. Hidden on a touch screen (a coarse
// primary pointer), which has no keys to press.
export function Key({ children }: { children: ReactNode }) {
  return (
    <kbd className="ml-1 inline-block rounded pointer-coarse:hidden bg-black/15 px-1.5 py-1 align-middle font-mono text-xs leading-none opacity-80 dark:bg-white/15">
      {children}
    </kbd>
  );
}

// How long CardSkeleton shows before it adds the seconds srl has taken:
// most cards, and finding none is left, come sooner, so the skeleton alone,
// in faint colors, comes and goes without a flash. A card slower than this,
// as one a kind program asks an LLM for, gets the seconds, which say it is
// still coming.
const SECONDS_SHOWN_AFTER_MS = 3000;

// A card while srl makes it, or, in Study, finds none is left: CardFace's
// shape in Skeleton bars, then StillLoading, then CHILDREN, the shape of
// what goes under the card. OF is what is loading: when it changes, only
// StillLoading is new, so its seconds start again while the bars go on
// pulsing, as a new skeleton's would not.
export function CardSkeleton({ of, children }: { of: string | number; children?: ReactNode }) {
  return (
    <Loading label="Loading the card" className="flex flex-1 flex-col gap-6">
      <div className="rounded-2xl border border-stone-200 bg-white px-6 pt-4 pb-6 shadow-sm dark:border-stone-800 dark:bg-stone-900">
        <Skeleton className="h-4 w-16" />
        <Skeleton className="mt-4 mb-3 h-3 w-16" />
        <Skeleton className="h-6 w-full" />
        <Skeleton className="mt-2 h-6 w-2/3" />
      </div>
      <StillLoading key={of} />
      {children}
    </Loading>
  );
}

// The seconds since this was first shown, once there are
// SECONDS_SHOWN_AFTER_MS of them.
function StillLoading() {
  const [since] = useState(() => performance.now());
  const [now, setNow] = useState(since);
  useEffect(() => {
    const timer = setInterval(() => setNow(performance.now()), 200);
    return () => clearInterval(timer);
  }, []);
  const elapsed = now - since;
  if (elapsed < SECONDS_SHOWN_AFTER_MS) return null;
  return (
    <p className="text-center text-sm text-stone-500">
      Still loading… <span className="tabular-nums">{(elapsed / 1000).toFixed(1)} s</span>
    </p>
  );
}

// The shape of what is loading, in place of a text that says so: a view
// shows one built from Skeleton bars, laid out like what will replace it,
// so the page doesn't jump when it comes. LABEL says what is loading to a
// screen reader. A card has CardSkeleton, which also shows the seconds
// once the card is slow to come.
export function Loading({ label, className, children }: { label: string; className?: string; children: ReactNode }) {
  return (
    <div role="status" aria-busy className={className}>
      <span className="sr-only">{label}</span>
      {children}
    </div>
  );
}

// A bar of a skeleton, sized by CLASSNAME. It pulses, but not with reduced
// motion.
export function Skeleton({ className }: { className: string }) {
  return (
    <div aria-hidden className={`rounded-md bg-stone-200 motion-safe:animate-pulse dark:bg-stone-800 ${className}`} />
  );
}

export function Notice({ children }: { children: ReactNode }) {
  return <p className="m-auto text-stone-400">{children}</p>;
}
