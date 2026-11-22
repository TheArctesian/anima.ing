/**
 * The one door from the inbox to settle, snooze and wake (TH61). The runner keeps the marks on each listing; the inbox
 * reads them from `live.listings` and changes them by `changeListing`, as pin and rename. Where the runner of this
 * computer is not there, the actions show disabled with `why`.
 */
import type { CallResult, LiveThreads } from '$lib/agent-threads/live/live.svelte';
import type { Marks } from '$lib/agent-threads/listing/shelf';

/** A change of the marks of one thread. */
export type InboxChange =
  | { readonly change: 'settle'; readonly on: boolean }
  | { readonly change: 'snooze'; readonly until: Date }
  | { readonly change: 'wake' };

export interface InboxEngine {
  /** Whether the runner answers, so the marks can change. */
  readonly ready: boolean;
  /** Why the actions are disabled, in words for the person; empty when ready. */
  readonly why: string;
  /** The marks of each thread, by its id. */
  readonly marks: ReadonlyMap<string, Marks>;
  change(thread: string, change: InboxChange): Promise<CallResult>;
}

/** Shown beside a disabled Settle, Snooze or Wake. */
export const WAITING_WHY = 'The thread runner of this computer keeps settle and snooze; it is not running.';

/** The engine with no runner: no marks, and the actions disabled. */
export const WAITING: InboxEngine = {
  ready: false,
  why: WAITING_WHY,
  marks: new Map(),
  change: () => Promise.resolve({ ok: false, words: WAITING_WHY }),
};

/** The engine of the runner of this computer, read through `live` each time (the store links the runner later). */
export function inboxEngine(live: () => LiveThreads | null): InboxEngine {
  return {
    get ready() {
      const runner = live();
      return runner !== null && runner.linked !== false;
    },
    get why() {
      return this.ready ? '' : WAITING_WHY;
    },
    get marks() {
      const runner = live();
      return runner === null ? WAITING.marks : new Map([...runner.listings].map(([id, listing]) => [id, listing.marks]));
    },
    change(thread, change) {
      const runner = live();
      if (runner === null) return WAITING.change(thread, change);
      return runner.changeListing(thread, change.change === 'snooze' ? { change: 'snooze', until: change.until.toISOString() } : change);
    },
  };
}

const HOUR = 60 * 60_000;

/** A snooze preset, as T3 Code's defaults resolve it from now (08 inbox 6). */
export interface SnoozePreset {
  readonly id: string;
  readonly label: string;
  readonly until: Date;
  /** Its label names the day ("This evening", "Tomorrow"), so its detail is the time alone. */
  readonly namesDay: boolean;
}

function at(now: Date, days: number, hour: number): Date {
  const when = new Date(now);
  when.setDate(when.getDate() + days);
  when.setHours(hour, 0, 0, 0);
  return when;
}

/**
 * T3 Code's five defaults: in 1 hour, in 3 hours, this evening at 18:00 (left out once past), tomorrow at 09:00 and
 * next Monday at 09:00; soonest first, one row for each wake minute. A Monday that is tomorrow reads "Tomorrow".
 */
export function snoozePresets(now: Date): SnoozePreset[] {
  const monday = (1 - now.getDay() + 7) % 7 || 7;
  const all: SnoozePreset[] = [
    { id: 'snooze-1h', label: 'In 1 hour', until: new Date(now.getTime() + HOUR), namesDay: false },
    { id: 'snooze-3h', label: 'In 3 hours', until: new Date(now.getTime() + 3 * HOUR), namesDay: false },
    { id: 'snooze-evening', label: 'This evening', until: at(now, 0, 18), namesDay: true },
    { id: 'snooze-tomorrow', label: 'Tomorrow', until: at(now, 1, 9), namesDay: true },
    { id: 'snooze-week', label: monday === 1 ? 'Tomorrow' : 'Next week', until: at(now, monday, 9), namesDay: monday === 1 },
  ];
  const minute = (preset: SnoozePreset): number => Math.floor(preset.until.getTime() / 60_000);
  const seen = new Set<number>();
  return all
    .filter((preset) => preset.until.getTime() > now.getTime())
    .sort((a, b) => minute(a) - minute(b))
    .filter((preset) => {
      if (seen.has(minute(preset))) return false;
      seen.add(minute(preset));
      return true;
    });
}

/** What the Custom snooze dialog holds: a date and a time, or a number of minutes, hours or days from now (T3 Code). */
export type CustomSnooze =
  | { readonly mode: 'date'; readonly date: string; readonly time: string }
  | { readonly mode: 'duration'; readonly amount: string; readonly unit: 'minutes' | 'hours' | 'days' };

const UNIT_MS = { minutes: 60_000, hours: 3_600_000, days: 86_400_000 } as const;

/** The local `YYYY-MM-DD` and `HH:MM` of `at`, as the date and time fields take them. */
export function localFields(at: Date): { date: string; time: string } {
  const two = (value: number): string => String(value).padStart(2, '0');
  return { date: `${String(at.getFullYear())}-${two(at.getMonth() + 1)}-${two(at.getDate())}`, time: `${two(at.getHours())}:${two(at.getMinutes())}` };
}

/** The time a custom snooze wakes, or `null` when it is not a valid time after `now` (T3's resolveCustomSnooze). */
export function customWake(input: CustomSnooze, now: Date): Date | null {
  if (input.mode === 'duration') {
    const amount = Number(input.amount);
    if (!Number.isFinite(amount) || amount <= 0) return null;
    return new Date(now.getTime() + amount * UNIT_MS[input.unit]);
  }
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(input.date);
  const clock = /^(\d{2}):(\d{2})$/.exec(input.time);
  if (match === null || clock === null) return null;
  const wake = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]), Number(clock[1]), Number(clock[2]));
  return Number.isNaN(wake.getTime()) || wake.getTime() <= now.getTime() ? null : wake;
}
