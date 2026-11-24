import type { LiveThreads } from './live/live.svelte';
import { listed, type ListedRow, type Listing } from './listing/listing';
import type { LiveThread } from './live/timeline';
import { mergedRows, startedThreadIds } from './live/rows';
import type { SentPrompt } from './live/timeline';
import { markSeen, needsYouCount, storedSeen, type Seen } from './state';
import type { Thread, ThreadRow, ThreadsShell } from './thread';

const NO_LISTINGS: ReadonlyMap<string, Listing> = new Map();

/** The last answer that was asked for wins (THR-011). */
export class ThreadsStore {
  rows = $state<readonly ThreadRow[] | null>(null);
  listProblem = $state<string | null>(null);
  thread = $state<Thread | null>(null);
  threadProblem = $state<string | null>(null);
  chosen = $state<string | null>(null);
  loadingThread = $state(false);
  /** Shown until the runner shows it. */
  sent = $state<SentPrompt | null>(null);
  /** A turn that ended needs the person until the thread is opened (THR-055). */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each opening, never changed in place
  seen = $state<Seen>(new Map());
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each read, never changed in place
  readAt = $state(new Date());

  constructor(
    private readonly shell: ThreadsShell,
    private readonly storage: Pick<Storage, 'getItem' | 'setItem'> | null = null,
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- the clock: read once at each call, never kept
    private readonly now: () => Date = () => new Date(),
    readonly live: LiveThreads | null = null,
  ) {
    if (storage !== null) this.seen = storedSeen(storage);
  }

  /** The driven threads, then the session files that are not one of them (THR-056). */
  get allRows(): readonly ThreadRow[] | null {
    const live = [...(this.live?.threads.values() ?? [])];
    return this.rows === null && live.length === 0 ? null : mergedRows(this.rows ?? [], live);
  }

  archived = $state(false);

  /** No removed thread, and only the archived ones when asked (TH46). */
  get listedRows(): readonly ListedRow[] | null {
    const rows = this.allRows;
    return rows === null ? null : listed(rows, this.live?.listings ?? NO_LISTINGS, this.archived);
  }

  get archivedCount(): number {
    return [...(this.live?.listings.values() ?? [])].filter((listing) => listing.archived && !listing.removed).length;
  }

  get liveThread(): LiveThread | null {
    return this.chosen === null ? null : (this.live?.threads.get(this.chosen) ?? null);
  }

  /** The Code badge counts Anima threads in the inbox (THR-057, TH46). */
  get needsYou(): number {
    const listings = this.live?.listings ?? NO_LISTINGS;
    const started = startedThreadIds(listings.values(), this.live?.threads.values() ?? []);
    const rows = listed(this.allRows ?? [], listings, false).filter((row) => started.has(row.id));
    return needsYouCount(rows, this.readAt, this.seen);
  }

  async loadList(): Promise<void> {
    try {
      this.rows = await this.shell.list(null);
      this.readAt = this.now();
      this.listProblem = null;
      void this.live?.readListings();
      // The list is read only while the window is seen: the open thread is in front of the person.
      if (this.chosen !== null) this.markSeen(this.chosen);
    } catch (reason) {
      this.listProblem = reason instanceof Error ? reason.message : 'The threads could not be read.';
    }
  }

  markSeen(id: string): void {
    this.seen = markSeen(this.storage, this.seen, id, this.now());
  }

  /** A thread the runner just `started` has no session file yet, so its row is read from the runner instead. */
  async open(id: string, started = false, sent: Omit<SentPrompt, 'thread'> | null = null): Promise<void> {
    this.chosen = id;
    if (sent !== null) this.sent = { thread: id, ...sent };
    this.markSeen(id);
    this.threadProblem = null;
    if (this.live?.threads.has(id) === true) return;
    if (started && this.live !== null) {
      await this.live.readRows().catch(() => undefined);
      return;
    }
    this.loadingThread = true;
    try {
      const thread = await this.shell.read(id);
      if (this.chosen !== id) return;
      this.thread = thread;
    } catch (reason) {
      if (this.chosen !== id) return;
      this.thread = null;
      this.threadProblem = reason instanceof Error ? reason.message : 'The thread could not be read.';
    } finally {
      if (this.chosen === id) this.loadingThread = false;
    }
  }
}
