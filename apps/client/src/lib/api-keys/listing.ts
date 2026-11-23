// INT-003, UI-003.
import type { ApiKey } from './api';

const SCOPE_LABELS: Record<ApiKey['scope'], string> = { READ: 'Read', WRITE: 'Write' };

export function scopeLabel(scope: ApiKey['scope']): string {
  return SCOPE_LABELS[scope];
}

/** Who uses the key, in words (LOG-015). */
const MADE_FOR_LABELS: Record<ApiKey['madeFor'], string> = { USER: 'A script I run', AGENT: 'An AI agent' };

export function madeForLabel(madeFor: ApiKey['madeFor']): string {
  return MADE_FOR_LABELS[madeFor];
}

/** A date the client cannot read counts as not expired. */
export function isExpired(key: Pick<ApiKey, 'expiresAt'>, now: Date): boolean {
  return Date.parse(key.expiresAt) <= now.getTime();
}

export function shownDate(iso: string, locale?: string): string {
  const at = new Date(iso);
  return Number.isNaN(at.getTime()) ? '—' : at.toLocaleDateString(locale, { dateStyle: 'medium' });
}

export function lastUsedText(key: Pick<ApiKey, 'lastUsedAt'>, locale?: string): string {
  return key.lastUsedAt === null ? 'Never' : shownDate(key.lastUsedAt, locale);
}

/** Never shown as text; the skeleton is drawn from it so it has a real row's geometry at every width (MOT-007). */
export const PLACEHOLDER_KEY: ApiKey = {
  id: 'placeholder',
  name: 'placeholder-key',
  scope: 'WRITE',
  madeFor: 'AGENT',
  prefix: 'anima_AAAAAA',
  expiresAt: '2099-12-31T00:00:00Z',
  createdAt: '2099-12-31T00:00:00Z',
  lastUsedAt: '2099-12-31T00:00:00Z',
};

/** `keys` is the list without the revoked key; `null` sends the focus to "Create API key". */
export function focusAfterRevoke(keys: readonly Pick<ApiKey, 'id'>[], index: number): string | null {
  return (keys[index] ?? keys[index - 1])?.id ?? null;
}
