/**
 * The queue of a thread as the runner holds it (TH109): the prompts that wait for the turn to end, oldest first, and how
 * the last turn ended. The runner sends the first queued prompt when a turn ended well with nothing open; an interrupt or
 * a failed turn stops the queue until the person sends.
 */

export type LastTurn = 'done' | 'failed' | 'interrupted';

export interface QueuedPrompt {
  readonly id: string;
  readonly text: string;
}

/** The most prompts that wait in the queue of one thread, as the runner counts them. */
export const MOST_QUEUED = 20;

const LAST_TURNS: readonly string[] = ['done', 'failed', 'interrupted'];

/** The queued prompts of a row; a row from a runner without the queue has none. A prompt that does not read is left out. */
export function queuedFrom(value: unknown): readonly QueuedPrompt[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((each: unknown) => {
    if (typeof each !== 'object' || each === null) return [];
    const { id, text } = each as Record<string, unknown>;
    return typeof id === 'string' && typeof text === 'string' ? [{ id, text }] : [];
  });
}

/** How the last turn of a row ended, or `null` when none ended or the runner does not say. */
export function lastTurnFrom(value: unknown): LastTurn | null {
  return typeof value === 'string' && LAST_TURNS.includes(value) ? (value as LastTurn) : null;
}
