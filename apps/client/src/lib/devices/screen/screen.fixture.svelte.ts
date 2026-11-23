import type { CardDevice } from '../api';
import { DEFAULT_QUERY, type ListQuery } from '../query';

export const url: { query: ListQuery; writes: ListQuery[] } = $state({ query: { ...DEFAULT_QUERY, view: 'grid', sort: 'NAME' }, writes: [] });

export function device(id: string, over: Partial<CardDevice> = {}): CardDevice {
  return {
    id,
    name: `Device ${id}`,
    host: `${id}.lan`,
    ip: null,
    kind: 'server',
    os: 'OTHER',
    distribution: null,
    tags: [],
    maintenance: false,
    status: 'UP',
    uptime: 0.9995,
    macAddress: null,
    metricsSource: 'NONE',
    readings: null,
    batteries: [],
    accessWarnings: [],
    rdp: { enabled: false },
    vnc: { enabled: false, username: null },
    ssh: { enabled: false, hostKey: null },
    checks: [],
    webInterfaces: [],
    ...over,
  };
}

export function devicesAnswer(devices: readonly CardDevice[], next: string | null = null) {
  return {
    devices: {
      totalCount: devices.length,
      pageInfo: { hasNextPage: next !== null, endCursor: next },
      edges: devices.map((node) => ({ node })),
    },
  };
}

export const CARDS: ListQuery = { ...DEFAULT_QUERY, view: 'grid', sort: 'NAME' };
