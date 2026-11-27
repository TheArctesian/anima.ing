import type { OsUpdateAction } from '$lib/hub/generated/graphql';
import type { OsUpdateCalls } from './api';
import { updateStatus } from './status';

type RestartAction = 'ACTIVATE' | 'ROLLBACK';
interface RestartReview { action: RestartAction; revision: string; selection: string }

export class OsUpdate {
  status = $state<ReturnType<typeof updateStatus> | null>(null);
  error = $state<string | null>(null);
  busy = $state(false);
  started = $state(false);
  confirmation = $state<RestartReview | null>(null);

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
    const restarts = action === 'ACTIVATE' || action === 'ROLLBACK';
    if (restarts && !confirmed) {
      this.review(action);
      return;
    }
    const review = this.confirmation;
    if (restarts && review?.action !== action) return;
    this.busy = true;
    this.confirmation = null;
    try {
      const result = await this.calls.request(this.device, action, confirmed, restarts ? review?.revision ?? null : null);
      this.error = result.ok
        ? (result.value.requestDeviceOsUpdate ? null : 'The device did not accept the request.')
        : result.error.message;
    } finally { this.busy = false; }
    if (this.error === null) await this.refresh();
  }

  private review(action: RestartAction): void {
    const status = this.status;
    const revision = action === 'ACTIVATE' ? status?.review.activate : status?.review.rollback;
    const selection = action === 'ACTIVATE' ? status?.version : status?.previous;
    if (!revision || !selection || (action === 'ACTIVATE' && status?.phase !== 'staged')) {
      this.confirmation = null;
      this.error = 'The device has no reviewable update selection. Refresh its status or update Anima OS.';
      return;
    }
    this.error = null;
    this.confirmation = { action, revision, selection: selection.replace(/^\/nix\/store\//, '') };
  }
}
