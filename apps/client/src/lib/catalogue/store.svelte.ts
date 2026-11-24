import type { CatalogueCalls, CatalogueEntry } from './api';
import { grouped, tierCounts, type Group, type TierChoice } from './entry';

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/** The hub reads the catalogue from its own binary, so it is read when first opened and again only on request (H38). */
export class CatalogueStore {
  status = $state<ReadStatus>('idle');
  problem = $state('');
  entries = $state.raw<readonly CatalogueEntry[]>([]);
  selected = $state<string | null>(null);
  search = $state('');
  /** The tier that the list shows (OD-HOST-50). */
  tier = $state<TierChoice>('ALL');
  #generation = 0;

  constructor(private readonly calls: CatalogueCalls) {}

  get shown(): CatalogueEntry | null {
    return this.entries.find((entry) => entry.id === this.selected) ?? null;
  }

  /** Grouped by category (H39). */
  get groups(): Group<CatalogueEntry>[] {
    return grouped(this.entries, this.search, this.tier);
  }

  /** How many entries each choice of the tier holds, before the search. */
  get counts(): Record<TierChoice, number> {
    return tierCounts(this.entries);
  }

  filter(search: string, tier: TierChoice): void {
    this.search = search;
    this.tier = tier;
  }

  move(step: number): void {
    const shown = this.groups.flatMap((group) => group.entries);
    if (shown.length === 0) return;
    const at = shown.findIndex((entry) => entry.id === this.selected);
    const next = at === -1 ? 0 : Math.min(Math.max(at + step, 0), shown.length - 1);
    this.selected = shown[next]?.id ?? null;
  }

  /** Reads the list; an answer to an earlier read is dropped. */
  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.status = 'loading';
    const result = await this.calls.entries();
    if (generation !== this.#generation) return;
    if (!result.ok) {
      this.problem = result.error.message;
      this.status = 'failed';
      return;
    }
    this.entries = result.value;
    this.problem = '';
    this.status = 'ready';
  }

  async open(): Promise<void> {
    if (this.status === 'idle') await this.load();
  }

  select(id: string | null): void {
    this.selected = id;
  }
}
