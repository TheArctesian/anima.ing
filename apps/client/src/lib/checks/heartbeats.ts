import type { NewHeartbeat } from './api';

/** Section 6.1. */
export const BAR_LENGTH = 50;

export interface Beat {
  readonly checkId: string;
  readonly at: string;
  readonly status: NewHeartbeat['heartbeat']['status'];
  readonly responseTimeMs?: number | null;
}

const NO_UPTIME = '—';

/** Time order; a time that does not parse sorts as the oldest. */
function instantOf(at: string): number {
  const time = Date.parse(at);
  return Number.isNaN(time) ? Number.NEGATIVE_INFINITY : time;
}

function when(beat: Beat): number {
  return instantOf(beat.at);
}

/** Heartbeats have no id: one check records at most one per instant, and `Z` and `+00:00` are one instant. */
function sameBeat(a: Beat, b: Beat): boolean {
  return a.checkId === b.checkId && when(a) === when(b);
}

function capped(beats: readonly Beat[]): readonly Beat[] {
  return beats.length > BAR_LENGTH ? beats.slice(-BAR_LENGTH) : beats;
}

interface CheckBeats {
  readonly id: string;
  readonly heartbeats: { readonly edges: readonly { readonly node: Omit<Beat, 'checkId'> }[] };
}

/** Merged from all the device's checks, oldest first (D-6); server order is not trusted. */
export function latestBeats(checks: readonly CheckBeats[]): readonly Beat[] {
  const seen = new Map<string, Set<number>>();
  const unique: { readonly beat: Beat; readonly instant: number }[] = [];
  for (const check of checks) {
    const instants = seen.get(check.id) ?? new Set<number>();
    seen.set(check.id, instants);
    for (const { node } of check.heartbeats.edges) {
      const instant = instantOf(node.at);
      if (instants.has(instant)) continue;
      instants.add(instant);
      unique.push({ beat: { checkId: check.id, ...node }, instant });
    }
  }
  // Each time is parsed once; sorting by the parsed instants keeps the cost at n log n.
  return capped(unique.toSorted((a, b) => a.instant - b.instant).map(({ beat }) => beat));
}

/** A duplicate, or a beat older than a full bar, returns `beats` itself so a caller can see that nothing changed. */
export function addBeat(beats: readonly Beat[], beat: Beat): readonly Beat[] {
  if (beats.some((known) => sameBeat(known, beat))) return beats;
  const time = when(beat);
  const after = beats.findLastIndex((known) => when(known) <= time) + 1;
  if (after === 0 && beats.length >= BAR_LENGTH) return beats;
  return capped([...beats.slice(0, after), beat, ...beats.slice(after)]);
}

export function beatOf(added: NewHeartbeat): Beat {
  return { checkId: added.checkId, ...added.heartbeat };
}

export function cellsOf(beats: readonly Beat[]): readonly (Beat['status'] | null)[] {
  const empty = Array.from({ length: Math.max(BAR_LENGTH - beats.length, 0) }, () => null);
  return [...empty, ...beats.slice(-BAR_LENGTH).map((beat) => beat.status)];
}

export function barSummary(beats: readonly Beat[]): string {
  const latest = beats.at(-1);
  if (latest === undefined) return 'No heartbeats yet';
  const count = beats.length === 1 ? '1 heartbeat' : `${String(beats.length)} heartbeats`;
  return `${count}, latest ${latest.status.toLowerCase()}`;
}

/** A bar that showed no beat is being loaded, not fed, so it does not move. */
export function shiftedByNewBeat(was: readonly Beat[], now: readonly Beat[]): boolean {
  const latest = now.at(-1);
  const previous = was.at(-1);
  return latest !== undefined && previous !== undefined && !sameBeat(previous, latest);
}

/** Rounding never shows "100.00 %" for an uptime below 1. */
export function uptimeText(ratio: number | null, compact = false): string {
  if (ratio === null || Number.isNaN(ratio)) return NO_UPTIME;
  const percent = Math.min(Math.max(ratio, 0), 1) * 100;
  const rounded = percent.toFixed(2);
  const text = rounded === '100.00' && percent < 100 ? '99.99' : rounded;
  return compact ? `${Number(text)}%` : `${text} %`;
}
