import type { HubResult } from '$lib/hub/hub';
import type { GithubAppInput, Source, SourceAccount, SourcesApi } from './api';

export type SourcesStatus = 'loading' | 'ready' | 'failed';

export interface AccountRow extends SourceAccount {
  readonly sourceName: string;
}

/** The source accounts the hub lists (SRC-003). Nothing here holds a credential: `connect` keeps no copy (PRJ-018). */
export class SourcesStore {
  status = $state<SourcesStatus>('loading');
  loadProblem = $state('');
  refusals = $state.raw<Readonly<Record<string, string>>>({});
  working = $state.raw<Readonly<Record<string, boolean>>>({});
  #sources = $state.raw<readonly Source[]>([]);
  #unwatch: (() => void) | null = null;

  readonly rows = $derived(this.#sources.flatMap((source) => source.accounts.map((account): AccountRow => ({ ...account, sourceName: source.name }))));

  constructor(private readonly api: SourcesApi) {}

  async load(): Promise<void> {
    const result = await this.api.list();
    if (result.ok) {
      this.#sources = result.value;
      this.loadProblem = '';
      this.status = 'ready';
    } else {
      this.loadProblem = result.error.message;
      this.status = 'failed';
    }
  }

  async ensureLoaded(): Promise<void> {
    if (this.status !== 'ready') await this.load();
    this.#unwatch ??= this.api.watch({
      next: (account) => {
        this.#place(account);
      },
      error: () => undefined,
      complete: () => undefined,
    });
  }

  stop(): void {
    this.#unwatch?.();
    this.#unwatch = null;
  }

  sync(id: string): Promise<void> {
    return this.#act(id, () => this.api.sync(id));
  }

  setPaused(id: string, paused: boolean): Promise<void> {
    return this.#act(id, () => this.api.setPaused(id, paused));
  }

  async remove(id: string, deleteRecords: boolean): Promise<void> {
    this.#begin(id);
    const result = await this.api.remove(id, deleteRecords);
    if (result.ok) {
      this.#sources = this.#sources.map((source) => ({ ...source, accounts: source.accounts.filter((account) => account.id !== id) }));
    }
    this.#end(id, result);
  }

  /** The input is passed on and not kept; the answer holds no credential (KEY-002, PRJ-018). */
  async connect(input: GithubAppInput): Promise<HubResult<SourceAccount>> {
    const result = await this.api.connectGithubApp(input);
    if (result.ok) this.#place(result.value);
    return result;
  }

  async #act(id: string, run: () => Promise<HubResult<SourceAccount>>): Promise<void> {
    this.#begin(id);
    const result = await run();
    if (result.ok) this.#place(result.value);
    this.#end(id, result);
  }

  #begin(id: string): void {
    this.refusals = Object.fromEntries(Object.entries(this.refusals).filter(([key]) => key !== id));
    this.working = { ...this.working, [id]: true };
  }

  #end(id: string, result: HubResult<unknown>): void {
    this.working = Object.fromEntries(Object.entries(this.working).filter(([key]) => key !== id));
    if (!result.ok) this.refusals = { ...this.refusals, [id]: result.error.message };
  }

  #place(account: SourceAccount): void {
    const known = this.#sources.some((source) => source.accounts.some((each) => each.id === account.id));
    this.#sources = this.#sources.map((source) => {
      if (known) return { ...source, accounts: source.accounts.map((each) => (each.id === account.id ? account : each)) };
      return source.key === account.sourceKey ? { ...source, accounts: [...source.accounts, account] } : source;
    });
  }
}
