import type { DeviceSort, Status } from '$lib/hub/generated/graphql';
import type { CardDevice, DevicePage, ReadingsChange, StatusChange } from './api';
import type { ListQuery } from './query';

export interface DeviceList {
  readonly devices: readonly CardDevice[];
  readonly totalCount: number;
  readonly endCursor: string | null;
  readonly hasNextPage: boolean;
}

export function fromPage(page: DevicePage): DeviceList {
  return {
    devices: page.edges.map((edge) => edge.node),
    totalCount: page.totalCount,
    endCursor: page.pageInfo.endCursor ?? null,
    hasNextPage: page.pageInfo.hasNextPage,
  };
}

/** A listed device keeps its entry, since its live status may be newer. */
export function appendPage(list: DeviceList, page: DevicePage): DeviceList {
  const listed = new Set(list.devices.map((device) => device.id));
  const added = page.edges.map((edge) => edge.node).filter((device) => !listed.has(device.id));
  return {
    devices: [...list.devices, ...added],
    totalCount: page.totalCount,
    endCursor: page.pageInfo.endCursor ?? null,
    hasNextPage: page.pageInfo.hasNextPage,
  };
}

/** After the R-079 flow turned VNC on. */
export function applyVncEnabled(list: DeviceList, id: string): DeviceList {
  return { ...list, devices: list.devices.map((device) => (device.id === id ? { ...device, vnc: { ...device.vnc, enabled: true } } : device)) };
}

/** Worst first (E-1): the hub's severity order. */
const SEVERITY: Record<Status, number> = { DOWN: 0, DEGRADED: 1, UNKNOWN: 2, MAINTENANCE: 3, UP: 4 };

/** Code-point order matches the hub's UTF-8 byte order (WP-06); `<` on UTF-16 code units does not. */
function compareNames(a: string, b: string): number {
  const left = Array.from(a, (char) => char.codePointAt(0) ?? 0);
  const right = Array.from(b, (char) => char.codePointAt(0) ?? 0);
  const shared = Math.min(left.length, right.length);
  for (let index = 0; index < shared; index += 1) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return left.length - right.length;
}

function compareUptime(a: number | null | undefined, b: number | null | undefined): number {
  if (a == null || b == null) return Number(a == null) - Number(b == null);
  return a - b;
}

function primaryOrder(sort: DeviceSort, a: CardDevice, b: CardDevice): number {
  switch (sort) {
    case 'STATUS':
      return SEVERITY[a.status] - SEVERITY[b.status];
    case 'UPTIME':
      return compareUptime(a.uptime, b.uptime);
    case 'NAME':
      return 0;
  }
}

/** The order of `sort` as the hub applies it (E-1); ties break by name in byte order. */
export function compareDevices(sort: DeviceSort): (a: CardDevice, b: CardDevice) => number {
  return (a, b) => primaryOrder(sort, a, b) || compareNames(a.name, b.name);
}

/** A push carries no device record, so a device cannot enter the list here; the page fetches again (DEV-003, DEV-004). */
export function applyStatus(list: DeviceList, change: StatusChange, query: ListQuery): DeviceList {
  if (!list.devices.some((device) => device.id === change.deviceId)) return list;
  const leaves = query.statuses.length > 0 && !query.statuses.includes(change.status);
  const changed = list.devices.flatMap((device) => {
    if (device.id !== change.deviceId) return [device];
    return leaves ? [] : [{ ...device, status: change.status }];
  });
  const reorders = query.sort === 'STATUS' || query.sort === 'UPTIME';
  return {
    ...list,
    devices: reorders ? changed.toSorted(compareDevices(query.sort)) : changed,
    totalCount: leaves ? list.totalCount - 1 : list.totalCount,
  };
}

/** Readings change no order or filter (MET-004). */
export function applyReadings(list: DeviceList, change: ReadingsChange): DeviceList {
  if (!list.devices.some((device) => device.id === change.deviceId)) return list;
  return {
    ...list,
    devices: list.devices.map((device) => (device.id === change.deviceId ? { ...device, readings: change.readings } : device)),
  };
}
