import type { TimeWindow } from '$lib/charts/scales';
import type { MetricRange } from './api';

/** The metric ranges an operator can choose (MET-003). */
export const RANGES = [
  { range: 'HOUR', param: 'hour', label: '1 hour', phrase: 'last hour' },
  { range: 'DAY', param: 'day', label: '24 hours', phrase: 'last 24 hours' },
  { range: 'WEEK', param: 'week', label: '7 days', phrase: 'last 7 days' },
] as const satisfies readonly { range: MetricRange; param: string; label: string; phrase: string }[];

const HOUR_MS = 60 * 60 * 1000;
const DAY_MS = 24 * HOUR_MS;
const SPAN_MS: Record<MetricRange, number> = { HOUR: HOUR_MS, DAY: DAY_MS, WEEK: 7 * DAY_MS };
/** WP-28 D-5: the width of one bucket of each range, in milliseconds. */
const BUCKET_MS: Record<MetricRange, number> = { HOUR: 60_000, DAY: 600_000, WEEK: HOUR_MS };

export function rangeWindow(range: MetricRange, now: Date): TimeWindow {
  return { start: now.getTime() - SPAN_MS[range], end: now.getTime() };
}

/** A series whose newest point is older than the window is an earlier answer's copy (FND-007): the window ends at that point. */
export function seriesWindow(range: MetricRange, now: Date, pointTimes: readonly number[]): TimeWindow {
  const newest = Math.max(...pointTimes);
  const old = Number.isFinite(newest) && newest < now.getTime() - SPAN_MS[range];
  return rangeWindow(range, old ? new Date(newest) : now);
}

/** A wider gap than this between two points breaks the line (MET-003): two and a half buckets, so a late point does not break it and two missing buckets do. */
export function rangeGap(range: MetricRange): number {
  return BUCKET_MS[range] * 2.5;
}
