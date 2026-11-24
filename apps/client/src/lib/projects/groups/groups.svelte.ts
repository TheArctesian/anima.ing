import { created, deleted, nameProblem, put, renamed, storeClusters, storedClusters, type Clusters, type Group } from './clusters';

/** Each change is kept in the storage of this computer at once (PRO-059). */
export class ProjectGroups {
  #clusters = $state.raw<Clusters>({ groups: [], of: {} });
  readonly groups = $derived(this.#clusters.groups.toSorted((a, b) => a.name.localeCompare(b.name)));

  constructor(
    private readonly storage: Storage,
    private readonly newId: () => string = () => crypto.randomUUID(),
  ) {
    this.#clusters = storedClusters(storage);
  }

  of(projectId: string): Group | undefined {
    const id = this.#clusters.of[projectId];
    return id === undefined ? undefined : this.#clusters.groups.find((group) => group.id === id);
  }

  members(id: string): string[] {
    return Object.keys(this.#clusters.of).filter((project) => this.#clusters.of[project] === id);
  }

  /** The reason when the name cannot be used, else null (PRO-060). */
  create(name: string, projectIds: readonly string[]): string | null {
    return this.#change(nameProblem(this.#clusters, name), () => created(this.#clusters, this.newId(), name, projectIds));
  }

  rename(id: string, name: string): string | null {
    return this.#change(nameProblem(this.#clusters, name, id), () => renamed(this.#clusters, id, name));
  }

  /** `null` puts the projects in no group (PRO-060, PRO-063). */
  put(projectIds: readonly string[], groupId: string | null): void {
    this.#change(null, () => put(this.#clusters, projectIds, groupId));
  }

  delete(id: string): void {
    this.#change(null, () => deleted(this.#clusters, id));
  }

  #change(problem: string | null, next: () => Clusters): string | null {
    if (problem !== null) return problem;
    this.#clusters = next();
    storeClusters(this.storage, this.#clusters);
    return null;
  }
}
