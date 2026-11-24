import type { Hub, HubError } from '$lib/hub/hub';
import { devicesApi } from '../client';
import { siteOptions, type MenuOption } from './toolbar';

export class Sites {
  #all = $state.raw<readonly { id: string; name: string }[]>([]);
  error = $state.raw<HubError | null>(null);
  readonly #api;

  constructor(hub: Hub) {
    this.#api = devicesApi(hub);
  }

  /** Empty while loading, after a failure and with no site, so the note shows. */
  get options(): readonly MenuOption[] {
    return this.#all.length === 0 ? [] : siteOptions(this.#all);
  }

  get note(): string {
    return this.error?.message ?? 'No sites yet';
  }

  async load(): Promise<void> {
    const result = await this.#api.sites();
    if (result.ok) this.#all = result.value;
    else this.error = result.error;
  }
}
