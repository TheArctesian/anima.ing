import { object } from '../receipts';
import { previewPort } from '../preview/grants';
interface Source { owner: string; repository: string; commit: string; repository_id?: string }
export interface CheckoutDraft { kind: 'public' | 'private'; repository: string; commit: string; repositoryId: string }
export const emptyCheckout: Readonly<CheckoutDraft> = { kind: 'public', repository: '', commit: '', repositoryId: '' };
interface Checkout { source: Source; ready: boolean }
interface MoveScope { generation: number; storage_bytes: number; continue_session?: true }
interface NamedMove extends MoveScope { kind: 'move'; destination_device: string }
interface AutomaticMove extends MoveScope { kind: 'move-auto' }
export type Move = NamedMove | AutomaticMove;
export type Action =
  | Move
  | { kind: 'prepare'; storage_bytes: number; source?: Source }
  | { kind: 'delete'; generation: number; confirmed: true }
  | { kind: 'start'; generation: number; memory_bytes: number; cpu_millis: number; tasks: number }
  | { kind: 'stop' | 'reconcile'; generation: number };
export interface Intent { device: string; request: string; workspace: string; action: Action; previewPort?: number }
export interface CompletedMove { destination: string; workspace: string; generation: number; previewPort?: number }
export interface Row { workspace: string; request: string; state: string; action: Action; phase: string | null; generation: number | null; checkout: Checkout | null; storageBytes: number | null; failure: string | null }
export interface Capacity { storageBytes: number; memoryBytes: number; cpuMillis: number; tasks: number }
export interface RetainedSource { transfer: string; workspace: string; sourceDevice: string; sourceName: string; generation: number; pending: boolean }
export interface Inventory { available: boolean; capacity: Capacity | null; privateCheckout: boolean; rows: Row[]; retainedSources: RetainedSource[] }
const phases = ['preparing', 'prepared', 'starting', 'running', 'stopping', 'stopped', 'unknown', 'deleting', 'deleted'];
const states = ['queued', 'offered', 'completed', 'unresolved', 'expired', 'revoked'];
export function uuid(value: unknown): value is string { return typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(value) && value !== '00000000-0000-0000-0000-000000000000'; }
function within(value: unknown, low: number, high: number): value is number { return typeof value === 'number' && Number.isSafeInteger(value) && value >= low && value <= high; }
function readSource(value: unknown): Source | null {
  const row = object(value);
  if (!row || typeof row.owner !== 'string' || !/^[A-Za-z0-9-]{1,39}$/u.test(row.owner)) return null;
  if (typeof row.repository !== 'string' || !/^[A-Za-z0-9._-]{1,100}$/u.test(row.repository) || ['.', '..'].includes(row.repository)) return null;
  if (typeof row.commit !== 'string' || !/^[0-9a-f]{40}$/u.test(row.commit)) return null;
  if (row.repository_id !== undefined && !uuid(row.repository_id)) return null;
  return { owner: row.owner, repository: row.repository, commit: row.commit,
    ...(row.repository_id === undefined ? {} : { repository_id: row.repository_id }) };
}
export function checkoutSource(repository: string, commit: string, repositoryId?: string): Source | null {
  const [owner, name, extra] = repository.trim().split('/');
  return extra === undefined
    ? readSource({ owner, repository: name, commit: commit.trim().toLowerCase(), repository_id: repositoryId })
    : null;
}
function preparation(row: Record<string, unknown>): Action | null {
  if (!within(row.storage_bytes, 2 ** 30, 64 * 2 ** 30)) return null;
  const prepared = { kind: 'prepare' as const, storage_bytes: row.storage_bytes };
  if (row.source === undefined) return prepared;
  const source = readSource(row.source);
  return source ? { ...prepared, source } : null;
}
export function isMove(action: Action): action is Move {
  return action.kind === 'move' || action.kind === 'move-auto';
}
function move(row: Record<string, unknown>, generation: number): Move | null {
  if (!within(row.storage_bytes, 2 ** 30, 64 * 2 ** 30)) return null;
  const scope = {
    generation, storage_bytes: row.storage_bytes,
    ...(row.continue_session === true ? { continue_session: true as const } : {}),
  };
  if (row.kind === 'move-auto') return { kind: 'move-auto', ...scope };
  return uuid(row.destination_device) ? { kind: 'move', destination_device: row.destination_device, ...scope } : null;
}
export function action(value: unknown): Action | null {
  const row = object(value);
  if (!row) return null;
  if (row.kind === 'prepare') return preparation(row);
  if (row.kind === 'delete' && row.confirmed === true && within(row.generation, 0, Number.MAX_SAFE_INTEGER)) return { kind: 'delete', generation: row.generation, confirmed: true };
  if (!within(row.generation, 1, Number.MAX_SAFE_INTEGER)) return null;
  if (row.kind === 'move' || row.kind === 'move-auto') return move(row, row.generation);
  if (row.kind === 'stop' || row.kind === 'reconcile') return { kind: row.kind, generation: row.generation };
  if (row.kind === 'start' && within(row.memory_bytes, 64 * 2 ** 20, 64 * 2 ** 30) && within(row.cpu_millis, 1, 64000) && within(row.tasks, 1, 512)) return { kind: 'start', generation: row.generation, memory_bytes: row.memory_bytes, cpu_millis: row.cpu_millis, tasks: row.tasks };
  return null;
}
function readCapacity(value: unknown): Capacity | null {
  const row = object(value);
  if (!row || !within(row.storageBytes, 0, 64 * 2 ** 30) || !within(row.memoryBytes, 0, 64 * 2 ** 30)
      || !within(row.cpuMillis, 0, 64000) || !within(row.tasks, 0, 512)) return null;
  return { storageBytes: row.storageBytes, memoryBytes: row.memoryBytes, cpuMillis: row.cpuMillis, tasks: row.tasks };
}
export function fitsCapacity(capacity: Capacity | null, requested: Action): boolean {
  if (!capacity) return true;
  if (requested.kind === 'prepare') return requested.storage_bytes <= capacity.storageBytes;
  if (requested.kind === 'start') return requested.memory_bytes <= capacity.memoryBytes
    && requested.cpu_millis <= capacity.cpuMillis && requested.tasks <= capacity.tasks;
  return true;
}
export function inventory(value: unknown, device: string): Inventory | null {
  const data = object(value);
  if (!data || data.device !== device || typeof data.available !== 'boolean' || !Array.isArray(data.workspaces) || data.workspaces.length > 128) return null;
  const capacity = readCapacity(data.capacity);
  if (data.capacity != null && !capacity) return null;
  const rows: Row[] = [];
  for (const value of data.workspaces) {
    const row = workspaceRow(value);
    if (!row) return null;
    rows.push(row);
  }
  if (new Set(rows.map((row) => row.workspace)).size !== rows.length) return null;
  const retainedSources = readRetainedSources(data.retainedSources);
  if (retainedSources === null) return null;
  return { available: data.available, capacity, privateCheckout: data.privateCheckout === true, rows, retainedSources };
}
function readRetainedSources(value: unknown): RetainedSource[] | null {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > 128) return null;
  const copies: RetainedSource[] = [];
  for (const raw of value) {
    const item = object(raw);
    if (!item || !uuid(item.transfer) || !uuid(item.workspace) || !uuid(item.sourceDevice)
      || typeof item.sourceName !== 'string' || !item.sourceName || item.sourceName.length > 256
      || !within(item.generation, 1, Number.MAX_SAFE_INTEGER) || typeof item.pending !== 'boolean') return null;
    copies.push({ transfer: item.transfer, workspace: item.workspace, sourceDevice: item.sourceDevice,
      sourceName: item.sourceName, generation: item.generation, pending: item.pending });
  }
  return new Set(copies.map((copy) => copy.transfer)).size === copies.length ? copies : null;
}
function resultView(value: unknown, workspace: string, planned?: Source): Pick<Row, 'phase' | 'generation' | 'checkout' | 'storageBytes'> | null {
  const result = object(value);
  if (!result) return { phase: null, generation: null, storageBytes: null, checkout: planned ? { source: planned, ready: false } : null };
  if (result.workspace !== workspace || !within(result.generation, 0, Number.MAX_SAFE_INTEGER) || typeof result.phase !== 'string' || !phases.includes(result.phase)) return null;
  const storageBytes = within(result.storageBytes, 2 ** 30, 64 * 2 ** 30) ? result.storageBytes : null;
  const view = { phase: result.phase, generation: result.generation, storageBytes };
  if (result.checkout === undefined) return { ...view, checkout: null };
  const checkout = object(result.checkout);
  const source = readSource(checkout?.source);
  if (!source || typeof checkout?.ready !== 'boolean' || (view.phase === 'running' && !checkout.ready)) return null;
  return { ...view, checkout: { source, ready: checkout.ready } };
}
function workspaceRow(value: unknown): Row | null {
  const item = object(value);
  if (!item || !uuid(item.workspace) || !uuid(item.request) || typeof item.state !== 'string' || !states.includes(item.state)) return null;
  const requested = action(item.action);
  if (!requested) return null;
  const receipt = object(item.receipt);
  const planned = requested.kind === 'prepare' ? requested.source : undefined;
  const result = resultView(receipt?.result, item.workspace, planned);
  if (!result) return null;
  return { workspace: item.workspace, request: item.request, state: item.state, action: requested,
    phase: result.phase,
    checkout: result.checkout,
    storageBytes: result.storageBytes,
    generation: result.generation ?? (requested.kind !== 'prepare' ? requested.generation : null),
    failure: typeof receipt?.failure === 'string' ? receipt.failure.slice(0,512) : null };
}
export function workspaceRequestKey(origin: string, device: string): string {
  return `anima-workspace-request:${origin}:${device}`;
}

export function readIntent(storage: Storage | null, key: string, device: string): Intent | null {
  try {
    const raw = storage?.getItem(key);
    if (!raw || raw.length > 4096) return null;
    const row = object(JSON.parse(raw));
    const requested = action(row?.action);
    if (!row || row.device !== device || !uuid(row.request) || !uuid(row.workspace) || !requested) return null;
    const port = isMove(requested) && previewPort(row.previewPort) ? row.previewPort : undefined;
    return { device, request: row.request, workspace: row.workspace, action: requested, previewPort: port };
  } catch { return null; }
}
export function writeIntent(storage: Storage | null, key: string, intent: Intent | null): boolean {
  try { if (!storage) return false; if (intent) storage.setItem(key, JSON.stringify(intent)); else storage.removeItem(key); return true; } catch { return false; }
}
export function status(row: Row, capacity: Capacity | null): string {
  if (row.state === 'queued') return fitsCapacity(capacity, row.action) ? 'Waiting for the node' : 'Waiting for available capacity';
  if (row.state === 'offered') return 'Working on the node';
  if (row.state === 'unresolved') return row.failure ?? 'Outcome uncertain. Reconcile before starting more work.';
  if (row.state === 'expired') return 'The request expired before delivery.';
  if (row.state === 'revoked') return 'Request cancelled';
  if (row.phase === 'stopped' && row.checkout?.ready === false) return 'Checkout interrupted. Create a new workspace.';
  return ({ prepared: 'Ready to start', running: 'Workspace running', stopped: 'Stopped · files retained', unknown: 'Needs reconciliation', deleting: 'Deleting workspace files', deleted: 'Workspace deleted' } as Record<string, string>)[row.phase ?? ''] ?? 'Check workspace state';
}

export function handoffState(value: unknown, intent: Intent): { state: string; destination: string | null } | null {
  const row = object(value);
  const states = ['awaiting-export', 'staged', 'ready', 'restored', 'activating', 'checking-activation', 'activated', 'expired', 'withdrawing', 'withdrawn', 'waiting-placement', 'placement-expired'];
  if (!isMove(intent.action) || !row || row.transfer !== intent.request || row.workspace !== intent.workspace
    || row.sourceDevice !== intent.device || typeof row.state !== 'string' || !states.includes(row.state)) return null;
  const destination = uuid(row.destinationDevice) && row.destinationDevice !== intent.device ? row.destinationDevice : null;
  if (intent.action.kind === 'move' && destination !== intent.action.destination_device) return null;
  if (destination === null && !['waiting-placement', 'placement-expired', 'withdrawn'].includes(row.state)) return null;
  return { state: row.state, destination };
}
export function handoffStatus(state: string, named = false): string {
  switch (state) {
    case 'waiting-placement': return named
      ? 'Waiting for capacity on the selected node. The workspace stays on this node.'
      : 'Waiting for a node with enough capacity. The workspace stays on this node.';
    case 'placement-expired': return 'The move expired while waiting for capacity. The workspace stayed on this node.';
    case 'awaiting-export': return 'Stopping the source workspace and preparing the project.';
    case 'staged': return 'Verifying the source export.';
    case 'ready': return 'Copying the project to the destination.';
    case 'restored':
    case 'activating': return 'Preparing the workspace on the destination.';
    case 'checking-activation': return 'Checking whether the destination completed the move.';
    case 'expired': return 'The move expired. The original project is still on the source.';
    case 'withdrawing': return 'Checking the destination before recovering the source.';
    default: return 'Checking the workspace move.';
  }
}
