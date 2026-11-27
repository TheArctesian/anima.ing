import type { HubError, HubResult } from '$lib/hub/hub';
import { onReturn } from '../live.svelte';
import type { RecoveryScope } from './recovery';
import { writeRefused } from './status';

interface Request<T> {
  id: string;
  submit?: () => Promise<HubResult<T>>;
}

interface Policy<T> {
  waiting: (receipt: T) => boolean;
  acknowledge?: (receipt: T) => boolean;
}

interface Slot {
  scope: RecoveryScope;
  key: string;
}

const STORAGE_FAILURE: HubError = {
  code: 'INTERNAL', message: 'This device could not save the recovery ID. Remote actions are paused.',
};

function savedId(key: string): HubResult<string | null> {
  try {
    return { ok: true, value: localStorage.getItem(key) };
  } catch {
    return { ok: false, error: STORAGE_FAILURE };
  }
}

function storeId(key: string, id: string | null): boolean {
  try {
    if (id === null) localStorage.removeItem(key);
    else localStorage.setItem(key, id);
    return true;
  } catch {
    return false;
  }
}

export function followReceipt<T>(read: (id: string) => Promise<HubResult<T>>, policy: Policy<T>, { scope, key }: Slot) {
  const previous = savedId(key);
  let request = $state.raw<Request<T> | null>(previous.ok && previous.value !== null ? { id: previous.value } : null);
  let receipt = $state.raw<T | null>(null);
  let busy = $state(false);
  let error = $state.raw<HubError | null>(previous.ok ? null : previous.error);
  // A later rejection says nothing about a write whose response was lost.
  let mayHaveSubmitted = previous.ok && previous.value !== null;
  let storageFailed = $state(!previous.ok);
  const completed = $derived(receipt !== null && !policy.waiting(receipt) && policy.acknowledge?.(receipt) === true);
  const pending = $derived(request !== null && !completed);

  function release(id: string): boolean {
    const prior = savedId(key);
    if (!prior.ok || (prior.value === id && !storeId(key, null))) {
      storageFailed = true;
      error = STORAGE_FAILURE;
      return false;
    }
    request = null;
    return true;
  }

  async function run(submit: boolean): Promise<void> {
    if (request === null || busy || completed) return;
    const current = request;
    if (submit && current.submit === undefined) return;
    busy = true;
    const authorized = await scope.authorize();
    if (!authorized.ok) {
      busy = false;
      error = authorized.error;
      if (!mayHaveSubmitted) release(current.id);
      return;
    }
    const result = await (submit && current.submit !== undefined ? current.submit() : read(current.id));
    busy = false;
    if (!result.ok) {
      error = result.error;
      if (submit && !mayHaveSubmitted && writeRefused(result.error)) release(current.id);
      else if (submit) mayHaveSubmitted = true;
      return;
    }
    error = null;
    receipt = result.value;
    mayHaveSubmitted = true;
    if (!policy.waiting(result.value) && policy.acknowledge?.(result.value) !== true) release(current.id);
  }

  return {
    get busy() { return busy; },
    get receipt() { return receipt; },
    get error() { return error; },
    get blocked() { return storageFailed || request !== null; },
    get pending() { return pending; },
    get completed() { return completed; },
    get canRetry() { return pending && request?.submit !== undefined && receipt === null && error !== null && !busy; },
    start(next: Required<Request<T>>) {
      if (request !== null || storageFailed) return;
      const prior = savedId(key);
      if (!prior.ok) {
        storageFailed = true;
        error = STORAGE_FAILURE;
        return;
      }
      receipt = null;
      error = null;
      if (prior.value !== null) {
        mayHaveSubmitted = true;
        request = { id: prior.value };
        return run(false);
      }
      if (!storeId(key, next.id)) {
        storageFailed = true;
        error = STORAGE_FAILURE;
        return;
      }
      mayHaveSubmitted = false;
      request = next;
      return run(true);
    },
    acknowledge() {
      if (!completed || request === null || !release(request.id)) return;
      receipt = null;
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
