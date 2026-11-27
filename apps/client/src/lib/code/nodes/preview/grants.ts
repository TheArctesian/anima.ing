import { object } from '../receipts';

export interface PreviewScope { device: string; workspace: string; generation: number }
export interface PreviewSelection extends PreviewScope { port: number }
export interface PreviewGrant { endpoint: string; port: number; expiresAt: string }

export function previewPort(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value > 0 && value <= 65535;
}

export function previewGrant(value: unknown, scope: PreviewScope): PreviewGrant | null {
  const row = object(value);
  if (!row || row.device !== scope.device || row.workspace !== scope.workspace || row.generation !== scope.generation) return null;
  if (typeof row.endpoint !== 'string' || !previewPort(row.port) || row.revoked !== false) return null;
  if (typeof row.expiresAt !== 'string' || !Number.isFinite(Date.parse(row.expiresAt))) return null;
  return { endpoint: row.endpoint, port: row.port, expiresAt: row.expiresAt };
}
