/** The size of the page under the device toolbar of the browser surface (report 05, Section 6.5). */
export interface Viewport {
  readonly width: number;
  readonly height: number;
  /** The id of a device, or none for a free size ("Responsive"). */
  readonly preset?: string | undefined;
  /** A change of one side changes the other, so that the shape stays. */
  readonly locked?: boolean;
}

export interface DevicePreset {
  readonly id: string;
  readonly label: string;
  readonly width: number;
  readonly height: number;
}

/** The devices that the report captured; T3 Code has more. */
export const PRESETS: readonly DevicePreset[] = [
  { id: 'iphone-se', label: 'iPhone SE', width: 375, height: 667 },
  { id: 'iphone-xr', label: 'iPhone XR', width: 414, height: 896 },
  { id: 'iphone-12-pro', label: 'iPhone 12 Pro', width: 390, height: 844 },
  { id: 'iphone-14-pro-max', label: 'iPhone 14 Pro Max', width: 430, height: 932 },
  { id: 'pixel-7', label: 'Pixel 7', width: 412, height: 915 },
];

export const FREE_VIEWPORT: Viewport = { width: 1280, height: 800 };
export const MIN_SIDE = 50;
export const MAX_SIDE = 9999;

const clamped = (side: number): number => Math.max(MIN_SIDE, Math.min(MAX_SIDE, Math.round(side)));

/** With the size of a device; an unknown id, or none, is the free size and keeps the size that shows. */
export function preset(viewport: Viewport, id: string | undefined): Viewport {
  const device = PRESETS.find((each) => each.id === id);
  if (device === undefined) return { width: viewport.width, height: viewport.height, locked: viewport.locked ?? false };
  return { width: device.width, height: device.height, preset: device.id, locked: viewport.locked ?? false };
}

/** With one side typed by the person: the size is free then, and a locked shape moves the other side with it. */
export function resized(viewport: Viewport, side: 'width' | 'height', value: number): Viewport {
  if (!Number.isFinite(value)) return viewport;
  const next = clamped(value);
  const other = side === 'width' ? 'height' : 'width';
  const ratio = viewport[other] / viewport[side];
  const kept = viewport.locked === true ? clamped(next * ratio) : viewport[other];
  return { width: side === 'width' ? next : kept, height: side === 'height' ? next : kept, locked: viewport.locked ?? false };
}

/** Turned a quarter: the sides change places, and the device stays. */
export function rotated(viewport: Viewport): Viewport {
  return { ...viewport, width: viewport.height, height: viewport.width };
}
