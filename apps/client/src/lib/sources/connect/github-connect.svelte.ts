import type { HubResult } from '$lib/hub/hub';
import type { ConnectAttempt, GithubConnectApi } from './api';
import { isOpen } from './attempt';
import type { LinkOpening } from './browser';

/** The one-time link stays in a local variable: never a field, stored, logged or in a message (PRJ-039, PRJ-041). */
export class GithubConnectStore {
  attempt = $state.raw<ConnectAttempt | null>(null);
  /** `null` when the hub cannot start the flow in a browser (HMD-070); the client has no rule of its own. */
  browserPage = $state<string | null | undefined>(undefined);
  restarted = $state(false);
  blocked = $state(false);
  refusal = $state('');
  busy = $state(false);
  #unwatch: (() => void) | null = null;

  constructor(private readonly api: GithubConnectApi) {}

  /** `null` after an open attempt means the hub restarted. */
  async load(): Promise<void> {
    const held = this.attempt !== null && isOpen(this.attempt);
    const result = await this.api.current();
    if (!result.ok) return;
    this.restarted = result.value === null && held;
    this.attempt = result.value;
  }

  async ensureLoaded(): Promise<void> {
    const [page] = await Promise.all([this.api.browserPage(), this.load()]);
    this.browserPage = page;
    this.#unwatch ??= this.api.watch({
      next: (attempt) => {
        this.restarted = false;
        this.attempt = attempt;
      },
      error: () => undefined,
      complete: () => undefined,
    });
  }

  stop(): void {
    this.#unwatch?.();
    this.#unwatch = null;
  }

  async start(opening: LinkOpening): Promise<void> {
    this.#begin();
    const result = await this.api.start();
    if (!result.ok) return this.#refused(result, opening);
    this.restarted = false;
    this.attempt = result.value;
    await this.#open(result.value.id, opening);
  }

  async continueInBrowser(opening: LinkOpening): Promise<void> {
    if (this.attempt === null) return opening.close();
    this.#begin();
    await this.#open(this.attempt.id, opening);
  }

  async cancel(): Promise<void> {
    if (this.attempt === null) return;
    this.#begin();
    const result = await this.api.cancel(this.attempt.id);
    if (result.ok) this.attempt = result.value;
    else this.refusal = result.error.message;
    this.busy = false;
  }

  async #open(id: string, opening: LinkOpening): Promise<void> {
    const link = await this.api.link(id);
    if (!link.ok) return this.#refused(link, opening);
    this.blocked = !(await opening.open(link.value));
    this.busy = false;
  }

  #begin(): void {
    this.refusal = '';
    this.blocked = false;
    this.busy = true;
  }

  #refused(result: Extract<HubResult<unknown>, { ok: false }>, opening: LinkOpening): void {
    opening.close();
    this.refusal = result.error.message;
    this.busy = false;
  }
}
