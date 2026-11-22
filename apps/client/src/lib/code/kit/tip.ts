import type { Anchor, Size } from '$lib/overlay/helptip/placement';

/**
 * Where a tip of `tip` size goes: on the `at.side` of `anchor`, `at.gap` away and centred on it, kept inside a window
 * of `viewport` size; on the other side when the asked one has no room. `at.room` is the run of the page the tip may
 * cover across, by default the whole window: the Code page gives its own, so a tip by its edge does not cover Anima's
 * sidebar (T3 Code keeps a tip inside its window, and the Code page is that window here).
 */
export function tipPlace(
  anchor: Anchor,
  tip: Size,
  viewport: Size,
  at: { side: 'above' | 'below'; gap: number; room?: { left: number; right: number } },
): { left: number; top: number } {
  const above = anchor.top - at.gap - tip.height;
  const below = anchor.top + anchor.height + at.gap;
  const roomAbove = above >= 0;
  const roomBelow = below + tip.height <= viewport.height;
  const useAbove = at.side === 'above' ? roomAbove || !roomBelow : !roomBelow && roomAbove;
  const top = useAbove ? above : below;
  const centred = anchor.left + anchor.width / 2 - tip.width / 2;
  const room = at.room ?? { left: 0, right: viewport.width };
  const left = Math.max(room.left, Math.min(centred, room.right - tip.width));
  return { left, top: Math.max(0, Math.min(top, viewport.height - tip.height)) };
}
