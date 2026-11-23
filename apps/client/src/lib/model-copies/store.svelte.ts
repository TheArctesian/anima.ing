import type { HubResult } from '$lib/hub/hub';
import type { CopyCalls, FoundModel, ModelCopy, PullChoices } from './api';
import { withReasons } from './refusal';
import { isMoving } from './rows';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/**
 * The copies of the models on the machines (HSE-013; PULL-001, PULL-007, PULL-009): the list, the choice of one machine,
 * and the pulls and removals that the operator asks for. The hub derives each state from the work of the machine. Also
 * the models that the machines found where a different program keeps them, and their use there (PULL-012, R-444).
 */
export class CopiesStore {
  status = $state<ReadStatus>('idle');
  /** Why the list did not load, in the hub's words. */
  problem = $state('');
  copies = $state.raw<readonly ModelCopy[]>([]);
  /** The found models that no copy uses. */
  found = $state.raw<readonly FoundModel[]>([]);
  /** True from the request to find models until the page has read again for a while: the machines answer later. */
  finding = $state(false);
  /** Why the last Find models or Use did not complete; each later Pull, Remove, Find models or Use clears it. */
  refusal = $state('');
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
    const [result, found] = await Promise.all([this.calls.list(), this.calls.found()]);
    if (generation !== this.#generation) return;
    if (!result.ok || !found.ok) {
      this.problem = result.ok ? (found.ok ? '' : found.error.message) : result.error.message;
      this.status = 'failed';
      return;
    }
    this.copies = result.value;
    this.found = found.value;
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
    this.refusal = '';
    const result = withReasons(await this.calls.pull(deviceId, address));
    if (result.ok) {
      this.#generation += 1;
      this.copies = [...this.copies.filter((copy) => copy.id !== result.value.id), result.value];
      this.status = 'ready';
    }
    return result;
  }

  /** PULL-009: a copy that went at once leaves the list; else its row says Removing until the machine deleted it. */
  async remove(id: string): Promise<HubResult<ModelCopy | null>> {
    this.refusal = '';
    const result = withReasons(await this.calls.remove(id));
    if (result.ok || result.error.code === 'NOT_FOUND') {
      this.#generation += 1;
      const left = result.ok ? result.value : null;
      this.copies = left === null ? this.copies.filter((copy) => copy.id !== id) : this.copies.map((copy) => (copy.id === id ? left : copy));
    }
    return result;
  }

  /**
   * PULL-012: asks each machine that has asked this hub for work to read its found models again. Gives the first refusal
   * of the hub, else the number of machines asked; the page then reads again while `finding` is true.
   */
  async findAll(): Promise<HubResult<number>> {
    this.refusal = '';
    await this.loadChoices();
    if (this.choicesStatus === 'failed') {
      this.refusal = this.choicesProblem;
      return { ok: false, error: { code: 'unreachable', message: this.choicesProblem } };
    }
    const machines = this.choices.machines.filter((machine) => machine.stop !== 'NO_SWITCH');
    const results = await Promise.all(machines.map(async (machine) => withReasons(await this.calls.find(machine.id))));
    const refused = results.find((result) => !result.ok);
    if (refused !== undefined && !refused.ok) {
      this.refusal = refused.error.message;
      return refused;
    }
    if (machines.length === 0) this.refusal = 'No machine has asked this hub for work.';
    this.finding = machines.length > 0;
    return { ok: true, value: machines.length };
  }

  /** R-444: uses a found model where it is; its copy joins the list and the model leaves the found list. */
  async use(model: FoundModel): Promise<HubResult<ModelCopy>> {
    this.refusal = '';
    const result = withReasons(await this.calls.use(model.place.id, model.found));
    if (!result.ok) this.refusal = result.error.message;
    if (result.ok) {
      this.#generation += 1;
      this.copies = [...this.copies.filter((copy) => copy.id !== result.value.id), result.value];
      this.found = this.found.filter((each) => !(each.found === model.found && each.place.id === model.place.id));
      this.status = 'ready';
    }
    return result;
  }
}
