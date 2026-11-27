import type { NodeCodeApi } from './api';
import { continuationIntent, load, receipt, save, type Intent, type Receipt, type Workspace } from './receipts';

/** Only routing identities survive reload; prompt and approval content never enters browser storage. */
export class NodeSession {
  intents = $state<Intent[]>([]);
  receipts = $state<Record<string, Receipt>>({});
  error = $state('');
  busy = $state(false);
  private reading = false;
  constructor(private api: NodeCodeApi, private storage: Storage | null, private key: string) {
    this.intents = load(storage, key);
  }
  async refresh(active?: Intent): Promise<void> {
    if (this.reading) return;
    this.reading = true;
    try {
      const intents = active ? [...this.intents, active] : this.intents;
      for (const intent of intents) {
        const held = this.receipts[intent.request];
        if (held && ['completed','revoked','expired'].includes(held.state) && !(held.result?.outcome === 'started' && !held.hubThread)) continue;
        const result = await this.api.receipt(intent.request);
        if (!result.ok) { this.error = result.error.message; break; }
        const value = result.value.managedCodeCommand;
        this.receipts[intent.request] = value === null ? { state: 'not-found', result: null } : receipt(value) ?? { state: 'unresolved', result: null };
      }
    } finally { this.reading = false; }
  }
  async send(device: string, workspace: Workspace, action: Record<string, unknown>): Promise<boolean> {
    if (this.busy) return false;
    this.error = '';
    const intent: Intent = { request: crypto.randomUUID(), device, workspace: workspace.workspace, generation: workspace.generation, kind: String(action.call) };
    if (typeof action.thread === 'string') intent.thread = action.thread;
    if (['start','send'].includes(String(action.call)) && this.intents.some((item) => item.device === device && item.workspace === workspace.workspace && item.generation === workspace.generation && !['completed','revoked','expired'].includes(this.receipts[item.request]?.state ?? ''))) {
      this.error = 'Wait for the pending delivery before sending another prompt.';
      return false;
    }
    const retained = this.intents.filter((item) => !['completed','revoked','expired'].includes(this.receipts[item.request]?.state ?? ''));
    if (retained.length >= 32) { this.error = 'Resolve pending deliveries before sending more work.'; return false; }
    const next = [...this.intents];
    while (next.length >= 32) {
      const terminal = next.findIndex((item) => ['completed','revoked','expired'].includes(this.receipts[item.request]?.state ?? '') && !this.keepsThread(item));
      if (terminal < 0) { this.error = 'End an older session before opening more workspaces.'; return false; }
      next.splice(terminal, 1);
    }
    next.push(intent);
    if (!save(this.storage, this.key, next)) { this.error = 'Command recovery storage is unavailable. No command was sent.'; return false; }
    this.intents = next;
    this.busy = true;
    try {
      const result = await this.api.submit(device, intent.request, intent, action);
      if (!result.ok) { this.error = `${result.error.message} Delivery will be checked; the command will not be repeated.`; return false; }
      this.receipts[intent.request] = { state: 'queued', result: null };
      return true;
    } finally { this.busy = false; }
  }
  private keepsThread(intent: Intent): boolean {
    const result = this.receipts[intent.request]?.result;
    if (result?.outcome !== 'started') return false;
    const active = this.thread(intent.device, { workspace: intent.workspace, generation: intent.generation, enabled: true, ready: true });
    return active === result.thread;
  }
  hubThread(thread: string | null): string | null {
    if (!thread) return null;
    for (const value of Object.values(this.receipts)) {
      if (value.result?.outcome === 'started' && value.result.thread === thread) return value.hubThread ?? null;
    }
    return null;
  }
  private continuedThread(device: string, workspace: Workspace): string | null {
    const continued = continuationIntent(device, workspace);
    const automatic = continued && this.receipts[continued.request]?.result;
    return automatic?.outcome === 'started' ? automatic.thread : null;
  }
  thread(device: string, workspace: Workspace): string | null {
    let thread = this.continuedThread(device, workspace);
    for (const intent of this.intents) {
      if (intent.device !== device || intent.workspace !== workspace.workspace || intent.generation !== workspace.generation) continue;
      const result = this.receipts[intent.request]?.result;
      if (result?.outcome === 'started') thread = result.thread;
      if (intent.kind === 'stop' && intent.thread === thread && result?.outcome === 'stopped') thread = null;
    }
    return thread;
  }
}
