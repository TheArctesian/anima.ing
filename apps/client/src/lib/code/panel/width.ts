/**
 * The width of the right panel of Code (report 02, Section 4.3): 540px at first, never under 360px, and never so wide
 * that the chat column has less than 360px or the panel more than 0.7 of the window.
 */
export const DEFAULT_WIDTH = 540;
export const MIN_WIDTH = 360;
/** At this width of the window and below, the panel is a sheet over the chat and not a column beside it. */
export const SHEET_AT = 980;
/** One press of an arrow key on the edge of the panel. */
export const KEY_STEP = 16;

export function maxWidth(windowWidth: number, rowWidth: number): number {
  return Math.max(MIN_WIDTH, Math.min(Math.floor(windowWidth * 0.7), rowWidth - MIN_WIDTH));
}

export function clamped(width: number, windowWidth: number, rowWidth: number): number {
  if (!Number.isFinite(width)) return DEFAULT_WIDTH;
  return Math.round(Math.max(MIN_WIDTH, Math.min(maxWidth(windowWidth, rowWidth), width)));
}

/** The panel is kept for each thread, as T3 Code keeps it. */
export function storageKey(thread: string): string {
  return `anima.code.panel-width:${thread}`;
}

/** The kept width of a thread, or the first width. A store that cannot be read gives the first width. */
export function storedWidth(store: Pick<Storage, 'getItem'>, thread: string): number {
  try {
    const kept = Number(store.getItem(storageKey(thread)));
    return Number.isFinite(kept) && kept >= MIN_WIDTH ? kept : DEFAULT_WIDTH;
  } catch {
    return DEFAULT_WIDTH;
  }
}

export function keepWidth(store: Pick<Storage, 'setItem'>, thread: string, width: number): void {
  try {
    store.setItem(storageKey(thread), String(Math.round(width)));
  } catch {
    // The width is a convenience; the panel works without the store.
  }
}
