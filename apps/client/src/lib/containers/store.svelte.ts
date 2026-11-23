import type { ContainerCalls, ContainerDetail, ContainerLog } from './api';
import { DEFAULT_LINES, type LineChoice } from './rows';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

export type Shown = 'log' | 'detail';

/** The hub reads the machine at each call, so nothing here reads again by itself (HOST-001, HOST-017). */
export class ContainersStore {
  selected = $state<string | null>(null);
  shown = $state<Shown>('log');
  lines = $state<LineChoice>(DEFAULT_LINES);
  logStatus = $state<ReadStatus>('idle');
  logProblem = $state('');
  log = $state.raw<ContainerLog | null>(null);

  detailStatus = $state<ReadStatus>('idle');
  detailProblem = $state('');
  detail = $state.raw<ContainerDetail | null>(null);

  #detailGeneration = 0;
  #logGeneration = 0;

  constructor(private readonly calls: Pick<ContainerCalls, 'log' | 'detail'>) {}

  async select(id: string): Promise<void> {
    this.selected = id;
    this.shown = 'log';
    this.log = null;
    await this.readLog();
  }

  async inspect(id: string): Promise<void> {
    this.selected = id;
    this.shown = 'detail';
    this.detail = null;
    await this.readDetail();
  }

  async readDetail(): Promise<void> {
    const id = this.selected;
    if (id === null) return;
    const generation = ++this.#detailGeneration;
    this.detailStatus = 'loading';
    const result = await this.calls.detail(id);
    if (generation !== this.#detailGeneration) return;
    if (!result.ok) {
      this.detailProblem = result.error.message;
      this.detailStatus = 'failed';
      return;
    }
    this.detail = result.value;
    this.detailProblem = '';
    this.detailStatus = 'ready';
  }

  async setLines(lines: LineChoice): Promise<void> {
    this.lines = lines;
    if (this.selected !== null) await this.readLog();
  }

  async readLog(): Promise<void> {
    const id = this.selected;
    if (id === null) return;
    const generation = ++this.#logGeneration;
    this.logStatus = 'loading';
    const result = await this.calls.log(id, Number(this.lines));
    if (generation !== this.#logGeneration) return;
    if (!result.ok) {
      this.logProblem = result.error.message;
      this.logStatus = 'failed';
      return;
    }
    this.log = result.value;
    this.logProblem = '';
    this.logStatus = 'ready';
  }

  /** The client keeps no log and no details of a container once it is closed. */
  close(): void {
    this.#logGeneration += 1;
    this.#detailGeneration += 1;
    this.detail = null;
    this.detailStatus = 'idle';
    this.detailProblem = '';
    this.selected = null;
    this.log = null;
    this.logStatus = 'idle';
    this.logProblem = '';
  }
}
