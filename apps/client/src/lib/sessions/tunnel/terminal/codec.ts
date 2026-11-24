import { isEndReason, type SessionEndReason } from '../../ending';

/** The WebSocket subprotocol of the terminal tunnel (K4 §8.1). */
export const TERMINAL_SUBPROTOCOL = 'anima-terminal';

/** The terminal size in cells the hub accepts (K4 §8.4). */
export const CELLS_MIN = 1;
export const CELLS_MAX = 1000;

/** The largest binary message the client sends is 64 KiB (K4 §8.5); input is cut well under it. */
export const FRAME_MAX_BYTES = 32_768;

/** No message of the closed set is longer than this; a longer text is not parsed at all. */
export const TEXT_MAX_CHARS = 256;

export interface CellSize {
  readonly cols: number;
  readonly rows: number;
}

export type HubMessage =
  | { readonly kind: 'ping' }
  | { readonly kind: 'end'; readonly reason: SessionEndReason };

function cells(value: number): number {
  return Number.isFinite(value) ? Math.min(CELLS_MAX, Math.max(CELLS_MIN, Math.round(value))) : CELLS_MIN;
}

export function clampSize(size: CellSize): CellSize {
  return { cols: cells(size.cols), rows: cells(size.rows) };
}

export function encodeResize(size: CellSize): string {
  return JSON.stringify({ type: 'resize', ...clampSize(size) });
}

export function encodePing(): string {
  return JSON.stringify({ type: 'ping' });
}

/** Anything outside the closed set gives `undefined` and is ignored, so that a newer hub's message does not end a session. */
export function decodeHubMessage(text: string): HubMessage | undefined {
  if (text.length > TEXT_MAX_CHARS) return undefined;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return undefined;
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined;
  const fields = value as Record<string, unknown>;
  const keys = Object.keys(fields);
  if (fields.type === 'ping' && keys.length === 1) return { kind: 'ping' };
  if (fields.type === 'end' && keys.length === 2 && isEndReason(fields.reason)) return { kind: 'end', reason: fields.reason };
  return undefined;
}

export function inputFrames(bytes: Uint8Array<ArrayBuffer>): Uint8Array<ArrayBuffer>[] {
  const frames: Uint8Array<ArrayBuffer>[] = [];
  for (let at = 0; at < bytes.length; at += FRAME_MAX_BYTES) frames.push(bytes.subarray(at, at + FRAME_MAX_BYTES));
  return frames;
}
