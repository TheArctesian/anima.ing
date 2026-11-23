import type { HubResult } from '$lib/hub/hub';
import type { ApplicationRoute, RouteCalls } from './api';
import { filterCounts, filtered, isNotLinux, type RouteFilter } from './route';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/** The hub says that it cannot route only when it refuses a check (R-366); the section then shows that in place of the list. */
export class RoutesStore {
  status = $state<ReadStatus>('idle');
  problem = $state('');
  routes = $state.raw<readonly ApplicationRoute[]>([]);
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each read, never changed in place
  deviceNames = $state.raw<ReadonlyMap<string, string>>(new Map());
  filter = $state<RouteFilter>('ALL');
  selected = $state<string | null>(null);
  notLinux = $state(false);
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each check, never changed in place
  checking = $state.raw<ReadonlySet<string>>(new Set());
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each check, never changed in place
  checkProblems = $state.raw<ReadonlyMap<string, string>>(new Map());
  #generation = 0;

  /** `hosted`: the hub is hosted by Anima.ing, so it has no streams (OD-HOST-47). */
  constructor(
    private readonly calls: RouteCalls,
    readonly hosted: boolean,
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- the clock: read once at each call, never kept
    private readonly now: () => Date = () => new Date(),
  ) {}

  get shown(): ApplicationRoute | null {
    return this.routes.find((route) => route.id === this.selected) ?? null;
  }

  get visible(): ApplicationRoute[] {
    return filtered(this.routes, this.filter, this.now());
  }

  get counts(): Record<RouteFilter, number> {
    return filterCounts(this.routes, this.now());
  }

  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    const result = await this.calls.list();
    if (generation !== this.#generation) return;
    if (!result.ok) {
      this.problem = result.error.message;
      this.status = 'failed';
      return;
    }
    this.routes = result.value.routes;
    this.deviceNames = result.value.deviceNames;
    this.problem = '';
    this.status = 'ready';
  }

  async open(): Promise<void> {
    if (this.status === 'idle') await this.load();
  }

  select(id: string | null): void {
    this.selected = id;
  }

  async check(id: string): Promise<void> {
    if (this.checking.has(id)) return;
    this.checking = new Set([...this.checking, id]); // eslint-disable-line svelte/prefer-svelte-reactivity -- replaced whole, never changed in place
    const result = await this.calls.check(id);
    this.checking = new Set([...this.checking].filter((each) => each !== id)); // eslint-disable-line svelte/prefer-svelte-reactivity -- replaced whole, never changed in place
    const problems = [...this.checkProblems].filter(([each]) => each !== id);
    if (result.ok) this.routes = this.routes.map((route) => (route.id === id ? result.value : route));
    else if (isNotLinux(result.error)) this.notLinux = true;
    else problems.push([id, result.error.message]);
    this.checkProblems = new Map(problems); // eslint-disable-line svelte/prefer-svelte-reactivity -- replaced whole, never changed in place
  }

  async checkAll(): Promise<void> {
    for (const route of this.routes) {
      if (this.notLinux) return;
      await this.check(route.id);
    }
  }

  async remove(id: string): Promise<HubResult<string>> {
    const result = await this.calls.remove(id);
    if (result.ok || result.error.code === 'NOT_FOUND') {
      this.routes = this.routes.filter((route) => route.id !== id);
      if (this.selected === id) this.selected = null;
    }
    return result;
  }
}
