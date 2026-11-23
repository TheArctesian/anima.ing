import type { HostingCalls, Machine } from '../api';
import type { ReadStatus } from '../store.svelte';

/**
 * The places where things can run (HSE-016), stage 1: the devices of the hub with SSH on, whose Docker the hub can read.
 * Nothing reads again by itself: the operator asks.
 */
export class MachineStore {
  status = $state<ReadStatus>('idle');
  /** Why the machines did not load, in the hub's words. */
  problem = $state('');
  machines = $state.raw<readonly Machine[]>([]);

  #generation = 0;

  constructor(private readonly calls: Pick<HostingCalls, 'machines'>) {}

  /** Reads the machines; an answer to an earlier read is dropped. The last list stays on screen while the next one loads. */
  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    const result = await this.calls.machines();
    if (generation !== this.#generation) return;
    if (!result.ok) {
      this.problem = result.error.message;
      this.status = 'failed';
      return;
    }
    this.machines = [...result.value].sort((a, b) => a.name.localeCompare(b.name));
    this.problem = '';
    this.status = 'ready';
  }
}
