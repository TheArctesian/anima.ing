import { formatBytes } from '$lib/metrics/units';

/** From the pool's answer (`contracts/account-usage.md`, HMD-073); the hub holds none of it. */
export interface AccountUsage {
  readonly at: Date;
  /** The month, `YYYY-MM`. */
  readonly month: string;
  readonly plan: { readonly name: string; readonly charged: boolean };
  readonly awakeSeconds: number;
  readonly asleepSeconds: number;
  /** Before tax, in minor units; `null` when the pool gives no prices or nothing is charged. */
  readonly amount: { readonly minor: number; readonly currency: string } | null;
  readonly disk: { readonly limitBytes: number; readonly usedBytes: number | null };
  readonly owes: boolean;
}

export type UsageAnswer =
  | { readonly state: 'figures'; readonly usage: AccountUsage }
  | { readonly state: 'signed-out' }
  | { readonly state: 'unavailable' };

export const ACCOUNT_PAGE = 'https://cloud.anima.ing/account';

export const ESTIMATE_NOTE = 'So far this month, before tax. This is an estimate, not a bill. Your invoice arrives after the month ends.';
export const NOT_CHARGED = 'Nothing is charged on this plan';
export const NOT_AVAILABLE = 'Not available just now';
export const SIGNED_OUT = 'Sign in to your Anima.ing account to see the figures of your plan.';
export const OWES = 'Your account has an amount due from an earlier month.';

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;
const isCount = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const isName = (value: unknown): value is string => typeof value === 'string' && value !== '';

interface PoolHub {
  readonly id: string;
  readonly plan: string;
  readonly limitBytes: number;
  readonly usedBytes: number | null;
}

function poolHub(value: unknown): PoolHub | undefined {
  if (!isRecord(value) || !isName(value.id) || !isName(value.plan) || !isCount(value.disk_limit_bytes)) return undefined;
  if (value.disk_used_bytes !== null && !isCount(value.disk_used_bytes)) return undefined;
  return { id: value.id, plan: value.plan, limitBytes: value.disk_limit_bytes, usedBytes: value.disk_used_bytes };
}

function hubOfPage(value: unknown, hubId: string | undefined): PoolHub | undefined {
  if (!Array.isArray(value)) return undefined;
  const hubs = value.map(poolHub);
  if (hubs.some((hub) => hub === undefined)) return undefined;
  return hubs.find((hub) => hub?.id === hubId) ?? (hubs.length === 1 ? hubs[0] : undefined);
}

function seconds(value: unknown): { awake: number; asleep: number } | undefined {
  if (!Array.isArray(value)) return undefined;
  const sums = { awake: 0, asleep: 0 };
  for (const line of value as unknown[]) {
    if (!isRecord(line) || !isCount(line.seconds) || (line.kind !== 'running' && line.kind !== 'asleep')) return undefined;
    if (line.kind === 'running') sums.awake += line.seconds;
    else sums.asleep += line.seconds;
  }
  return sums;
}

const planName = (plan: string): string => plan.charAt(0).toUpperCase() + plan.slice(1);

/** Checks only the members the page uses; others are ignored. */
export function parseUsage(value: unknown, hubId?: string): AccountUsage | undefined {
  if (!isRecord(value) || !isCount(value.until) || typeof value.period !== 'string' || !/^\d{4}-\d{2}$/.test(value.period)) return undefined;
  const [hub, counted] = [hubOfPage(value.hubs, hubId), seconds(value.lines)];
  if (hub === undefined || counted === undefined || !isCount(value.total) || typeof value.prices_set !== 'boolean') return undefined;
  if (typeof value.currency !== 'string' || !/^[a-zA-Z]{3}$/.test(value.currency)) return undefined;
  const charged = hub.plan !== 'free';
  const amount = charged && value.prices_set ? { minor: value.total, currency: value.currency.toUpperCase() } : null;
  return {
    at: new Date(value.until * 1000),
    month: value.period,
    plan: { name: planName(hub.plan), charged },
    awakeSeconds: counted.awake,
    asleepSeconds: counted.asleep,
    amount,
    disk: { limitBytes: hub.limitBytes, usedBytes: hub.usedBytes },
    owes: value.owes === true,
  };
}

export function hoursText(seconds: number): string {
  const hours = seconds / 3600;
  return hours < 10 ? `${(Math.floor(hours * 10) / 10).toString()} h` : `${Math.floor(hours).toString()} h`;
}

export function amountText(value: NonNullable<AccountUsage['amount']>, locale?: string): string {
  const format = new Intl.NumberFormat(locale, { style: 'currency', currency: value.currency });
  const digits = format.resolvedOptions().maximumFractionDigits ?? 2;
  return format.format(value.minor / 10 ** digits);
}

export function planDiskText(value: AccountUsage['disk']): string {
  const limit = formatBytes(value.limitBytes);
  if (value.usedBytes === null) return `Limit ${limit}`;
  const share = value.limitBytes === 0 ? 0 : Math.round((value.usedBytes / value.limitBytes) * 100);
  return `${formatBytes(value.usedBytes)} of ${limit} (${share.toString()}%)`;
}

export function figuresTime(at: Date, now: Date): string {
  const minutes = Math.floor((now.getTime() - at.getTime()) / 60_000);
  if (minutes < 1) return 'Figures from just now';
  if (minutes < 60) return `Figures from ${minutes.toString()} min ago`;
  return `Figures from ${at.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })}`;
}

/** HMD-072. */
export function hubDataBytes(series: readonly { readonly kind: string; readonly points: readonly { readonly value: number | null }[] }[]): number | null {
  const bytes = series.find((one) => one.kind === 'DISK_BYTES');
  const values = (bytes?.points ?? []).map((point) => point.value).filter((value): value is number => value !== null);
  return values.at(-1) ?? null;
}
