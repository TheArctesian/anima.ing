import type { SessionDisplay } from '../api';

export interface Size {
  readonly width: number;
  readonly height: number;
}

/** The size range the hub forwards (K4 §4, R-018). */
export const SIZE_MIN = 1;
export const SIZE_MAX = 8192;
/** The DPI range the hub forwards (K4 §4). */
export const DPI_MIN = 48;
export const DPI_MAX = 480;
const CSS_DPI = 96;

/** A hidden pane measures 0 × 0; the first real resize corrects this size. */
export const UNMEASURED_PANE: Size = { width: 1024, height: 768 };

export const RESIZE_SETTLE_MS = 150;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function ratio(devicePixelRatio: number): number {
  return Number.isFinite(devicePixelRatio) && devicePixelRatio > 0 ? devicePixelRatio : 1;
}

export function isMeasured(pane: Size): boolean {
  return pane.width > 0 && pane.height > 0;
}

/** `pane` is in CSS px; the result is in device pixels (D-7). */
export function remoteSize(pane: Size, devicePixelRatio: number): Size {
  const scale = ratio(devicePixelRatio);
  const pixels = (css: number) => clamp(Math.round((Number.isFinite(css) ? css : 0) * scale), SIZE_MIN, SIZE_MAX);
  return { width: pixels(pane.width), height: pixels(pane.height) };
}

export function displayDpi(devicePixelRatio: number): number {
  return clamp(Math.round(CSS_DPI * ratio(devicePixelRatio)), DPI_MIN, DPI_MAX);
}

/** The `DisplayInput` a session starts with (GW-012, D-7). */
export function displayInput(pane: Size, devicePixelRatio: number, timezone: string): SessionDisplay {
  const size = remoteSize(isMeasured(pane) ? pane : UNMEASURED_PANE, devicePixelRatio);
  return { widthPx: size.width, heightPx: size.height, dpi: displayDpi(devicePixelRatio), timezone };
}

export interface SettleTimer {
  set(callback: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

export interface SizeSender {
  resized(size: Size): void;
  cancel(): void;
}

/** The pane's size never depends on what is sent, so a sent size cannot cause another resize (D-7). */
export function sizeSender(initial: Size, send: (size: Size) => void, timer: SettleTimer): SizeSender {
  let last = initial;
  let waiting: unknown;
  const cancel = () => {
    if (waiting !== undefined) timer.clear(waiting);
    waiting = undefined;
  };
  return {
    resized: (size) => {
      cancel();
      waiting = timer.set(() => {
        waiting = undefined;
        if (size.width === last.width && size.height === last.height) return;
        last = size;
        send(size);
      }, RESIZE_SETTLE_MS);
    },
    cancel,
  };
}
