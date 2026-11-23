import type { HubResult } from '$lib/hub/hub';
import type { AgentUsageView, UsageRequest } from './api';

export type UsageStatus = 'loading' | 'ready' | 'failed';

export class UsageStore {
  status = $state<UsageStatus>('loading');
  loadProblem = $state('');
  /** The last answer; it stays while the next one is read, so a change of filter does not blank the page. */
  view = $state.raw<AgentUsageView | null>(null);
  #generation = 0;

  constructor(private readonly read: (request: UsageRequest) => Promise<HubResult<AgentUsageView>>) {}

  /** An answer for an earlier request is dropped. */
  async load(request: UsageRequest): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    const result = await this.read(request);
    if (generation !== this.#generation) return;
    if (!result.ok) {
      this.loadProblem = result.error.message;
      this.status = 'failed';
      return;
    }
    this.view = result.value;
    this.loadProblem = '';
    this.status = 'ready';
  }
}
