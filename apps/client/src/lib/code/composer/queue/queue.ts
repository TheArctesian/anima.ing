import type { Chip } from '../draft';

/** A message that waits for the running turn to end (03, "3. Queued messages"). */
export interface Queued {
  readonly id: string;
  /** The prompt as it will be sent: Markdown. */
  readonly prompt: string;
  /** Not yet taken by the runner: it can be neither sent, edited nor removed (03 3.4, "Saving"). */
  readonly saving: boolean;
}

/** More than three queued messages fold to the first two and a line that shows the rest (03 3.2). */
export const QUEUE_ROWS = 3;

export function foldable(queue: readonly Queued[]): boolean {
  return queue.length > QUEUE_ROWS;
}

/** The messages on screen: all of them, or the first two of a folded queue. */
export function shown(queue: readonly Queued[], all: boolean): readonly Queued[] {
  return foldable(queue) && !all ? queue.slice(0, QUEUE_ROWS - 1) : queue;
}

/** The name of the list for assistive technology: "1 queued message", "3 queued messages". */
export function queueName(count: number): string {
  return `${String(count)} queued ${count === 1 ? 'message' : 'messages'}`;
}

export function foldWords(count: number, all: boolean): string {
  return all ? 'Show fewer' : `Show all ${String(count)} queued`;
}

/** The tip of "Send now": into the running turn, or as the next turn (03 3.4). */
export function sendNowTip(running: boolean): string {
  return running ? 'Send into the current turn' : 'Start a turn with this message';
}

/** The prompt as a bubble shows it: each chip by its label, so a file link reads as the file's name (03 3.3). */
export function labelled(prompt: string, chips: readonly Chip[]): string {
  let words = '';
  let at = 0;
  for (const chip of [...chips].sort((a, b) => a.start - b.start)) {
    if (chip.start < at || chip.end > prompt.length) continue;
    words += prompt.slice(at, chip.start) + chip.label;
    at = chip.end;
  }
  return words + prompt.slice(at);
}
