import { nearestSnap, rubberband } from '$lib/motion/gesture';

/** In px, measured from the layout at the grab. */
export interface BarWidths {
  readonly icons: number;
  readonly full: number;
}

/** Rubber-band resistance below the icon width and above the full width (5.7.4, MOT-033). */
export function draggedWidth(position: number, widths: BarWidths): number {
  if (position < widths.icons) return widths.icons + rubberband(position - widths.icons, widths.icons);
  if (position > widths.full) return widths.full + rubberband(position - widths.full, widths.full);
  return position;
}

/** The bar rests at the width nearest where the release would carry it (MOT-032, MOT-034). */
export function restsCollapsed(position: number, velocityPxPerS: number, widths: BarWidths): boolean {
  return nearestSnap(position, velocityPxPerS, [widths.icons, widths.full]) === widths.icons;
}

const WIDTH = '--nav-inline-size';

/** The page follows, as the hand does (5.7.4). */
export function holdWidth(root: HTMLElement, width: number): void {
  root.style.setProperty(WIDTH, `${String(width)}px`);
}

export function releaseWidth(root: HTMLElement): void {
  root.style.removeProperty(WIDTH);
}
