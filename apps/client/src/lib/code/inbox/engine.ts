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

/** The snooze presets (02 section 5.5): a label, and the time it wakes, from now. */
export interface SnoozePreset {
  readonly id: string;
  readonly label: string;
  readonly until: (now: Date) => Date;
}

function nextMorning(now: Date, days: number): Date {
  const at = new Date(now);
  at.setDate(at.getDate() + days);
  at.setHours(9, 0, 0, 0);
  return at;
}

export const SNOOZE_PRESETS: readonly SnoozePreset[] = [
  { id: 'snooze-1h', label: 'In 1 hour', until: (now) => new Date(now.getTime() + HOUR) },
  { id: 'snooze-3h', label: 'In 3 hours', until: (now) => new Date(now.getTime() + 3 * HOUR) },
  { id: 'snooze-tomorrow', label: 'Tomorrow', until: (now) => nextMorning(now, 1) },
  { id: 'snooze-week', label: 'Next week', until: (now) => nextMorning(now, 7) },
];
