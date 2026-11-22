/** The numbers and the times of the details card in words (07 sections 2.5, 6 and 7). No drawing here. */

/** A count of tokens in short form: "950", "9.5k", "128k", "1.2m". Nothing known is "0". */
export function shortTokens(count: number | null | undefined): string {
  if (count === null || count === undefined || !Number.isFinite(count) || count <= 0) return '0';
  if (count < 1000) return String(Math.round(count));
  if (count < 10_000) return `${trimmed(count / 1000)}k`;
  if (count < 999_500) return `${String(Math.round(count / 1000))}k`;
  return `${trimmed(count / 1_000_000)}m`;
}

function trimmed(value: number): string {
  return value.toFixed(1).replace(/\.0$/, '');
}

/** The time to a reset: "2d 4h", "3h 20m" or "45m". */
export function untilReset(seconds: number): string {
  const minutes = Math.max(0, Math.floor(seconds / 60));
  const hours = Math.floor(minutes / 60);
  if (hours >= 24) return `${String(Math.floor(hours / 24))}d ${String(hours % 24)}h`;
  if (hours > 0) return `${String(hours)}h ${String(minutes % 60)}m`;
  return `${String(minutes)}m`;
}

/** How long an action has run: "42s", then "2m 5s". */
export function elapsed(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return whole < 60 ? `${String(whole)}s` : `${String(Math.floor(whole / 60))}m ${String(whole % 60)}s`;
}

/** The colour of a meter: plain, a warning, or an error. */
export type Tone = 'plain' | 'warning' | 'error';

/** A limit warns at 20% left or less and is an error at 10% or less. */
export function limitTone(left: number): Tone {
  if (left <= 10) return 'error';
  return left <= 20 ? 'warning' : 'plain';
}

/** The context window warns at 80% used or more and is an error at 90% or more. */
export function contextTone(used: number): Tone {
  if (used >= 90) return 'error';
  return used >= 80 ? 'warning' : 'plain';
}

/** A share between 0 and 100, whole. */
export function percent(part: number, whole: number): number {
  if (!(whole > 0)) return 0;
  return Math.min(100, Math.max(0, Math.round((part / whole) * 100)));
}

/** How the use of a limit compares with the time of its window; the engine says which, the card only names it. */
export const PACE = { ahead: 'Ahead of pace', on: 'On pace', under: 'Under pace' } as const;
