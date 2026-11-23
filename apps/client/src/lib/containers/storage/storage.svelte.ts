import type { HubResult } from '$lib/hub/hub';
import type { ContainerCalls, ContainerNetwork, ContainerStorage } from '../api';
import type { ReadStatus } from '../store.svelte';

class Read<T> {
  status = $state<ReadStatus>('idle');
  problem = $state('');
  value = $state.raw<T | null>(null);
  #generation = 0;

  constructor(private readonly ask: () => Promise<HubResult<T>>) {}

  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    const result = await this.ask();
    if (generation !== this.#generation) return;
    if (!result.ok) {
      this.problem = result.error.message;
      this.status = 'failed';
      return;
    }
    this.value = result.value;
    this.problem = '';
    this.status = 'ready';
  }

  async open(): Promise<void> {
    if (this.status === 'idle') await this.load();
  }
}

/** HOST-020: the hub reads the device at each call and sizes are slow, so a list loads on first open and on request. */
export class StorageStore {
  readonly storage: Read<ContainerStorage>;
  readonly networks: Read<readonly ContainerNetwork[]>;

  constructor(calls: Pick<ContainerCalls, 'storage' | 'networks'>) {
    this.storage = new Read(() => calls.storage());
    this.networks = new Read(() => calls.networks());
  }
}
