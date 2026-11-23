import type { HubResult } from '$lib/hub/hub';
import type { CopyCalls, ModelCopy, PullChoices } from './api';
import { isMoving } from './rows';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/**
 * The copies of the models on the machines (HSE-013; PULL-001, PULL-007, PULL-009): the list, the choice of one machine,
 * and the pulls and removals that the operator asks for. The hub derives each state from the work of the machine.
 */
export class CopiesStore {
  status = $state<ReadStatus>('idle');
  /** Why the list did not load, in the hub's words. */
  problem = $state('');
  copies = $state.raw<readonly ModelCopy[]>([]);
  /** The identifier of one machine, or empty for all. */
  machine = $state('');

  choicesStatus = $state<ReadStatus>('idle');
  choicesProblem = $state('');
  choices = $state.raw<PullChoices>({ machines: [], variants: [] });

  #generation = 0;

  constructor(private readonly calls: CopyCalls) {}

  /** True while a copy waits, downloads, is checked or is removed: the page then reads again (PULL-007). */
  get moving(): boolean {
    return isMoving(this.copies);
  }

  /** Reads the list; an answer to an earlier read is dropped. `quiet` keeps the rows on screen while it reads. */
  async load(quiet = false): Promise<void> {
    const generation = ++this.#generation;
    if (!quiet) this.status = 'loading';
    const result = await this.calls.list();
    if (generation !== this.#generation) return;
    if (!result.ok) {
      this.problem = result.error.message;
      this.status = 'failed';
      return;
    }
    this.copies = result.value;
    this.problem = '';
    this.status = 'ready';
    if (this.machine !== '' && !this.copies.some((copy) => copy.place.id === this.machine)) this.machine = '';
  }

  /** Reads the machines and the variants that a pull can name. */
  async loadChoices(): Promise<void> {
    this.choicesStatus = 'loading';
    const result = await this.calls.choices();
    if (!result.ok) {
      this.choicesProblem = result.error.message;
      this.choicesStatus = 'failed';
      return;
    }
    this.choices = result.value;
    this.choicesProblem = '';
    this.choicesStatus = 'ready';
  }

  /** PULL-001: asks the machine for the variant; the new copy joins the list. The hub's refusal is the answer. */
  async pull(deviceId: string, address: string): Promise<HubResult<ModelCopy>> {
    const result = await this.calls.pull(deviceId, address);
    if (result.ok) {
      this.#generation += 1;
      this.copies = [...this.copies.filter((copy) => copy.id !== result.value.id), result.value];
      this.status = 'ready';
    }
    return result;
  }

  /** PULL-009: a copy that went at once leaves the list; else its row says Removing until the machine deleted it. */
  async remove(id: string): Promise<HubResult<ModelCopy | null>> {
    const result = await this.calls.remove(id);
    if (result.ok || result.error.code === 'NOT_FOUND') {
      this.#generation += 1;
      const left = result.ok ? result.value : null;
      this.copies = left === null ? this.copies.filter((copy) => copy.id !== id) : this.copies.map((copy) => (copy.id === id ? left : copy));
    }
    return result;
  }
}
