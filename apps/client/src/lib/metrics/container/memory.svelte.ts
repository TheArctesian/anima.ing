import type { Hub, HubError } from '$lib/hub/hub';
import { containerApi } from './api';
import { mergeMemoryBytes } from './container';

/** Memory in use of the hub's own device (HMD-073); unknown until the first batch, pushed every 10 seconds (MET-018). */
export class LiveMemory {
  bytes = $state.raw<number | null>(null);
  error = $state.raw<HubError | null>(null);
  #deviceId: string | null = null;
  #stop: (() => void) | undefined;

  of(id: string): number | null {
    return id === this.#deviceId ? this.bytes : null;
  }

  watch(hub: Pick<Hub, 'subscribe'>, deviceId: string): void {
    if (deviceId === this.#deviceId) return;
    this.stop();
    this.#deviceId = deviceId;
    this.#stop = containerApi(hub).watch(deviceId, {
      next: ({ samples }) => {
        this.bytes = mergeMemoryBytes(this.bytes, samples);
      },
      error: (failure) => {
        this.error = failure;
      },
      complete: () => undefined,
    });
  }

  stop(): void {
    this.#stop?.();
    this.#stop = undefined;
    this.#deviceId = null;
    this.bytes = null;
  }
}
