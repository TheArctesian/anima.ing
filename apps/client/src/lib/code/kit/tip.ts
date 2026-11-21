import type { Anchor, Size } from '$lib/overlay/helptip/placement';

/**
 * Where a tip of `tip` size goes: on the `at.side` of `anchor`, `at.gap` away and centred on it, kept inside a window
 * of `viewport` size; on the other side when the asked one has no room.
 */
export function tipPlace(anchor: Anchor, tip: Size, viewport: Size, at: { side: 'above' | 'below'; gap: number }): { left: number; top: number } {
  const above = anchor.top - at.gap - tip.height;
  const below = anchor.top + anchor.height + at.gap;
  const roomAbove = above >= 0;
  const roomBelow = below + tip.height <= viewport.height;
  const useAbove = at.side === 'above' ? roomAbove || !roomBelow : !roomBelow && roomAbove;
  const top = useAbove ? above : below;
  const centred = anchor.left + anchor.width / 2 - tip.width / 2;
  const left = Math.max(0, Math.min(centred, viewport.width - tip.width));
  return { left, top: Math.max(0, Math.min(top, viewport.height - tip.height)) };
}
