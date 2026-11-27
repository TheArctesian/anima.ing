import type { Size } from '../../display';

/** A part of the pane, in CSS px from its top-left corner. */
export interface Region extends Size {
  readonly x: number;
  readonly y: number;
}

export interface TouchLayout {
  /** The part of the pane the display shows in, from the pane's top-left corner. */
  readonly screen: Size;
  /** The room the display leaves empty, drawn as a trackpad; none when the display fills the pane. */
  readonly pad?: Region | undefined;
  /** The display's scale at rest, in CSS px for each remote pixel. */
  readonly scale: number;
}

/** Less room than this, in CSS px, is too small for a thumb, so it stays empty pane. */
export const LEAST_PAD = 120;
/** The trackpad's size when the display grows into the room: enough for a thumb to slide. */
export const PAD = 160;
/** The least part of the pane, along the split, that the screen takes, so the screen is always bigger than the trackpad. */
export const SCREEN_SHARE = 0.6;

function known(size: Size): boolean {
  return Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0;
}

function turned(size: Size): Size {
  return { width: size.height, height: size.width };
}

/**
 * The layout with the trackpad under the display. A remote that is wider than it is high keeps its whole height and
 * grows until the trackpad is `PAD` high; its sides are then out of the screen, and the view follows the pointer to them.
 * Each remote grows at least until the screen takes `SCREEN_SHARE` of the pane, also when its whole height does not
 * then show. The growth stops at one CSS px for each remote pixel, as a fitted VNC display does (`displayScale` in
 * `../fit`); the screen keeps its share, and the display is in the middle of it.
 */
function padBelow(pane: Size, remote: Size, scale: number): TouchLayout {
  const least = pane.height * SCREEN_SHARE;
  const wanted = remote.width > remote.height ? Math.max(least, pane.height - PAD) : least;
  const rest = Math.max(scale, Math.min(1, wanted / remote.height));
  const height = Math.max(remote.height * rest, least);
  return { screen: { width: pane.width, height }, pad: { x: 0, y: height, width: pane.width, height: pane.height - height }, scale: rest };
}

/**
 * A touch screen moves the remote pointer as a trackpad does (RA-050), from any place in the pane. The display keeps the
 * pane's start, so the room it leaves is in one piece: below it in portrait, beside it in landscape. The remote's short
 * side stays whole, the height of a landscape screen and the width of a portrait one, while the screen is still bigger
 * than the trackpad.
 */
export function trackpadLayout(pane: Size, remote: Size, scale: number): TouchLayout {
  if (!known(pane) || !known(remote) || !(scale > 0)) return { screen: pane, scale };
  const below = pane.height - remote.height * scale;
  const beside = pane.width - remote.width * scale;
  if (Math.max(below, beside) < LEAST_PAD) return { screen: pane, scale };
  if (below >= beside) return padBelow(pane, remote, scale);
  // Beside the display is below it with the pane turned a quarter.
  const { screen, pad, scale: rest } = padBelow(turned(pane), turned(remote), scale);
  return { screen: turned(screen), pad: pad && { x: pad.y, y: pad.x, width: pad.height, height: pad.width }, scale: rest };
}
