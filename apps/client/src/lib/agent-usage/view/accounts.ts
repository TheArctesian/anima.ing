import type { AccountLimit, AgentAccount, DeviceSourceRow } from '../api';
import type { Count } from '../query';
import { countOf, share } from './shape';

// The part "By account" (AIS-022, R-313).

const NUMBER_WORDS: Readonly<Record<string, number>> = { one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7 };
const MINUTES: Readonly<Record<string, number>> = { min: 1, minute: 1, hour: 60, day: 1440, week: 10_080 };

function counted(count: number, unit: string): string {
  return `${String(count)} ${unit}${count === 1 ? '' : 's'}`;
}

function lengthWords(minutes: number): string {
  if (minutes % 1440 === 0) return counted(minutes / 1440, 'day');
  if (minutes % 60 === 0) return counted(minutes / 60, 'hour');
  return counted(minutes, 'minute');
}

export function windowWords(window: string): string {
  if (window === 'weekly') return lengthWords(MINUTES.week ?? 0);
  if (window === 'daily') return lengthWords(MINUTES.day ?? 0);
  const match = /^(\d+|[a-z]+)[_ ](min|minute|hour|day|week)s?$/.exec(window);
  const amount = match === null ? undefined : (NUMBER_WORDS[match[1] ?? ''] ?? Number(match[1]));
  const unit = match === null ? undefined : MINUTES[match[2] ?? ''];
  if (amount === undefined || unit === undefined || !Number.isInteger(amount) || amount <= 0) {
    const words = window.replaceAll('_', ' ');
    return words.charAt(0).toUpperCase() + words.slice(1);
  }
  return lengthWords(amount * unit);
}

export function limitLevel(limit: Pick<AccountLimit, 'usedPercent' | 'reached'>): 'reached' | 'high' | 'normal' {
  if (limit.reached) return 'reached';
  return (limit.usedPercent ?? 0) >= 80 ? 'high' : 'normal';
}

export function limitFill(limit: Pick<AccountLimit, 'usedPercent' | 'reached'>): number {
  return limit.reached ? 100 : Math.min(100, Math.max(0, limit.usedPercent ?? 0));
}

const DAY_MS = 86_400_000;

export function resetWords(resetsAt: string, now: number, zone?: string): string {
  const at = Date.parse(resetsAt);
  const day = (time: number) => new Intl.DateTimeFormat('en-CA', { timeZone: zone, dateStyle: 'short' }).format(time);
  const clock = new Intl.DateTimeFormat('en-GB', { timeZone: zone, hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(at);
  if (day(at) === day(now)) return clock;
  if (at - now < 6 * DAY_MS) return `${new Intl.DateTimeFormat('en-GB', { timeZone: zone, weekday: 'short' }).format(at)} ${clock}`;
  return new Intl.DateTimeFormat('en-GB', { timeZone: zone, day: 'numeric', month: 'short' }).format(at);
}

export function limitText(limit: AccountLimit, now: number, zone?: string): string {
  const used = limit.reached ? 'Limit reached' : limit.usedPercent === null ? 'Not reached' : `${String(limit.usedPercent)} % used`;
  return limit.resetsAt === null ? used : `${used} · starts again ${resetWords(limit.resetsAt, now, zone)}`;
}

export interface DeviceShare {
  readonly id: string;
  readonly name: string;
  /** `null` when the account has no use in these days or this device is not counted. */
  readonly percent: number | null;
  /** False when the daemon reads no sessions and there is no use: unknown use is not no use (AIS-024). */
  readonly counted: boolean;
}

export function deviceShares(account: Pick<AgentAccount, 'source' | 'devices'>, rows: readonly DeviceSourceRow[], count: Count, notCounted: ReadonlySet<string> = new Set()): DeviceShare[] {
  const found = account.devices.map((device) => rows.find((one) => one.device.id === device.id && one.source === account.source));
  const counts = found.map((row) => (row === undefined ? 0 : countOf(row, count)));
  const sum = counts.reduce((all, one) => all + one, 0);
  return account.devices.map((device, index) => {
    // A device with use in these days has its share, also when its daemon reads no sessions now: the use is from before.
    const counted = found[index] !== undefined || !notCounted.has(device.id);
    return { id: device.id, name: device.name, counted, percent: sum === 0 || !counted ? null : Math.round(share(counts[index] ?? 0, sum) * 100) };
  });
}
