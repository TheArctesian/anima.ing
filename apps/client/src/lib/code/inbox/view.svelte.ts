import type { ThreadsStore } from '$lib/agent-threads/store.svelte';
import { narrowed } from '$lib/agent-threads/thread';
import type { InboxEngine } from './engine';
import { arranged, inOrder, inProjects, projectsOf, type InboxRow, type ShelfKey } from './inbox';
import { kept, NO_SELECTION, type Selection } from './selection';

const SHELVES: readonly ShelfKey[] = ['working', 'snoozed', 'settled'];

function shelfKey(key: ShelfKey): string {
  return `anima.code.shelf.${key}`;
}

/** The shelves the person opened on this device; all start folded (02 section 5.4). Storage that refuses folds them. */
function storedShelves(storage: Pick<Storage, 'getItem'> | null): ReadonlySet<ShelfKey> {
  try {
    return new Set(SHELVES.filter((key) => storage?.getItem(shelfKey(key)) === 'true'));
  } catch {
    return new Set();
  }
}

/**
 * What the inbox shows of the threads of the store: the search, the project filter, the open shelves and the
 * selection, and from them the parts and the order of the list. The page reads `order` for its keys.
 */
export class InboxView {
  search = $state('');
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each change, never changed in place
  projects = $state<ReadonlySet<string | null>>(new Set());
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each change, never changed in place
  shelves = $state<ReadonlySet<ShelfKey>>(new Set());
  #selection = $state<Selection>(NO_SELECTION);

  constructor(
    private readonly store: ThreadsStore,
    readonly engine: InboxEngine,
    private readonly storage: Pick<Storage, 'getItem' | 'setItem'> | null = null,
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- the clock: read once at each read of the list
    private readonly now: () => Date = () => new Date(),
  ) {
    this.shelves = storedShelves(storage);
  }

  /** The time of the last read of the list: the states and ages are of this time. */
  readonly moment = $derived.by(() => {
    void this.store.allRows;
    return this.now();
  });

  readonly rows = $derived.by(() => this.store.listedRows ?? []);
  /** The newest prompt of each thread the runner drives: a prompt after a settle takes the thread back (TH61). */
  readonly prompted = $derived.by(() => {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a lookup built and read in this derivation only
    const prompts = new Map<string, Date>();
    for (const [id, thread] of this.store.live?.threads ?? []) {
      const at = thread.entries.findLast((entry) => entry.kind === 'prompt')?.at;
      if (at != null) prompts.set(id, at);
    }
    return prompts;
  });
  readonly allProjects = $derived(projectsOf(this.rows));
  readonly parts = $derived.by(() =>
    arranged(inProjects(this.rows, this.projects), { now: this.moment, seen: this.store.seen, marks: this.engine.marks, prompted: this.prompted }),
  );

  /** The rows in the order of the list; a search shows its hits in place of the list (02 section 5.10). */
  readonly order = $derived.by((): readonly InboxRow[] => {
    if (this.search.trim() === '') return inOrder(this.parts, this.shelves);
    const all = [...this.parts.pinned, ...this.parts.active, ...this.parts.working, ...this.parts.snoozed, ...this.parts.settled];
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a lookup built and read in this derivation only
    const hits = new Set(narrowed(all.map((item) => item.row), null, this.search).map((row) => row.id));
    return all.filter((item) => hits.has(item.row.id));
  });

  /** The selection, without the rows that left the list. */
  get selection(): Selection {
    return kept(this.#selection, this.order.map((item) => item.row.id));
  }

  set selection(next: Selection) {
    this.#selection = next;
  }

  /** The row of `id` wherever it is, on a folded shelf too; none when the filter or the list leaves it out. */
  itemOf(id: string): InboxRow | undefined {
    const { pinned, active, working, snoozed, settled } = this.parts;
    return [...pinned, ...active, ...working, ...snoozed, ...settled].find((item) => item.row.id === id);
  }

  /** Whether `id` is in Settled or Snoozed, for its menu. */
  shelfOf(id: string): 'list' | 'snoozed' | 'settled' {
    if (this.parts.snoozed.some((item) => item.row.id === id)) return 'snoozed';
    if (this.parts.settled.some((item) => item.row.id === id)) return 'settled';
    return 'list';
  }

  toggleShelf(key: ShelfKey): void {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- built here, then set whole as the new state
    const next = new Set(this.shelves);
    const open = !next.has(key);
    if (open) next.add(key);
    else next.delete(key);
    this.shelves = next;
    try {
      this.storage?.setItem(shelfKey(key), String(open));
    } catch {
      // The shelf still opens; only the next start forgets it.
    }
  }
}
