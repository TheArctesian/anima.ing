import type { HubResult } from '$lib/hub/hub';
import type { ApplicationRoute, RouteCalls } from './api';
import { filterCounts, filtered, isNotLinux, type RouteFilter } from './route';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/**
 * The routes of the hub (task H21): the list, the route that is open, and the checks and removals that the operator
 * asks for. The hub says that it cannot route only when it refuses a check (R-366); from then on the section shows
 * that sentence in place of the list.
 */
export class RoutesStore {
  status = $state<ReadStatus>('idle');
  /** Why the list did not load, in the hub's words. */
  problem = $state('');
  routes = $state.raw<readonly ApplicationRoute[]>([]);
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each read, never changed in place
  deviceNames = $state.raw<ReadonlyMap<string, string>>(new Map());
  filter = $state<RouteFilter>('ALL');
  /** The ID of the route whose page is open; null for the list. */
  selected = $state<string | null>(null);
  /** True after the hub refused a check with "Routing needs a hub on Linux." */
  notLinux = $state(false);
  /** The IDs of the routes that the hub checks now. */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each check, never changed in place
  checking = $state.raw<ReadonlySet<string>>(new Set());
  /** Why the last check of a route failed, by the ID of the route. */
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

  /** The route whose page is open, if it is still in the list. */
  get shown(): ApplicationRoute | null {
    return this.routes.find((route) => route.id === this.selected) ?? null;
  }

  /** The routes that the filter keeps, in the order of their names. */
  get visible(): ApplicationRoute[] {
    return filtered(this.routes, this.filter, this.now());
  }

  get counts(): Record<RouteFilter, number> {
    return filterCounts(this.routes, this.now());
  }

  /** Reads the list; an answer to an earlier read is dropped. */
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

  /** Reads when nothing was read yet. */
  async open(): Promise<void> {
    if (this.status === 'idle') await this.load();
  }

  select(id: string | null): void {
    this.selected = id;
  }

  /** Asks the hub to read the proof of one name once, and keeps the state that it answers. */
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

  /** Checks each route, one after the other; stops when the hub says that it cannot route. */
  async checkAll(): Promise<void> {
    for (const route of this.routes) {
      if (this.notLinux) return;
      await this.check(route.id);
    }
  }

  /** Removes a route; when the hub removed it, or has it no more, the list no longer has it and the list is shown again. */
  async remove(id: string): Promise<HubResult<string>> {
    const result = await this.calls.remove(id);
    if (result.ok || result.error.code === 'NOT_FOUND') {
      this.routes = this.routes.filter((route) => route.id !== id);
      if (this.selected === id) this.selected = null;
    }
    return result;
  }
}
