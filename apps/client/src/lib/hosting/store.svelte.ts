import type { ContainerActionCalls } from '$lib/containers/action/api';
import { ContainersStore } from '$lib/containers/store.svelte';
import type { HostingCalls, Machine } from './api';
import type { Place, Show, View } from './list';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/** HSE-002: each machine is read on its own, so one that does not answer holds no other back; nothing reads again unless the operator asks. */
export class HostingStore {
  status = $state<ReadStatus>('idle');
  problem = $state('');
  places = $state.raw<readonly Place[]>([]);

  search = $state('');
  machine = $state('');
  show = $state<Show>('all');
  /** HOST-034: the applications, or each container of each machine. */
  view = $state<View>('applications');
  open = $state.raw<readonly string[]>([]);

  /** HSE-008: the one machine of a device page; the list then reads no other machine. Null on the page Hosting. */
  only = $state.raw<Machine | null>(null);

  /** HOST-001 and HOST-017: the container whose log or details are open below the list, and the store that reads them. */
  pane = $state.raw<{ readonly machineId: string; readonly containerId: string; readonly name: string; readonly store: ContainersStore } | null>(null);

  /** HOST-021: idle until the operator asks; the hub then asks public registries for each machine. */
  updatesStatus = $state<ReadStatus>('idle');
  updatesProblem = $state('');

  #generation = 0;

  constructor(private readonly calls: HostingCalls) {}

  get actions(): ContainerActionCalls {
    return this.calls.actions;
  }

  get apply(): HostingCalls['apply'] {
    return this.calls.apply;
  }

  applied(id: string): void {
    if (this.places.some((place) => place.id === id)) void this.refreshMachine(id);
  }

  async refreshMachine(id: string): Promise<void> {
    await this.#readContainers(id, this.#generation);
  }

  /** Reads the machines, then the containers of each at the same time. An answer to an earlier read is dropped. */
  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    this.updatesStatus = 'idle';
    const machines = this.only === null ? await this.calls.machines() : { ok: true as const, value: [this.only] };
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
    // An old answer beside a failure would read as the answer of this try, so a machine that failed loses its marks.
    for (const { id, result } of answers) this.#change(id, { updates: result.ok ? result.value : [] });
    const failed = answers.find(({ result }) => !result.ok)?.result;
    this.updatesProblem = failed !== undefined && !failed.ok ? failed.error.message : '';
    this.updatesStatus = failed === undefined ? 'ready' : 'failed';
  }

  toggle(key: string): void {
    this.open = this.open.includes(key) ? this.open.filter((each) => each !== key) : [...this.open, key];
  }

  async openLog(machineId: string, containerId: string, name: string): Promise<void> {
    await this.#open(machineId, containerId, name).select(containerId);
  }

  async openDetail(machineId: string, containerId: string, name: string): Promise<void> {
    await this.#open(machineId, containerId, name).inspect(containerId);
  }

  #open(machineId: string, containerId: string, name: string): ContainersStore {
    const same = this.pane !== null && this.pane.machineId === machineId;
    const store = same && this.pane !== null ? this.pane.store : new ContainersStore(this.calls.machine(machineId));
    if (!same) this.pane?.store.close();
    this.pane = { machineId, containerId, name, store };
    return store;
  }

  async #readContainers(id: string, generation: number): Promise<void> {
    const result = await this.calls.containers(id);
    if (generation !== this.#generation) return;
    this.#change(id, result.ok ? { status: 'ready', containers: result.value } : { status: 'failed', problem: result.error.message });
    const pane = this.pane;
    if (pane !== null && pane.machineId === id && (!result.ok || !result.value.some((container) => container.id === pane.containerId))) {
      pane.store.close();
      this.pane = null;
    }
  }

  #change(id: string, change: Partial<Place>): void {
    this.places = this.places.map((place) => (place.id === id ? { ...place, ...change } : place));
  }
}
