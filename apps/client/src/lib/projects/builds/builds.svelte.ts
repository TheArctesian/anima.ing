import type { BuildInput, BuildReceipt, SourceBuildApi } from './api';
import { finished, identity, limit, load, save, type BuildIntent } from './records';

const definiteRefusal = new Set(['VALIDATION', 'CONFLICT', 'FORBIDDEN', 'UNAUTHENTICATED', 'NOT_FOUND', 'unserved']);

export class SourceBuilds {
  requests = $state<BuildIntent[]>([]);
  receipts = $state<Record<string, BuildReceipt>>({});
  unavailable = $state<Record<string, boolean>>({});
  sending = $state(false);
  error = $state('');
  private reading = false;

  constructor(private api: SourceBuildApi, private storage: Storage | null, private key: string) {
    this.requests = load(storage, key);
  }

  async start(selection: Omit<BuildInput, 'request'>): Promise<boolean> {
    if (this.sending || this.requests.some((entry) => entry.job === null)) return false;
    const entry = { input: { ...selection, request: crypto.randomUUID() }, job: null };
    const next = [entry, ...this.requests].slice(0, limit);
    if (!save(this.storage, this.key, next)) {
      this.error = 'Enable local storage to recover this build request.';
      return false;
    }
    this.requests = next;
    return await this.submit(entry, false);
  }

  retry(entry: BuildIntent): Promise<boolean> {
    return this.submit(entry, true);
  }

  private async submit(entry: BuildIntent, retry: boolean): Promise<boolean> {
    if (this.sending || entry.job !== null) return false;
    this.sending = true;
    this.error = '';
    try {
      const result = await this.api.queue(entry.input);
      if (!result.ok) {
        if (!retry && definiteRefusal.has(result.error.code)) {
          this.remove(entry.input.request);
          this.error = result.error.message;
        } else {
          this.error = 'Build submission could not be confirmed. Recover the original request.';
        }
        return false;
      }
      const receipt = result.value.queueSourceBuild;
      if (!identity(receipt?.id)) {
        this.error = 'The Hub returned an unreadable build receipt. Recover the original request.';
        return false;
      }
      this.remember(entry, receipt);
      return true;
    } finally { this.sending = false; }
  }

  private remove(request: string): void {
    const next = this.requests.filter((entry) => entry.input.request !== request);
    if (save(this.storage, this.key, next)) this.requests = next;
  }

  private remember(entry: BuildIntent, receipt: BuildReceipt): void {
    this.receipts[receipt.id] = receipt;
    const next = this.requests.map((row) => row.input.request === entry.input.request ? { ...row, job: receipt.id } : row);
    if (!save(this.storage, this.key, next)) {
      this.error = 'The build was queued. The original request remains available for recovery.';
      return;
    }
    this.requests = next;
  }

  async refresh(): Promise<void> {
    if (this.reading) return;
    this.reading = true;
    try {
      for (const { job } of this.requests) {
        if (!job || finished(this.receipts[job])) continue;
        const result = await this.api.status(job);
        const receipt = result.ok ? result.value.sourceBuild : null;
        this.unavailable[job] = receipt?.id !== job;
        if (receipt?.id === job) this.receipts[job] = receipt;
      }
    } finally { this.reading = false; }
  }
}
