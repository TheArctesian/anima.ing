import type { Anchor, Size } from '$lib/overlay/helptip/placement';

/**
 * Where a tip of `tip` size goes: above `anchor` by `gap` and centred on it, kept inside a window of `viewport` size;
 * under the anchor when there is no room above.
 */
export function tipPlace(anchor: Anchor, tip: Size, viewport: Size, gap: number): { left: number; top: number } {
  const above = anchor.top - gap - tip.height;
  const top = above >= 0 ? above : anchor.top + anchor.height + gap;
  const centred = anchor.left + anchor.width / 2 - tip.width / 2;
  const left = Math.max(0, Math.min(centred, viewport.width - tip.width));
  return { left, top: Math.max(0, Math.min(top, viewport.height - tip.height)) };
}
