/** The plot's drawing box, in viewBox units; the SVG stretches it to its slot, so lines keep a fixed stroke. */
export const PLOT = { width: 600, height: 120 } as const;

/** The value axis is cut into this many equal intervals, so every chart and its skeleton share one grid. */
const VALUE_INTERVALS = 4;

export interface TimeWindow {
  readonly start: number;
  readonly end: number;
}

export type ValueAxis = 'percent' | 'open';

export interface ValueScale {
  readonly top: number;
  readonly step: number;
}

const NICE = [1, 2, 2.5, 5, 10];
const PERCENT: ValueScale = { top: 100, step: 25 };
const EMPTY: ValueScale = { top: 1, step: 0.25 };

/** Drops the floating-point noise of `2.5 * 0.1`. */
function clean(value: number): number {
  return Number(value.toPrecision(12));
}

export function lengthOf(span: TimeWindow): number {
  const length = span.end - span.start;
  return Number.isFinite(length) && length > 0 ? length : 0;
}

export function timeScale(span: TimeWindow): (at: number) => number {
  const length = lengthOf(span);
  return (at) => (length === 0 ? PLOT.width : ((at - span.start) / length) * PLOT.width);
}

export function valueScale(axis: ValueAxis, highest: number): ValueScale {
  if (axis === 'percent') return PERCENT;
  if (!Number.isFinite(highest) || highest <= 0) return EMPTY;
  const wanted = highest / VALUE_INTERVALS;
  const power = 10 ** Math.floor(Math.log10(wanted));
  const step = clean((NICE.find((nice) => clean(nice * power) >= wanted) ?? 10) * power);
  return { top: clean(step * VALUE_INTERVALS), step };
}

export function valueY(scale: ValueScale, value: number): number {
  return PLOT.height - (value / scale.top) * PLOT.height;
}

export function slideOffset(was: TimeWindow, now: TimeWindow): number {
  const length = lengthOf(now);
  const moved = now.start - was.start;
  if (length === 0 || lengthOf(was) !== length || !(moved > 0 && moved < length)) return 0;
  return (moved / length) * PLOT.width;
}
