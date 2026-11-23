/**
 * The one line that the runner's words become over the chat (TH98). The daemon's words for an older holder are two
 * sentences; the page says what they mean in one short line, and offers the stop they ask for. Other words, such as a
 * runner that is not running, show as they are. Pure.
 */
export interface Notice {
  /** The words of the line. */
  readonly line: string;
  /** The daemon's full words, for the tip of the line, when the line is shorter. */
  readonly full: string | null;
  /** How many threads run and keep the older holder; the line offers to stop them when there are any. */
  readonly running: number;
}

const OLDER = 'The holder of the threads is older than the daemon';
const RUNNING = /\((\d+) running now\)/;

export function noticeOf(words: string): Notice {
  if (!words.startsWith(OLDER)) return { line: words, full: null, running: 0 };
  const running = Number(RUNNING.exec(words)?.[1] ?? 0);
  const line = running === 0 ? 'The thread holder of an older build is being replaced.' : 'The thread holder is of an older build; this build takes over when no thread runs.';
  return { line, full: words, running };
}

/** The words of the action: "Stop the 2 running threads and replace". */
export function stopWords(running: number): string {
  return running === 1 ? 'Stop the running thread and replace' : `Stop the ${String(running)} running threads and replace`;
}
