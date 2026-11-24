/** The runner sends the first queued prompt after a turn that ended well; an interrupt or a failure stops the queue (TH109). */

export type LastTurn = 'done' | 'failed' | 'interrupted';

export interface QueuedPrompt {
  readonly id: string;
  readonly text: string;
}

export const MOST_QUEUED = 20;

const LAST_TURNS: readonly string[] = ['done', 'failed', 'interrupted'];

/** A row from a runner without the queue has none; a prompt that does not read is left out. */
export function queuedFrom(value: unknown): readonly QueuedPrompt[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((each: unknown) => {
    if (typeof each !== 'object' || each === null) return [];
    const { id, text } = each as Record<string, unknown>;
    return typeof id === 'string' && typeof text === 'string' ? [{ id, text }] : [];
  });
}

export function lastTurnFrom(value: unknown): LastTurn | null {
  return typeof value === 'string' && LAST_TURNS.includes(value) ? (value as LastTurn) : null;
}
