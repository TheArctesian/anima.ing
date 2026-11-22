/** What a change does, for undo grouping (03, The editor: Undo grouping). Paste, drop, cut and every rewrite are `other`. */
export type ChangeKind = 'insert' | 'delete' | 'other';

/** The prompt and its selection at one moment. */
export interface Moment {
  readonly value: string;
  readonly anchor: number;
  readonly focus: number;
}

export interface History {
  readonly past: readonly Moment[];
  readonly future: readonly Moment[];
  /** The kind and the time of the last change; null after an undo, a redo or a fresh prompt. */
  readonly kind: ChangeKind | null;
  readonly at: number;
}

/** Changes of one kind less than this apart are one step of undo (T3's `newGroupDelay`). */
export const UNDO_GROUP_DELAY = 1000;
/** How many steps are kept. */
export const UNDO_DEPTH = 200;

export const NO_HISTORY: History = { past: [], future: [], kind: null, at: 0 };

/**
 * The history after a change of `kind` at `now`, made from the moment `before` it. A new step starts when the kind
 * changes, after `UNDO_GROUP_DELAY` without a change, and for every `other` change.
 */
export function recorded(history: History, before: Moment, kind: ChangeKind, now: number): History {
  const joins = history.kind === kind && kind !== 'other' && now - history.at <= UNDO_GROUP_DELAY;
  const past = joins ? history.past : [...history.past, before].slice(-UNDO_DEPTH);
  return { past, future: [], kind, at: now };
}

/** One step back: the moment to show and the history after it, or null when there is nothing to undo. */
export function undone(history: History, current: Moment): { history: History; moment: Moment } | null {
  const moment = history.past.at(-1);
  if (moment === undefined) return null;
  return { moment, history: { past: history.past.slice(0, -1), future: [...history.future, current], kind: null, at: 0 } };
}

export function redone(history: History, current: Moment): { history: History; moment: Moment } | null {
  const moment = history.future.at(-1);
  if (moment === undefined) return null;
  return { moment, history: { past: [...history.past, current], future: history.future.slice(0, -1), kind: null, at: 0 } };
}
