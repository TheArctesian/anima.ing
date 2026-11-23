import { SvelteSet } from 'svelte/reactivity';
import type { Project, ProjectBrowse, ProjectResource, ProjectsApi } from './api';
import { isLeftOut } from './words';

export type ProjectsStatus = 'loading' | 'ready' | 'failed';

export interface Refusal {
  readonly message: string;
}

type View = 'main' | 'leftOut';

/** PRO-001: a change shows at once over the hub's last answer; a refused one goes back and sets `refusal`. */
export class ProjectsStore {
  status = $state<ProjectsStatus>('loading');
  loadProblem = $state('');
  refusal = $state.raw<Refusal | null>(null);
  /** `undefined` while unread, `null` when the hub has none. */
  found = $state.raw<Project | null | undefined>(undefined);
  #generation = 0;
  #browse = $state.raw<ProjectBrowse>({});
  #known = $state.raw<readonly Project[]>([]);
  #next = $state.raw<Readonly<Record<View, string | null>>>({ main: null, leftOut: null });
  #leaving = $state.raw<Readonly<Record<string, boolean>>>({});
  #confirming = $state.raw<readonly string[]>([]);

  languages = $state.raw<readonly string[]>([]);
  readonly browse = $derived(this.#browse);
  readonly byName = $derived(this.#browse.order == null || this.#browse.order === 'NAME');
  readonly filtered = $derived(this.#browse.language != null || this.#browse.minSizeKib != null || this.#browse.maxSizeKib != null);

  /** Keep the hub's order for metadata sorts, with local changes laid over it. */
  readonly shown = $derived.by(() => {
    const projects = this.#known.map((project) => this.#overlaid(project));
    return this.byName ? projects.toSorted((a, b) => a.name.localeCompare(b.name)) : projects;
  });
  readonly main = $derived(this.shown.filter((project) => !isLeftOut(project)));
  readonly leftOut = $derived(this.shown.filter((project) => isLeftOut(project)));
  readonly current = $derived(this.found == null ? this.found : this.#overlaid(this.#latest(this.found)));

  constructor(private readonly api: ProjectsApi) {}

  hasMore(view: View): boolean {
    return this.#next[view] !== null;
  }

  async setBrowse(change: ProjectBrowse): Promise<void> {
    this.#browse = { ...this.#browse, ...change };
    await this.load();
  }

  async load(): Promise<void> {
    const generation = ++this.#generation;
    const browse = this.#browse;
    this.status = 'loading';
    const [main, left] = await Promise.all([this.api.list(false, null, browse), this.api.list(true, null, browse)]);
    if (generation !== this.#generation) return;
    if (!main.ok || !left.ok) {
      this.loadProblem = !main.ok ? main.error.message : left.ok ? '' : left.error.message;
      this.status = 'failed';
      return;
    }
    this.#known = [];
    this.#merge(main.value.projects);
    this.#merge(left.value.projects);
    this.#next = { main: main.value.next, leftOut: left.value.next };
    this.languages = [...new SvelteSet([...(main.value.languages ?? []), ...(left.value.languages ?? [])])].sort((a, b) => a.localeCompare(b));
    this.loadProblem = '';
    this.status = 'ready';
  }

  async more(view: View): Promise<void> {
    const after = this.#next[view];
    if (after === null) return;
    const generation = this.#generation;
    const result = await this.api.list(view === 'leftOut', after, this.#browse);
    if (generation !== this.#generation || after !== this.#next[view]) return;
    if (!result.ok) {
      this.refusal = { message: result.error.message };
      return;
    }
    this.#merge(result.value.projects);
    this.#next = { ...this.#next, [view]: result.value.next };
  }

  async open(slug: string): Promise<void> {
    this.status = 'loading';
    const result = await this.api.find(slug);
    if (!result.ok) {
      this.loadProblem = result.error.message;
      this.status = 'failed';
      return;
    }
    this.found = result.value;
    if (result.value !== null) this.#merge([result.value]);
    this.loadProblem = '';
    this.status = 'ready';
  }

  /** Leaves out a repository (PRO-002); the entry goes at once. */
  leaveOut(repositoryId: string): Promise<void> {
    return this.#setLeftOut(repositoryId, true);
  }

  restore(repositoryId: string): Promise<void> {
    return this.#setLeftOut(repositoryId, false);
  }

  /** Confirms a proposed resource (PRO-004); it shows as confirmed at once. */
  async confirm(resourceId: string): Promise<void> {
    this.refusal = null;
    this.#confirming = [...this.#confirming, resourceId];
    const result = await this.api.confirm(resourceId);
    if (result.ok) this.#replaceResource(result.value);
    else this.refusal = { message: result.error.message };
    this.#confirming = this.#confirming.filter((each) => each !== resourceId);
  }

  async #setLeftOut(repositoryId: string, leftOut: boolean): Promise<void> {
    this.refusal = null;
    this.#leaving = { ...this.#leaving, [repositoryId]: leftOut };
    const result = await (leftOut ? this.api.leaveOut(repositoryId) : this.api.restore(repositoryId));
    if (result.ok) {
      this.#merge([result.value]);
      if (this.filtered || !this.byName) await this.load();
    } else this.refusal = { message: result.error.message };
    this.#leaving = Object.fromEntries(Object.entries(this.#leaving).filter(([id]) => id !== repositoryId));
  }

  #latest(project: Project): Project {
    return this.#known.find((each) => each.id === project.id) ?? project;
  }

  #merge(projects: readonly Project[]): void {
    const ids = projects.map((project) => project.id);
    this.#known = [...this.#known.filter((each) => !ids.includes(each.id)), ...projects];
  }

  #replaceResource(resource: ProjectResource): void {
    this.#known = this.#known.map((project) => ({
      ...project,
      resources: project.resources.map((each) => (each.id === resource.id ? resource : each)),
    }));
  }

  #overlaid(project: Project): Project {
    return {
      ...project,
      repositories: project.repositories.map((repository) => ({ ...repository, leftOut: this.#leaving[repository.id] ?? repository.leftOut })),
      resources: project.resources.map((resource) => (this.#confirming.includes(resource.id) ? { ...resource, state: 'CONFIRMED' } : resource)),
    };
  }
}
