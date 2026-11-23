import type { AgentSource, AgentUsageFilter } from '$lib/hub/generated/graphql';
import type { UsageRequest } from './api';

export type Count = 'cost' | 'tokens' | 'sessions';

/** The page state that lives in the address, so Back, Forward and a copied link restore the page. */
export interface UsageQuery {
  readonly days: Days;
  readonly count: Count;
  readonly device: string | null;
  readonly source: AgentSource | null;
  /** `host/owner/name` as the daemon sends it. */
  readonly repository: string | null;
}

export const DAY_CHOICES = [7, 30, 90] as const;
export type Days = (typeof DAY_CHOICES)[number];

export const COUNT_CHOICES: readonly { readonly value: Count; readonly label: string }[] = [
  { value: 'cost', label: 'Cost' },
  { value: 'tokens', label: 'Tokens' },
  { value: 'sessions', label: 'Sessions' },
];

export const SOURCE_CHOICES: readonly { readonly value: AgentSource; readonly label: string }[] = [
  { value: 'CLAUDE_CODE', label: 'Claude Code' },
  { value: 'CODEX', label: 'Codex' },
];

export const DEFAULT_QUERY: UsageQuery = { days: 30, count: 'cost', device: null, source: null, repository: null };

const DAY_MS = 86_400_000;

/** The address is typed by hand, so unknown values fall back to the defaults. */
export function parseUsageQuery(params: URLSearchParams): UsageQuery {
  const days = DAY_CHOICES.find((choice) => String(choice) === params.get('days')) ?? DEFAULT_QUERY.days;
  const count = COUNT_CHOICES.find((choice) => choice.value === params.get('count'))?.value ?? DEFAULT_QUERY.count;
  const source = SOURCE_CHOICES.find((choice) => choice.value === params.get('source'))?.value ?? null;
  return { days, count, device: params.get('device') || null, source, repository: params.get('repository') || null };
}

export function writeUsageQuery(query: UsageQuery): URLSearchParams {
  const params = new URLSearchParams();
  if (query.days !== DEFAULT_QUERY.days) params.set('days', String(query.days));
  if (query.count !== DEFAULT_QUERY.count) params.set('count', query.count);
  if (query.device !== null) params.set('device', query.device);
  if (query.source !== null) params.set('source', query.source);
  if (query.repository !== null) params.set('repository', query.repository);
  return params;
}

export function utcDay(moment: number): string {
  return new Date(moment).toISOString().slice(0, 10);
}

/** The days end today in UTC (AIS-020); what is counted is the page's choice, not the hub's. */
export function usageRequest(query: UsageQuery, now: Date): UsageRequest {
  const filter: AgentUsageFilter = {
    firstDay: utcDay(now.getTime() - (query.days - 1) * DAY_MS),
    lastDay: utcDay(now.getTime()),
    ...(query.device === null ? {} : { deviceId: query.device }),
    ...(query.source === null ? {} : { source: query.source }),
    ...(query.repository === null ? {} : { repository: query.repository }),
  };
  return { filter };
}
