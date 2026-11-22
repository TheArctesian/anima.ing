/** How many bars the effort glyph draws and how many of them are full (03 A.3.2). */
export interface EffortBars {
  readonly count: number;
  readonly filled: number;
}

/** One bar of the glyph, in whole pixels of its box. */
export interface GlyphBar {
  readonly x: number;
  readonly y: number;
  readonly height: number;
  readonly full: boolean;
}

/** More bars than this do not fit the glyph's height in whole pixels. */
export const MAX_BARS = 7;

const HEIGHTS = { sm: 14, xs: 12 } as const;
/* The bars stop 2 px over the bottom of the box, which puts their base on the text baseline. */
const BASELINE_INSET = 2;
const BAR = 2;
const PITCH = 4;

/**
 * The heights of `count` bars that rise by one whole step from about half of `maxBar` to `maxBar`. The step is
 * rounded once: rounding each bar would mix steps of 1 and 2 px.
 */
export function barHeights(count: number, maxBar: number): number[] {
  const step = Math.max(1, Math.round(maxBar / 2 / Math.max(count - 1, 1)));
  return Array.from({ length: count }, (_, index) => maxBar - (count - 1 - index) * step);
}

/** The box of the glyph and its bars: 2 px bars every 4 px, standing on one line. */
export function glyphBox(bars: EffortBars, size: 'sm' | 'xs'): { width: number; height: number; barWidth: number; bars: GlyphBar[] } {
  const height = HEIGHTS[size];
  const maxBar = height - BASELINE_INSET;
  return {
    width: Math.max(0, PITCH * bars.count - BAR),
    height,
    barWidth: BAR,
    bars: barHeights(bars.count, maxBar).map((barHeight, index) => ({
      x: index * PITCH,
      y: maxBar - barHeight,
      height: barHeight,
      full: index < bars.filled,
    })),
  };
}
