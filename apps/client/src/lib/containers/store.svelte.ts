import type { ContainerCalls, ContainerDetail, ContainerLog } from './api';
import { DEFAULT_LINES, type LineChoice } from './rows';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/** What the pane below the list shows of the selected container. */
export type Shown = 'log' | 'detail';

/**
 * The log or the details of the one container of a machine that the operator selected (HOST-001, HOST-017). The hub
 * reads the machine at each call, so nothing here reads again by itself: the operator asks.
 */
export class ContainersStore {
  /** The identifier of the container whose log or details are shown; null when none is. */
  selected = $state<string | null>(null);
  shown = $state<Shown>('log');
  lines = $state<LineChoice>(DEFAULT_LINES);
  logStatus = $state<ReadStatus>('idle');
  logProblem = $state('');
  log = $state.raw<ContainerLog | null>(null);

  /** HOST-017: the details of the selected container. */
  detailStatus = $state<ReadStatus>('idle');
  detailProblem = $state('');
  detail = $state.raw<ContainerDetail | null>(null);

  #detailGeneration = 0;
  #logGeneration = 0;

  constructor(private readonly calls: Pick<ContainerCalls, 'log' | 'detail'>) {}

  /** Shows the log of a container and reads it. */
  async select(id: string): Promise<void> {
    this.selected = id;
    this.shown = 'log';
    this.log = null;
    await this.readLog();
  }

  /** Shows the details of a container and reads them. */
  async inspect(id: string): Promise<void> {
    this.selected = id;
    this.shown = 'detail';
    this.detail = null;
    await this.readDetail();
  }

  /** Reads the details of the selected container again. */
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

  /** Changes how many lines the log shows and reads it again. */
  async setLines(lines: LineChoice): Promise<void> {
    this.lines = lines;
    if (this.selected !== null) await this.readLog();
  }

  /** Reads the log of the selected container again. */
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

  /** Closes the log or the details. They are dropped: the client keeps no log and no details of a container. */
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
