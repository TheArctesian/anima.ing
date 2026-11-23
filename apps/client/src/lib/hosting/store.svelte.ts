import type { HostingCalls } from './api';
import type { Place, Show } from './list';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/**
 * The machines of the hub and the containers of each (HSE-002, task H42). Each machine is read on its own, so a machine
 * that does not answer does not hold the others back. Nothing reads again by itself: the operator asks.
 */
export class HostingStore {
  status = $state<ReadStatus>('idle');
  /** Why the machines did not load, in the hub's words. */
  problem = $state('');
  places = $state.raw<readonly Place[]>([]);

  search = $state('');
  machine = $state('');
  show = $state<Show>('all');
  /** The keys of the applications that are open to their containers. */
  open = $state.raw<readonly string[]>([]);

  /** HOST-021: idle until the operator asks; the hub then asks public registries for each machine. */
  updatesStatus = $state<ReadStatus>('idle');

  #generation = 0;

  constructor(private readonly calls: HostingCalls) {}

  /** Reads the machines, then the containers of each at the same time. An answer to an earlier read is dropped. */
  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    this.updatesStatus = 'idle';
    const machines = await this.calls.machines();
    if (generation !== this.#generation) return;
    if (!machines.ok) {
      this.problem = machines.error.message;
      this.status = 'failed';
      return;
    }
    this.problem = '';
    this.status = 'ready';
    this.places = machines.value.map((machine) => ({ ...machine, status: 'loading', problem: '', containers: [], updates: [] }));
    if (this.machine !== '' && !machines.value.some((machine) => machine.id === this.machine)) this.machine = '';
    await Promise.all(machines.value.map((machine) => this.#readContainers(machine.id, generation)));
  }

  /** HOST-021: asks the hub for newer images on each machine that answered. Anima updates nothing. */
  async checkUpdates(): Promise<void> {
    const generation = this.#generation;
    this.updatesStatus = 'loading';
    const ready = this.places.filter((place) => place.status === 'ready');
    const answers = await Promise.all(ready.map(async (place) => ({ id: place.id, result: await this.calls.updates(place.id) })));
    if (generation !== this.#generation) return;
    for (const { id, result } of answers) if (result.ok) this.#change(id, { updates: result.value });
    this.updatesStatus = answers.every(({ result }) => result.ok) ? 'ready' : 'failed';
  }

  /** Opens an application to its containers, or closes it. */
  toggle(key: string): void {
    this.open = this.open.includes(key) ? this.open.filter((each) => each !== key) : [...this.open, key];
  }

  async #readContainers(id: string, generation: number): Promise<void> {
    const result = await this.calls.containers(id);
    if (generation !== this.#generation) return;
    this.#change(id, result.ok ? { status: 'ready', containers: result.value } : { status: 'failed', problem: result.error.message });
  }

  #change(id: string, change: Partial<Place>): void {
    this.places = this.places.map((place) => (place.id === id ? { ...place, ...change } : place));
  }
}
