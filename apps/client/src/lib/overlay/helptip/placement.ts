export interface Size {
  readonly width: number;
  readonly height: number;
}

/** In viewport coordinates, as `getBoundingClientRect` gives it. */
export interface Anchor extends Size {
  readonly left: number;
  readonly top: number;
}

export function placeInside(anchor: Anchor, panel: Size, viewport: Size): { left: number; top: number } {
  const below = anchor.top + anchor.height;
  const above = anchor.top - panel.height;
  const fitsBelow = below + panel.height <= viewport.height;
  const top = fitsBelow || above < 0 ? below : above;
  const left = Math.min(anchor.left, viewport.width - panel.width);
  return { left: Math.max(0, left), top: Math.max(0, Math.min(top, viewport.height - panel.height)) };
}
