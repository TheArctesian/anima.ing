import { outcomeFrom, type Outcome } from '$lib/agent-threads/live/run';
import { REFUSAL_WORDS } from '$lib/agent-threads/live/refusals';

interface Continuation {
  request: string;
  provider: 'codex' | 'claude_code';
  sourceThread: string | null;
  delivery: string | null;
  signedIn: boolean;
}
export interface Workspace { workspace: string; generation: number; enabled: boolean; ready: boolean; listeningPorts?: number[]; continuation?: Continuation }
export interface Intent { request: string; device: string; workspace: string; generation: number; kind: string; thread?: string }
export interface Receipt { state: string; result: Outcome | null; hubThread?: string }
export function object(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : null;
}
function listeningPorts(value: unknown): number[] {
  if (!Array.isArray(value) || value.length > 16) return [];
  return [...new Set(value.filter((port): port is number =>
    typeof port === 'number' && Number.isInteger(port) && port >= 1 && port <= 65535))].sort((a, b) => a - b);
}

function continuation(value: unknown): Continuation | undefined {
  const row = object(value);
  if (!row || typeof row.request !== 'string' || typeof row.signedIn !== 'boolean') return undefined;
  if (row.provider !== 'codex' && row.provider !== 'claude') return undefined;
  return {
    request: row.request,
    provider: row.provider === 'claude' ? 'claude_code' : 'codex',
    sourceThread: typeof row.sourceThread === 'string' ? row.sourceThread : null,
    delivery: typeof row.delivery === 'string' ? row.delivery : null,
    signedIn: row.signedIn,
  };
}

export function continuationIntent(device: string, workspace: Workspace | undefined): Intent | undefined {
  if (!workspace?.continuation?.delivery) return undefined;
  return { request: workspace.continuation.request, device, workspace: workspace.workspace, generation: workspace.generation, kind: 'continue' };
}

export function workspaces(value: unknown): Workspace[] | null {
  if (!Array.isArray(value) || value.length > 128) return null;
  const rows: Workspace[] = [];
  for (const valueRow of value) {
    const row = object(valueRow);
    if (!row || typeof row.workspace !== 'string' || !Number.isSafeInteger(row.generation) || Number(row.generation) < 1 || typeof row.enabled !== 'boolean' || typeof row.ready !== 'boolean') return null;
    rows.push({ workspace: row.workspace, generation: Number(row.generation), enabled: row.enabled, ready: row.ready, listeningPorts: listeningPorts(row.listeningPorts), continuation: continuation(row.continuation) });
  }
  return rows;
}
export function receipt(value: unknown): Receipt | null {
  const row = object(value);
  if (!row || typeof row.state !== 'string') return null;
  return { state: row.state, result: outcomeFrom(row.result), hubThread: typeof row.hubThread === 'string' ? row.hubThread : undefined };
}
export function words(value: Receipt | undefined): string {
  if (!value) return 'Checking delivery…';
  if (value.result?.outcome === 'refused') return value.result.said ?? REFUSAL_WORDS[value.result.why];
  if (value.result?.outcome === 'started') return 'Session started';
  if (value.state === 'completed') return 'Command completed';
  if (value.state === 'unresolved') return 'Delivery is uncertain. The command will not be repeated.';
  if (value.state === 'not-found') return 'No receipt yet. A lost response may still be settling.';
  return ({ queued: 'Waiting for the node', offered: 'Delivering to the node', expired: 'Delivery expired', revoked: 'Delivery cancelled' } as Record<string,string>)[value.state] ?? 'Checking delivery…';
}
export function load(storage: Storage | null, key: string): Intent[] {
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(key) ?? '[]');
    if (!Array.isArray(parsed) || parsed.length > 32) return [];
    return parsed.filter((value): value is Intent => {
      const row = object(value);
      return !!row && ['request','device','workspace','kind'].every((key) => typeof row[key] === 'string') && Number.isSafeInteger(row.generation);
    });
  } catch { return []; }
}
export function save(storage: Storage | null, key: string, items: Intent[]): boolean {
  try { if (!storage) return false; storage.setItem(key, JSON.stringify(items)); return true; } catch { return false; }
}
