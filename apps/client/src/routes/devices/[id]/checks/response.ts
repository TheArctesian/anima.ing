import type { ChartPoint } from '$lib/charts/plot';
import type { TimeWindow } from '$lib/charts/scales';

/** The most heartbeats the chart draws: one history page (D-10). */
export const RESPONSE_POINTS = 100;

export interface ChartBeat {
  readonly at: string;
  readonly responseTimeMs?: number | null;
}

/** The x axis counts beats, not time, so the newest stays at the right edge and the spacing is even (Chart update role). */
export interface ResponseSteps {
  readonly points: readonly ChartPoint[];
  readonly span: TimeWindow;
}

/** One beat more sits just left of the window, so the left edge is not empty while a new beat slides the line in. */
export function responseSteps(beats: readonly ChartBeat[], newest: number): ResponseSteps {
  const points = beats.slice(0, RESPONSE_POINTS + 1).map((beat, index) => ({ at: newest - index, value: beat.responseTimeMs ?? null }));
  return { points, span: { start: newest - (RESPONSE_POINTS - 1), end: newest } };
}

/** A first load or older beats add no step, so they draw in place. */
export function stepsAfter(was: readonly ChartBeat[], now: readonly ChartBeat[], steps: number): number {
  const before = was[0];
  if (before === undefined) return steps;
  const newestBefore = Date.parse(before.at);
  const newer = now.filter((beat) => Date.parse(beat.at) > newestBefore).length;
  return steps + newer;
}
