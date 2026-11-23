import { FRAME_MAX_BYTES } from './codec';

/** The upload waits while the socket holds more than this many unsent bytes (K4 §8.5 backpressure, D-3). */
export const HOLD_BACK_BYTES = 1_048_576;
export const DRAIN_POLL_MS = 50;

export interface UploadSource {
  readonly size: number;
  slice(start: number, end: number): { arrayBuffer(): Promise<ArrayBuffer> };
}

/** Bytes the socket has taken so far and the upload's size (MOT-009: real progress, never above `size`). */
export type Progress = (done: number, size: number) => void;

export interface UploadPort {
  /** Send one binary frame; false when the link can no longer send. */
  readonly send: (frame: ArrayBuffer) => boolean;
  readonly buffered: () => number;
  /** Resolves after `ms`, or at once when `signal` aborts, leaving no timer (CQ-021). */
  readonly wait: (ms: number, signal: AbortSignal) => Promise<void>;
}

export interface UploadPump {
  stop(): void;
  sent(): boolean;
}

/** `progress` hears the bytes the socket has taken, rising only; after `unreadable` no frame follows. */
export function pumpUpload(source: UploadSource, port: UploadPort, progress: Progress, unreadable: () => void): UploadPump {
  const size = source.size;
  let stopped = false;
  const stopping = new AbortController();
  let offset = 0;
  let taken = 0;

  const report = () => {
    const now = Math.min(size, Math.max(taken, offset - port.buffered()));
    if (now === taken) return;
    taken = now;
    progress(taken, size);
  };

  const run = async () => {
    while (!stopped && offset < size) {
      if (port.buffered() > HOLD_BACK_BYTES) {
        await port.wait(DRAIN_POLL_MS, stopping.signal);
        report();
        continue;
      }
      const end = Math.min(size, offset + FRAME_MAX_BYTES);
      let frame: ArrayBuffer;
      try {
        frame = await source.slice(offset, end).arrayBuffer();
      } catch {
        frame = new ArrayBuffer(0);
      }
      if (stopped) return;
      if (frame.byteLength !== end - offset) {
        stopped = true;
        unreadable();
        return;
      }
      if (!port.send(frame)) return;
      offset = end;
      report();
    }
    while (!stopped && taken < size) {
      await port.wait(DRAIN_POLL_MS, stopping.signal);
      report();
    }
  };
  // Started after the caller holds the pump, so even a slice that fails at once finds the transfer to stop.
  void Promise.resolve().then(run);

  return {
    stop: () => {
      stopped = true;
      stopping.abort();
    },
    sent: () => offset === size,
  };
}
