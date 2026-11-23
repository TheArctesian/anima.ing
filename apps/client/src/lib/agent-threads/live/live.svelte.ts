import type { RunnerShell } from '$lib/desktop/agent-threads/runner';
import { foreseen, type Listing, type ListingChange } from '../listing/listing';
import { noticeFor, type Looking } from './notify';
import type { FoundFile } from './files';
import { REFUSAL_WORDS } from './refusals';
import type { Action, Outcome, Update } from './run';
import { applied, begun, type LiveThread } from './timeline';

/** The runner replays its kept updates when the watch reaches it; no notification goes out this long after (THR-059). */
export const REPLAY_MS = 2_000;
/** While the replay goes on, each update keeps the page quiet this long again. */
export const REPLAY_GAP_MS = 500;
export const STOPPED_KEPT = 50;

export type CallResult =
  | { readonly ok: true; readonly outcome: Outcome }
  /** `answered`: the copies of a program that answered when the chosen one did not (TH97). */
  | { readonly ok: false; readonly words: string; readonly answered?: readonly string[] };

/** The threads that this computer's runner drives (TH14); a start over the soft limit waits for the person to raise it (THR-114). */
export class LiveThreads {
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each update, never changed in place
  threads = $state<ReadonlyMap<string, LiveThread>>(new Map());
  linked = $state<boolean | null>(null);
  problem = $state<string | null>(null);
  /** Words while the holder of the programs is older than the daemon (TH98). */
  holder = $state<string | null>(null);
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- replaced whole at each answer, never changed in place
  listings = $state<ReadonlyMap<string, Listing>>(new Map());
  overLimit = $state<Action | null>(null);
  /** Words of the update of this computer's daemon (HMD-055). */
  daemon = $state<string | null>(null);
  /** Called with a thread that changed while the person looks at it (THR-055). */
  seenOpen: ((id: string) => void) | null = null;
  daemonWords: (() => Promise<string | null>) | null = null;
  private quietUntil = 0;

  constructor(
    private readonly shell: RunnerShell,
    private readonly looking: () => Looking,
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- the clock: read once at each call, never kept
    private readonly now: () => Date = () => new Date(),
  ) {}

  async begin(): Promise<() => void> {
    try {
      const stop = await this.shell.watch(
        (update) => {
          this.take(update);
        },
        (linked) => {
          this.linked = linked;
          if (linked) this.quietUntil = this.now().getTime() + REPLAY_MS;
          if (linked) void this.readDaemon();
        },
      );
      void this.readDaemon();
      await this.readRows();
      await this.readListings();
      return stop;
    } catch (reason) {
      this.linked = false;
      this.problem = reason instanceof Error ? reason.message : 'The thread runner did not answer.';
      return () => undefined;
    }
  }

  /** The update of the daemon ends after the app started; words of an earlier read stay until new words come. */
  private async readDaemon(): Promise<void> {
    const words = await this.daemonWords?.().catch(() => null);
    if (words !== null && words !== undefined) this.daemon = words;
  }

  /** The holder is replaced when no thread runs: its words go when the list no longer has them. */
  private async readHolder(): Promise<void> {
    const listed = await this.shell.call({ call: 'list' }).catch(() => null);
    if (listed?.outcome === 'threads') this.holder = listed.holder ?? null;
  }

  async readRows(): Promise<void> {
    const listed = await this.shell.call({ call: 'list' });
    if (listed.outcome !== 'threads') return;
    this.holder = listed.holder ?? null;
    const at = this.now();
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- built here, then set whole as the new state
    const next = new Map(this.threads);
    for (const row of listed.threads) if (!next.has(row.id)) next.set(row.id, begun(row, at));
    this.threads = next;
  }

  #setListing(thread: string, listing: Listing | undefined): void {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a copy set whole as the new state
    const next = new Map(this.listings);
    if (listing === undefined) next.delete(thread);
    else next.set(thread, listing);
    this.listings = next;
  }

  async readListings(): Promise<void> {
    const read = await this.shell.call({ call: 'listings' }).catch(() => null);
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- built here, then set whole as the new state
    if (read?.outcome === 'listings') this.listings = new Map(read.listings.map((listing) => [listing.thread, listing]));
  }

  /** Changes how the person lists a thread (TH46): the row moves at once and goes back when the runner refuses. */
  async changeListing(thread: string, change: ListingChange): Promise<CallResult> {
    const before = this.listings.get(thread);
    const next = foreseen(before, thread, change, this.now());
    if (next !== null) this.#setListing(thread, next);
    const result = await this.call({ call: 'change_listing', thread, change });
    if (!result.ok || result.outcome.outcome !== 'listings') {
      if (next !== null && this.listings.get(thread) === next) this.#setListing(thread, before);
      return result;
    }
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- built here, then set whole as the new state
    this.listings = new Map(result.outcome.listings.map((listing) => [listing.thread, listing]));
    if (change.change === 'remove' && this.threads.has(thread)) {
      // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a copy set whole as the new state
      const next = new Map(this.threads);
      next.delete(thread);
      this.threads = next;
    }
    return result;
  }

  /** The `@` list of the composer (TH108): the files of the folder of a thread that match `query`, the best first. */
  async files(thread: string, query: string, most: number): Promise<{ readonly ok: true; readonly paths: readonly FoundFile[]; readonly more: boolean } | { readonly ok: false; readonly words: string }> {
    const result = await this.call({ call: 'files', thread, query, most });
    if (!result.ok) return result;
    return result.outcome.outcome === 'files' ? { ok: true, paths: result.outcome.paths, more: result.outcome.more } : { ok: false, words: 'The thread runner did not answer with files.' };
  }

  take(update: Update): void {
    const at = this.now();
    const before = this.threads.get(update.thread);
    const base = before ?? (update.change.change === 'state' ? begun(update.change.row, at) : undefined);
    if (base === undefined) return;
    const after = applied(base, update, at);
    if (after === before) return;
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a copy set whole as the new state
    this.threads = pruned(new Map(this.threads).set(update.thread, after));
    if (this.holder !== null && update.change.change === 'state' && update.change.state === 'stopped') void this.readHolder();
    const looking = this.looking();
    if (looking.front && looking.chosen === update.thread) this.seenOpen?.(update.thread);
    if (at.getTime() < this.quietUntil) {
      this.quietUntil = Math.max(this.quietUntil, at.getTime() + REPLAY_GAP_MS);
      return;
    }
    const notice = noticeFor(before, after, update, looking);
    if (notice !== null) {
      this.shell.notify(notice.title, notice.words).catch((reason: unknown) => {
        this.problem = reason instanceof Error ? reason.message : 'The notification was not shown.';
      });
    }
  }

  async call(action: Action): Promise<CallResult> {
    let outcome: Outcome;
    try {
      outcome = await this.shell.call(action);
    } catch (reason) {
      return { ok: false, words: reason instanceof Error ? reason.message : 'The thread runner did not answer.' };
    }
    if (outcome.outcome !== 'refused') return { ok: true, outcome };
    if (outcome.why === 'over_limit' && action.call === 'start') this.overLimit = action;
    const words = outcome.said ?? REFUSAL_WORDS[outcome.why];
    return outcome.answered === undefined ? { ok: false, words } : { ok: false, words, answered: outcome.answered };
  }

  /** When the raise fails, the start still waits, so that the dialog can say why. */
  async raiseLimit(): Promise<CallResult> {
    const waiting = this.overLimit;
    const raised = await this.call({ call: 'raise_limit' });
    if (!raised.ok || waiting === null) return raised;
    this.overLimit = null;
    return this.call(waiting);
  }

  keepLimit(): void {
    this.overLimit = null;
  }
}

function pruned(threads: Map<string, LiveThread>): Map<string, LiveThread> {
  const stopped = [...threads.values()].filter((thread) => thread.row.state === 'stopped').sort((a, b) => a.last.getTime() - b.last.getTime());
  for (const thread of stopped.slice(0, Math.max(0, stopped.length - STOPPED_KEPT))) threads.delete(thread.row.id);
  return threads;
}
