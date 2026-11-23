import { needsNewerHub, type Hub, type HubResult } from '$lib/hub/hub';
import { NO_CLUSTERS, storedClusters, storeClusters, type Clusters, type Group } from './clusters';

import { CREATE, DELETE, GROUPS, IMPORT, MOVE, RENAME } from './api';

export class SharedProjectGroups {
  #clusters = $state.raw<Clusters>(NO_CLUSTERS);
  busy = $state(false);
  problem = $state('');
  ready = $state(false);
  /** True when the hub is older than project groups (PRO-059). */
  unserved = $state(false);
  readonly groups = $derived(this.#clusters.groups);

  constructor(private readonly hub: Pick<Hub, 'request'>, private readonly storage?: Storage) {}

  of(projectId: string): Group | undefined {
    const id = this.#clusters.of[projectId];
    return this.groups.find((group) => group.id === id);
  }

  async refresh(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      if (!(await this.#read())) return;
      const imported = await this.#importDeviceGroups();
      const migrationProblem = this.problem;
      if (imported && await this.#read()) this.problem = migrationProblem;
    } finally {
      this.busy = false;
    }
  }

  async #read(): Promise<boolean> {
    const result = await this.hub.request(GROUPS, {});
    if (!result.ok) {
      this.unserved = needsNewerHub(result.error);
      this.problem = this.unserved ? '' : result.error.message;
      return false;
    }
    const groups = result.value.projectGroups;
    this.#clusters = {
      groups: groups.map(({ id, name }) => ({ id, name })),
      of: Object.fromEntries(groups.flatMap(({ id, projectIds }) => projectIds.map((project) => [project, id]))),
    };
    this.ready = true;
    this.unserved = false;
    this.problem = '';
    return true;
  }

  async #importDeviceGroups(): Promise<boolean> {
    if (this.storage === undefined) return false;
    const local = storedClusters(this.storage);
    let imported = false;
    for (const group of local.groups) {
      const members = Object.keys(local.of).filter((id) => local.of[id] === group.id);
      for (let offset = 0; offset < Math.max(1, members.length); offset += 200) {
        const projects = members.slice(offset, offset + 200);
        const empty = members.length === 0;
        const input = { source: group.id, name: group.name, projects, ...(empty ? { includeEmpty: true } : {}) };
        const result = await this.hub.request(IMPORT, { input });
        if (!result.ok) {
          this.problem ||= needsNewerHub(result.error)
            ? 'Sharing earlier groups needs a newer hub. Your device groups are kept.'
            : result.error.message;
          if (result.error.code === 'VALIDATION') break;
          return imported;
        }
        this.#acknowledge(group, result.value.importProjectGroup, empty);
        imported ||= empty || result.value.importProjectGroup.length > 0;
      }
    }
    return imported;
  }

  #acknowledge(group: Group, projects: readonly string[], empty: boolean): void {
    if (this.storage === undefined || (!empty && projects.length === 0)) return;
    // Read again after the network wait: another tab may have edited the local groups.
    const current = storedClusters(this.storage);
    if (!current.groups.some((each) => each.id === group.id && each.name === group.name)) return;
    const of = Object.fromEntries(Object.entries(current.of).filter(([id, owner]) => owner !== group.id || !projects.includes(id)));
    const groups = Object.values(of).includes(group.id) ? current.groups : current.groups.filter((each) => each.id !== group.id);
    storeClusters(this.storage, { groups, of });
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
      await this.#read();
      return { ok: true, value: null };
    } finally {
      this.busy = false;
    }
  }
}
