import type { HubError, HubResult } from '$lib/hub/hub';
import { onReturn } from '../live.svelte';
import { writeRefused } from './status';

interface Request<T> {
  id: string;
  submit: () => Promise<HubResult<T>>;
}

export function followReceipt<T>(read: (id: string) => Promise<HubResult<T>>, waiting: (receipt: T) => boolean) {
  let request = $state.raw<Request<T> | null>(null);
  let receipt = $state.raw<T | null>(null);
  let busy = $state(false);
  let error = $state.raw<HubError | null>(null);

  async function run(submit: boolean): Promise<void> {
    if (request === null || busy) return;
    busy = true;
    if (submit) error = null;
    const result = await (submit ? request.submit() : read(request.id));
    busy = false;
    if (!result.ok) {
      if (submit || receipt !== null) error = result.error;
      if (submit && writeRefused(result.error)) request = null;
      return;
    }
    error = null;
    receipt = result.value;
    if (!waiting(receipt)) request = null;
  }

  return {
    get busy() { return busy; },
    get receipt() { return receipt; },
    get error() { return error; },
    get pending() { return request !== null; },
    get canRetry() { return request !== null && receipt === null && error !== null && !busy; },
    start(next: Request<T>) {
      if (request !== null) return;
      receipt = null;
      request = next;
      return run(true);
    },
    retry: () => run(true),
    refresh: () => run(false),

  };
}

const REFRESH_MS = 5_000;

export function watchRemote(refresh: () => void): () => void {
  const timer = setInterval(() => {
    if (document.visibilityState === 'visible') refresh();
  }, REFRESH_MS);
  const stop = onReturn(refresh);
  return () => {
    clearInterval(timer);
    stop();
  };
}
