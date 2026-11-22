/**
 * The width of the file explorer beside an open file (t3-ui-copy-2026-11-21/05, Section 2.9): 256px at first, from 160
 * to 560px, and the content of the file always keeps 256px.
 */
export const EXPLORER_DEFAULT = 256;
export const EXPLORER_MIN = 160;
export const EXPLORER_MAX = 560;
export const CONTENT_MIN = 256;

/** The widest the explorer may be in a row of this width. */
export function explorerMax(rowWidth: number): number {
  return Math.max(EXPLORER_MIN, Math.min(EXPLORER_MAX, Math.floor(rowWidth - CONTENT_MIN)));
}

/** The asked width held to its limits for a row of this width. */
export function explorerWidth(asked: number, rowWidth: number): number {
  if (!Number.isFinite(asked)) return Math.min(EXPLORER_DEFAULT, explorerMax(rowWidth));
  return Math.round(Math.max(EXPLORER_MIN, Math.min(explorerMax(rowWidth), asked)));
}
