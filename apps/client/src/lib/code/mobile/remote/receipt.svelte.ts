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

class Receipt<T> {
  #request = $state.raw<Request<T> | null>(null);
  #receipt = $state.raw<T | null>(null);
  #busy = $state(false);
  #error = $state.raw<HubError | null>(null);
  #storageFailed = $state(false);
  #mayHaveSubmitted = false;

  constructor(
    private readonly read: (id: string) => Promise<HubResult<T>>,
    private readonly policy: Policy<T>,
    private readonly slot: Slot,
  ) {
    const previous = savedId(slot.key);
    if (!previous.ok) this.#failStorage();
    else if (previous.value !== null) this.#restore(previous.value);
  }

  get busy(): boolean { return this.#busy; }
  get receipt(): T | null { return this.#receipt; }
  get error(): HubError | null { return this.#error; }
  get blocked(): boolean { return this.#storageFailed || this.#request !== null; }
  get pending(): boolean { return this.#request !== null && !this.completed; }

  get completed(): boolean {
    return this.#receipt !== null && !this.policy.waiting(this.#receipt) && this.policy.acknowledge?.(this.#receipt) === true;
  }

  get canRetry(): boolean {
    return this.pending && this.#request?.submit !== undefined && this.#receipt === null && this.#error !== null && !this.#busy;
  }

  #restore(id: string): void {
    this.#request = { id };
    this.#mayHaveSubmitted = true;
  }

  #failStorage(): void {
    this.#storageFailed = true;
    this.#error = STORAGE_FAILURE;
  }

  #release(id: string): boolean {
    const prior = savedId(this.slot.key);
    if (!prior.ok || (prior.value === id && !storeId(this.slot.key, null))) {
      this.#failStorage();
      return false;
    }
    this.#request = null;
    return true;
  }

  #finished(id: string, result: HubResult<T>, submit: boolean): void {
    if (!result.ok) {
      this.#error = result.error;
      // A later rejection says nothing about a write whose response was lost.
      if (submit && !this.#mayHaveSubmitted && writeRefused(result.error)) this.#release(id);
      else if (submit) this.#mayHaveSubmitted = true;
      return;
    }
    this.#error = null;
    this.#receipt = result.value;
    this.#mayHaveSubmitted = true;
    if (!this.policy.waiting(result.value) && this.policy.acknowledge?.(result.value) !== true) this.#release(id);
  }

  async #run(submit: boolean): Promise<void> {
    const request = this.#request;
    if (request === null || this.#busy || this.completed) return;
    const operation = submit ? request.submit : () => this.read(request.id);
    if (operation === undefined) return;
    this.#busy = true;
    const authorized = await this.slot.scope.authorize();
    if (authorized.ok) this.#finished(request.id, await operation(), submit);
    else {
      this.#error = authorized.error;
      if (!this.#mayHaveSubmitted) this.#release(request.id);
    }
    this.#busy = false;
  }

  start(next: Required<Request<T>>): Promise<void> | undefined {
    if (this.blocked) return;
    const prior = savedId(this.slot.key);
    if (!prior.ok) {
      this.#failStorage();
      return;
    }
    this.#receipt = null;
    this.#error = null;
    if (prior.value !== null) {
      this.#restore(prior.value);
      return this.#run(false);
    }
    if (!storeId(this.slot.key, next.id)) {
      this.#failStorage();
      return;
    }
    this.#mayHaveSubmitted = false;
    this.#request = next;
    return this.#run(true);
  }

  acknowledge(): void {
    if (this.completed && this.#request !== null && this.#release(this.#request.id)) this.#receipt = null;
  }

  retry(): Promise<void> { return this.#run(true); }
  refresh(): Promise<void> { return this.#run(false); }
}

export function followReceipt<T>(read: (id: string) => Promise<HubResult<T>>, policy: Policy<T>, slot: Slot) {
  return new Receipt(read, policy, slot);
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
