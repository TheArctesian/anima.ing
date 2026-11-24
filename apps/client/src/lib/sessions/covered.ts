/** Where the pages were scrolled when a session tab covered them (WP-14 E-2). */
export interface CoveredScroll {
  readonly path: string;
  readonly y: number;
}

/** A covered page has no height, so the document loses its scroll position; only the same page gets it back. */
export function restoredScroll(covered: CoveredScroll | null, path: string): number | undefined {
  return covered?.path === path ? covered.y : undefined;
}
