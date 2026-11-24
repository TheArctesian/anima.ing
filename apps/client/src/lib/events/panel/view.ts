import type { EventSource, Severity } from '$lib/hub/generated/graphql';
import { toggled } from '$lib/devices/toolbar/toolbar';
import type { IconName } from '$lib/icons/icons';
import type { LogFilter, Since } from '../log';

export interface Choice<T extends string> {
  readonly value: T;
  readonly label: string;
}

export const SINCE_CHOICES: readonly Choice<Since>[] = [
  { value: '1h', label: '1 h' },
  { value: '24h', label: '24 h' },
  { value: '7d', label: '7 d' },
  { value: 'all', label: 'All' },
];

export const SEVERITY_CHOICES: readonly Choice<Severity>[] = [
  { value: 'ERROR', label: 'Error' },
  { value: 'WARNING', label: 'Warning' },
  { value: 'INFO', label: 'Info' },
];

/** In the order of the filter; a record, so a source that the hub adds fails the type check here. */
const SOURCE_WORDS: Readonly<Record<EventSource, string>> = {
  CHECK: 'Check',
  SESSION: 'Session',
  DEVICE: 'Device',
  ACCESS: 'Access',
  HUB: 'Hub',
  VPN: 'VPN',
  METRICS: 'Metrics',
  NOTIFICATION: 'Notification',
};

export const SOURCE_CHOICES: readonly Choice<EventSource>[] = (Object.keys(SOURCE_WORDS) as EventSource[]).map(
  (value) => ({ value, label: SOURCE_WORDS[value] }),
);

/** The mark beside a source's word; the word stays, so the icon is never the only sign (UR-7). */
export const SOURCE_ICONS: Readonly<Record<EventSource, IconName>> = {
  CHECK: 'carbon:task-complete',
  SESSION: 'carbon:terminal',
  DEVICE: 'carbon:bare-metal-server',
  ACCESS: 'carbon:security',
  HUB: 'carbon:network-3',
  VPN: 'carbon:vpn',
  METRICS: 'carbon:activity',
  NOTIFICATION: 'carbon:notification',
};

export function sourceIcon(source: string): IconName {
  return (SOURCE_ICONS as Partial<Record<string, IconName>>)[source] ?? 'carbon:catalog';
}

function sentence(value: string): string {
  const words = value.replaceAll('_', ' ').toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

export function labelOf<T extends string>(choices: readonly Choice<T>[], value: T): string {
  return choices.find((choice) => choice.value === value)?.label ?? sentence(value);
}

const pad = (part: number): string => String(part).padStart(2, '0');

export function clockLabel(at: string): string {
  const time = new Date(at);
  if (Number.isNaN(time.getTime())) return at;
  return `${pad(time.getHours())}:${pad(time.getMinutes())}:${pad(time.getSeconds())}`;
}

export function timeLabel(at: string, now: Date): string {
  const time = new Date(at);
  if (Number.isNaN(time.getTime())) return at;
  const clock = clockLabel(at);
  return time.toDateString() === now.toDateString() ? clock : `${pad(time.getMonth() + 1)}-${pad(time.getDate())} ${clock}`;
}

const DAY = new Intl.DateTimeFormat('en-GB', { weekday: 'short', day: 'numeric', month: 'short' });

export function dayLabel(at: string, now: Date): string {
  const time = new Date(at);
  if (Number.isNaN(time.getTime())) return '';
  if (time.toDateString() === now.toDateString()) return 'Today';
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  return time.toDateString() === yesterday.toDateString() ? 'Yesterday' : DAY.format(time);
}

export function deviceLabel(deviceName: string | null | undefined): string {
  return deviceName ?? '—';
}

export function switched<T extends string>(
  choices: readonly Choice<T>[],
  current: readonly T[],
  value: string,
): readonly T[] {
  const choice = choices.find((known) => known.value === value);
  return choice === undefined ? current : toggled(current, choice.value);
}

export function withSince(filter: LogFilter, value: string): LogFilter {
  const since = SINCE_CHOICES.find((known) => known.value === value)?.value;
  return since === undefined ? filter : { ...filter, since };
}

export function withDevice(filter: LogFilter, deviceId: string): LogFilter {
  return { ...filter, deviceId: deviceId === '' ? null : deviceId };
}
