import type { HostingCalls } from '../api';
import type { ReadStatus } from '../store.svelte';
import type { VolumePlace } from './volumes';

/**
 * The volumes of each machine of the hub (HSE-015), read only. Each machine is read on its own, so a machine that does
 * not answer does not hold the others back. The read of the sizes is slow, so nothing reads again by itself.
 */
export class VolumeStore {
  status = $state<ReadStatus>('idle');
  /** Why the machines did not load, in the hub's words. */
  problem = $state('');
  places = $state.raw<readonly VolumePlace[]>([]);
  search = $state('');

  #generation = 0;

  constructor(private readonly calls: Pick<HostingCalls, 'machines' | 'containers' | 'storage'>) {}

  /** Reads the machines, then the volumes and the containers of each at the same time. An answer to an earlier read is dropped. */
  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    const machines = await this.calls.machines();
    if (generation !== this.#generation) return;
    if (!machines.ok) {
      this.problem = machines.error.message;
      this.status = 'failed';
      return;
    }
    this.problem = '';
    this.status = 'ready';
    this.places = machines.value.map((machine) => ({ ...machine, status: 'loading', problem: '', volumes: [], usage: [], containers: [] }));
    await Promise.all(machines.value.map((machine) => this.#read(machine.id, generation)));
  }

  async #read(id: string, generation: number): Promise<void> {
    const [storage, containers] = await Promise.all([this.calls.storage(id), this.calls.containers(id)]);
    if (generation !== this.#generation) return;
    // Without the containers a volume names the containers that use it, not their application.
    const change: Partial<VolumePlace> = storage.ok
      ? { status: 'ready', volumes: storage.value.volumes, usage: storage.value.usage, containers: containers.ok ? containers.value : [] }
      : { status: 'failed', problem: storage.error.message };
    this.places = this.places.map((place) => (place.id === id ? { ...place, ...change } : place));
  }
}
