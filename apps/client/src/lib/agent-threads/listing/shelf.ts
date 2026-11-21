/**
 * Where the list shows a thread (TH61): with the active threads, on the shelf Snoozed, or on the shelf Settled. The
 * runner keeps the marks of the person; this is the same rule as `Listing::shelf` of the runner
 * (`crates/agent-threads/src/control/shelf.rs`), with the same cases in its test. Pure.
 */

const HOUR_MS = 60 * 60 * 1000;

/** A thread with no work and no prompt for this time settles by itself (T3 Code's default). */
export const SETTLE_AFTER_MS = 7 * 24 * HOUR_MS;

/** The longest snooze the runner keeps. */
export const MOST_SNOOZE_MS = 30 * 24 * HOUR_MS;

/** When the person last settled, took back, snoozed or woke a thread; `null` until then. */
export interface Marks {
  readonly settledAt: Date | null;
  readonly unsettledAt: Date | null;
  readonly snoozedAt: Date | null;
  readonly snoozedUntil: Date | null;
  readonly wokeAt: Date | null;
}

export const NO_MARKS: Marks = { settledAt: null, unsettledAt: null, snoozedAt: null, snoozedUntil: null, wokeAt: null };

/** What the list knows of the work of a thread. */
export interface Activity {
  /** The newest prompt of the person, or the start of the thread. */
  readonly prompted: Date;
  /** The newest moment of work: a prompt, a reply or the end of a turn. */
  readonly last: Date;
  readonly working: boolean;
  /** The newest request that waits for the person, or `null`. */
  readonly asked: Date | null;
}

export type Shelf =
  | { readonly shelf: 'active' }
  | { readonly shelf: 'snoozed'; readonly until: Date }
  /** `auto` when no person settled it: it was quiet for `SETTLE_AFTER_MS` from `since`. */
  | { readonly shelf: 'settled'; readonly since: Date; readonly auto: boolean };

function dateOf(value: unknown): Date | null {
  if (typeof value !== 'string') return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

/** The marks of one listing as the runner writes them (`settled_at` and the rest; an absent mark is `null`). */
export function marksFrom(fields: Readonly<Record<string, unknown>>): Marks {
  return {
    settledAt: dateOf(fields.settled_at),
    unsettledAt: dateOf(fields.unsettled_at),
    snoozedAt: dateOf(fields.snoozed_at),
    snoozedUntil: dateOf(fields.snoozed_until),
    wokeAt: dateOf(fields.woke_at),
  };
}

function snoozedUntil(marks: Marks, activity: Activity, now: Date): Date | null {
  const { snoozedUntil: until, snoozedAt: from } = marks;
  if (until === null || from === null || until <= now) return null;
  const woken = activity.prompted > from || (activity.asked !== null && activity.asked > from);
  return woken ? null : until;
}

function quietSince(marks: Marks, activity: Activity, now: Date): Date {
  const ranOut = marks.snoozedUntil !== null && marks.snoozedUntil <= now ? marks.snoozedUntil : null;
  const times = [activity.last, activity.prompted, marks.unsettledAt, marks.wokeAt, ranOut].filter((time) => time !== null);
  return new Date(Math.max(...times.map((time) => time.getTime())));
}

/**
 * Where the list shows a thread at `now`. A prompt after the person settled the thread takes it back; a prompt or a
 * request after a snooze started ends the snooze; a pinned or archived thread, or one that works or waits for the
 * person, never settles by itself.
 */
export function shelfOf(listing: { readonly pinned: boolean; readonly archived: boolean; readonly marks: Marks }, activity: Activity, now: Date): Shelf {
  const { marks } = listing;
  if (marks.settledAt !== null && activity.prompted <= marks.settledAt) return { shelf: 'settled', since: marks.settledAt, auto: false };
  const until = snoozedUntil(marks, activity, now);
  if (until !== null) return { shelf: 'snoozed', until };
  const holds = listing.pinned || listing.archived || activity.working || activity.asked !== null;
  const since = quietSince(marks, activity, now);
  if (!holds && now.getTime() - since.getTime() >= SETTLE_AFTER_MS) return { shelf: 'settled', since, auto: true };
  return { shelf: 'active' };
}
