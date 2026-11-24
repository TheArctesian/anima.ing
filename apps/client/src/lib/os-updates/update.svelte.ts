import type { OsUpdateAction } from '$lib/hub/generated/graphql';
import type { OsUpdateCalls } from './api';
import { updateStatus } from './status';

export class OsUpdate {
  status = $state<ReturnType<typeof updateStatus> | null>(null);
  error = $state<string | null>(null);
  busy = $state(false);
  started = $state(false);
  confirmation = $state<'ACTIVATE' | 'ROLLBACK' | null>(null);

  constructor(private readonly calls: OsUpdateCalls, private readonly device: string) {}

  async refresh(): Promise<void> {
    if (this.busy) return;
    this.started = true;
    this.busy = true;
    try {
      const result = await this.calls.status(this.device);
      if (result.ok) {
        this.status = updateStatus(result.value.deviceOsUpdate);
        this.error = null;
      } else {
        this.status = null;
        this.error = result.error.message;
      }
    } finally { this.busy = false; }
  }

  async request(action: OsUpdateAction, confirmed = false): Promise<void> {
    if (this.busy) return;
    if ((action === 'ACTIVATE' || action === 'ROLLBACK') && !confirmed) {
      this.confirmation = action;
      return;
    }
    this.busy = true;
    this.confirmation = null;
    try {
      const result = await this.calls.request(this.device, action, confirmed);
      this.error = result.ok && result.value.requestDeviceOsUpdate ? null : result.ok ? 'The device did not accept the request.' : result.error.message;
    } finally { this.busy = false; }
    if (this.error === null) await this.refresh();
  }
}
