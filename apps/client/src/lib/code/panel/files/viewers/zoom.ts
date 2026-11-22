/**
 * The zoom of the image viewer (t3-ui-copy-2026-11-21/05, Section 3.10): each step is times or divided by 1.25, from
 * 0.05 to 8; "fit" is the largest scale that fits the image in the viewport less 32px, never above 1.
 */
export const ZOOM_STEP = 1.25;
export const ZOOM_MIN = 0.05;
export const ZOOM_MAX = 8;
/** The room around the image: 16px on each side. */
export const FIT_MARGIN = 32;

export interface Size {
  readonly width: number;
  readonly height: number;
}

/** A scale held to the range of the zoom. */
export function clampZoom(scale: number): number {
  return Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, scale));
}

/** One step in. */
export function zoomIn(scale: number): number {
  return clampZoom(scale * ZOOM_STEP);
}

/** One step out. */
export function zoomOut(scale: number): number {
  return clampZoom(scale / ZOOM_STEP);
}

/** The scale that fits the image in the viewport; 1 while the image or the viewport has no size yet. */
export function fitScale(image: Size, viewport: Size): number {
  const room = { width: viewport.width - FIT_MARGIN, height: viewport.height - FIT_MARGIN };
  if (image.width <= 0 || image.height <= 0 || room.width <= 0 || room.height <= 0) return 1;
  return clampZoom(Math.min(1, room.width / image.width, room.height / image.height));
}

/** The scale as the bar says it: "100%". */
export function percent(scale: number): string {
  return `${String(Math.round(scale * 100))}%`;
}

/** The size of the image as the bar says it: "1,920 × 1,080". */
export function sizeLabel(image: Size): string {
  return `${image.width.toLocaleString('en-US')} × ${image.height.toLocaleString('en-US')}`;
}
