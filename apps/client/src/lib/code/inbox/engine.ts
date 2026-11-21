/**
 * The one door from the inbox to settle, snooze and wake. The runner does not keep them yet: they are task TH61 of
 * docs/proposals/agent-threads.md, not built, and not on the branch threads/engine-for-copy (TH101 to TH107). Until
 * the runner keeps them, `inboxEngine` gives `WAITING`: the actions show, disabled, with `why` as their reason, and the
 * inbox puts the threads that "Earlier" held (OD-THR24) in Settled (OD-THR55). When TH61 lands, only this file changes.
 */
import type { CallResult, LiveThreads } from '$lib/agent-threads/live/live.svelte';

/** What the runner is to keep of one thread for the inbox; the shape proposed for TH61. */
export interface InboxMark {
  /** When the person settled the thread; `null` when it is not settled. */
  readonly settledAt: Date | null;
  /** Until when the thread sleeps; `null` when it is not snoozed. */
  readonly snoozedUntil: Date | null;
  /** When a snooze ended, until the person opens or dismisses the thread: the status "Woke". */
  readonly wokeAt: Date | null;
}

/** A change of the mark of one thread, in the form proposed for the socket of the runner. */
export type InboxChange =
  | { readonly change: 'settle'; readonly on: boolean }
  | { readonly change: 'snooze'; readonly until: Date }
  | { readonly change: 'wake' };

export interface InboxEngine {
  /** Whether the runner keeps the marks: false until TH61. */
  readonly ready: boolean;
  /** Why the actions are disabled, in words for the person; empty when ready. */
  readonly why: string;
  readonly marks: ReadonlyMap<string, InboxMark>;
  change(thread: string, change: InboxChange): Promise<CallResult>;
}

/** Shown beside a disabled Settle, Snooze or Wake. */
export const WAITING_WHY = 'Not yet: the thread runner does not keep settle or snooze (task TH61).';

/** The engine while the runner has no settle, snooze or wake. */
export const WAITING: InboxEngine = {
  ready: false,
  why: WAITING_WHY,
  marks: new Map(),
  change: () => Promise.resolve({ ok: false, words: WAITING_WHY }),
};

/** The engine of the runner of this computer. `live` is unused until TH61 gives it the calls. */
export function inboxEngine(live: LiveThreads | null): InboxEngine {
  void live;
  return WAITING;
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
