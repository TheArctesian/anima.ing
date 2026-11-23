import { needsNewerHub, type Hub, type HubResult } from '$lib/hub/hub';
import { NO_CLUSTERS, type Clusters, type Group } from './clusters';

import { CREATE, DELETE, GROUPS, MOVE, RENAME } from './api';

export class SharedProjectGroups {
  #clusters = $state.raw<Clusters>(NO_CLUSTERS);
  #request = 0;
  busy = $state(false);
  problem = $state('');
  ready = $state(false);
  /** True when the hub is older than project groups (PRO-059). */
  unserved = $state(false);
  readonly groups = $derived(this.#clusters.groups);

  constructor(private readonly hub: Pick<Hub, 'request'>) {}

  of(projectId: string): Group | undefined {
    const id = this.#clusters.of[projectId];
    return this.groups.find((group) => group.id === id);
  }

  async refresh(): Promise<void> {
    const request = ++this.#request;
    const result = await this.hub.request(GROUPS, {});
    if (request !== this.#request) return;
    if (!result.ok) {
      this.unserved = needsNewerHub(result.error);
      this.problem = this.unserved ? '' : result.error.message;
      return;
    }
    const groups = result.value.projectGroups;
    this.#clusters = {
      groups: groups.map(({ id, name }) => ({ id, name })),
      of: Object.fromEntries(groups.flatMap(({ id, projectIds }) => projectIds.map((project) => [project, id]))),
    };
    this.ready = true;
    this.problem = '';
  }

  async create(name: string, projectIds: readonly string[]): Promise<string | null> {
    const result = await this.#change(() => this.hub.request(CREATE, { name, projects: [...projectIds] }));
    return result.ok ? null : result.error.message;
  }

  async rename(id: string, name: string): Promise<string | null> {
    const result = await this.#change(() => this.hub.request(RENAME, { id, name }));
    return result.ok ? null : result.error.message;
  }

  async put(projectIds: readonly string[], group: string | null): Promise<string | null> {
    const result = await this.#change(() => this.hub.request(MOVE, { projects: [...projectIds], group }));
    return result.ok ? null : result.error.message;
  }

  delete(id: string): Promise<HubResult<null>> {
    return this.#change(() => this.hub.request(DELETE, { id }));
  }

  async #change(send: () => Promise<HubResult<unknown>>): Promise<HubResult<null>> {
    if (this.busy) return { ok: false, error: { code: 'CONFLICT', message: 'A group change is already in progress.' } };
    this.busy = true;
    try {
      const result = await send();
      if (!result.ok) return result;
      await this.refresh();
      return { ok: true, value: null };
    } finally {
      this.busy = false;
    }
  }
}
