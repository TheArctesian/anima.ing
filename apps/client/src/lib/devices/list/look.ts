import type { Beat } from '$lib/checks/heartbeats';
import type { Status } from '$lib/hub/generated/graphql';
import type { CardDevice } from '../api';
import type { ListQuery } from '../query';

export type LookDevice = Pick<CardDevice, 'id' | 'status' | 'checks'>;

/** The groups of a list sorted by status (UR-7, D18), in the hub's STATUS order. */
export type GroupKey = 'look' | 'maintenance' | 'up';

const GROUP_TITLES: Record<GroupKey, string> = { look: 'Needs a look', maintenance: 'In maintenance', up: 'Up' };

export function groupOf(status: Status): GroupKey {
  if (status === 'UP') return 'up';
  return status === 'MAINTENANCE' ? 'maintenance' : 'look';
}

export type ListEntry<T> =
  | { readonly kind: 'group'; readonly key: string; readonly title: string; readonly count: number }
  | { readonly kind: 'device'; readonly key: string; readonly device: T };

/** Grouped only when sorted by status and not filtered by it, so the problems read first without a second list (D18). */
export function listItems<T extends LookDevice>(devices: readonly T[], query: Pick<ListQuery, 'sort' | 'statuses'>): readonly ListEntry<T>[] {
  const rows = devices.map((device) => ({ kind: 'device' as const, key: device.id, device }));
  if (!grouped(query)) return rows;
  const counts = new Map<GroupKey, number>();
  for (const device of devices) counts.set(groupOf(device.status), (counts.get(groupOf(device.status)) ?? 0) + 1);
  const items: ListEntry<T>[] = [];
  let open: GroupKey | null = null;
  for (const row of rows) {
    const group = groupOf(row.device.status);
    if (group !== open) {
      items.push({ kind: 'group', key: `group:${group}`, title: GROUP_TITLES[group], count: counts.get(group) ?? 0 });
      open = group;
    }
    items.push(row);
  }
  return items;
}

export function grouped({ sort, statuses }: Pick<ListQuery, 'sort' | 'statuses'>): boolean {
  return sort === 'STATUS' && statuses.length === 0;
}

/** The count line's statuses, worst first, each with the words after its number. */
const TALLY: readonly (readonly [Status, string])[] = [
  ['DOWN', 'down'],
  ['DEGRADED', 'degraded'],
  ['UNKNOWN', 'unknown'],
  ['MAINTENANCE', 'in maintenance'],
  ['UP', 'up'],
];

export interface Tally {
  readonly total: number;
  readonly parts: readonly { readonly status: Status; readonly count: number; readonly words: string }[];
}

export function tallyOf(devices: readonly Pick<LookDevice, 'status'>[]): Tally {
  const parts = TALLY.map(([status, words]) => ({ status, words, count: devices.filter((device) => device.status === status).length }));
  return { total: devices.length, parts: parts.filter((part) => part.count > 0) };
}

export function tallyQuery(query: ListQuery, status: Status | null): ListQuery {
  const only = query.statuses.length === 1 ? query.statuses[0] : undefined;
  return { ...query, statuses: status === null || status === only ? [] : [status] };
}

export interface Reason {
  readonly text: string;
  /** How long it has been so, "12 min", or "over 12 min" when no kept beat is up; `null` when the beats do not say. */
  readonly since: string | null;
}

const WORDS: Record<Status, string> = { UP: 'up', DOWN: 'down', DEGRADED: 'degraded', MAINTENANCE: 'in maintenance', UNKNOWN: 'unknown' };

/** Why a device is not up (D18); `beats` are its live beats, oldest first. */
export function reasonOf(device: LookDevice, beats: readonly Beat[] | undefined, now: number): Reason | null {
  if (device.status === 'UP') return null;
  if (device.status === 'MAINTENANCE') return { text: 'In maintenance', since: null };
  if (device.checks.length === 0) return { text: 'No checks yet', since: null };
  const running = device.checks.filter((check) => !check.paused);
  if (running.length === 0) return { text: 'All checks paused', since: null };
  const worst = running.find((check) => check.status === device.status) ?? running.find((check) => check.status !== 'UP');
  if (worst === undefined) return { text: `Status is ${WORDS[device.status]}`, since: null };
  const last = worst.heartbeats.edges.at(0)?.node;
  const text = last?.message?.trim() || `${worst.name} is ${WORDS[worst.status]}`;
  return { text, since: sinceOf(beats?.filter((beat) => beat.checkId === worst.id), now) };
}

/** Only the newest beats are kept, so a run that fills them all began before the oldest. */
function sinceOf(beats: readonly Beat[] | undefined, now: number): string | null {
  if (beats === undefined || beats.length === 0) return null;
  let start: string | null = null;
  let sawUp = false;
  for (let index = beats.length - 1; index >= 0; index -= 1) {
    const beat = beats[index];
    if (beat === undefined) break;
    if (beat.status === 'UP') {
      sawUp = true;
      break;
    }
    start = beat.at;
  }
  const from = start === null ? Number.NaN : Date.parse(start);
  if (Number.isNaN(from)) return null;
  return sawUp ? durationText(now - from) : `over ${durationText(now - from)}`;
}

export function durationText(ms: number): string {
  const minutes = Math.floor(Math.max(0, ms) / 60_000);
  if (minutes < 1) return '<1 min';
  if (minutes < 90) return `${String(minutes)} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 36) return `${String(hours)} h`;
  const days = Math.round(hours / 24);
  return `${String(days)} ${days === 1 ? 'day' : 'days'}`;
}

/** A device that no check reaches gives no reading that is of now: its usage is a dash, not old values. */
export function hasLiveUsage(status: Status): boolean {
  return status !== 'DOWN' && status !== 'UNKNOWN';
}

export const UNGROUPED: Pick<ListQuery, 'sort' | 'statuses'> = { sort: 'NAME', statuses: [] };

export type UsageSlot = 'cpu' | 'memory' | 'disk' | 'vram' | 'battery';

/** GPU memory and battery appear only when a device of the list shows one (MET-008, MET-032). */
export function usageSlots(devices: readonly Pick<CardDevice, 'status' | 'readings' | 'batteries'>[]): readonly UsageSlot[] {
  const slots: UsageSlot[] = ['cpu', 'memory', 'disk'];
  const vram = devices.some((device) => hasLiveUsage(device.status) && device.readings?.vramGpu?.hasVram === true && device.readings.vramPercent !== null);
  if (vram) slots.push('vram');
  if (devices.some((device) => device.batteries.length > 0)) slots.push('battery');
  return slots;
}
