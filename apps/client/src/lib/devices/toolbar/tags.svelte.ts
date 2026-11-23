import type { Hub, HubError } from '$lib/hub/hub';
import { devicesApi } from '../client';

export class Tags {
  names = $state.raw<readonly string[]>([]);
  error = $state.raw<HubError | null>(null);
  readonly #api;

  constructor(hub: Hub) {
    this.#api = devicesApi(hub);
  }

  get note(): string {
    return this.error?.message ?? 'No tags yet';
  }

  async load(): Promise<void> {
    const result = await this.#api.tags();
    if (result.ok) this.names = result.value;
    else this.error = result.error;
  }
}
