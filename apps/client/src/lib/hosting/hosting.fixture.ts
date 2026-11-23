import { container, fakeCalls, type FakeCalls } from '$lib/containers/containers.fixture';
import type { Container, ContainerStorage, ContainerVolume, ImageUpdate } from '$lib/containers/api';
import type { HubResult } from '$lib/hub/hub';
import type { HostingCalls, Machine } from './api';

// Test support: three machines in the hub's shape, and calls that answer with them and record what the list asked.

export const MACHINES: readonly Machine[] = [
  { id: 'm-pop', name: 'pop-os', state: 'UP' },
  { id: 'm-nas', name: 'nas', state: 'UP' },
  { id: 'm-mini', name: 'mini', state: 'DOWN' },
];

export const CONTAINERS: Readonly<Record<string, readonly Container[]>> = {
  'm-pop': [
    container({ id: 'a'.repeat(64), name: 'immich-server', composeProject: 'immich', composeService: 'server', composeProjectState: 'RUNNING' }),
    container({ id: 'b'.repeat(64), name: 'immich-redis', composeProject: 'immich', composeService: 'redis', composeProjectState: 'RUNNING' }),
    container({ id: 'c'.repeat(64), name: 'watchtower', image: 'containrrr/watchtower', composeProject: null, composeService: null, composeProjectState: null }),
  ],
  'm-nas': [container({ id: 'd'.repeat(64), name: 'samba', image: 'example/samba', state: 'EXITED', status: 'Exited (0) 1 hour ago', health: 'NONE', composeProject: null, composeService: null, composeProjectState: null })],
  'm-mini': [],
};

const volume = (name: string, size: string, containers: string[]): ContainerVolume => ({ name, driver: 'local', size, containerCount: containers.length, containers });

/** HSE-015: the volumes of each machine; one of pop-os and the one of nas are used by no container. */
export const VOLUMES: Readonly<Record<string, ContainerVolume[]>> = {
  'm-pop': [volume('immich_pgdata', '1.2GB', ['immich-server', 'immich-redis']), { ...volume('watchtower-cache', '12kB', ['watchtower']), driver: 'nfs' }, volume('old-photos', '38.4GB', [])],
  'm-nas': [volume('samba-shares', '0B', [])],
  'm-mini': [],
};

/** The machine of a device page (HSE-008). */
export const DEVICE: Machine = { id: 'device-1', name: 'nas', state: 'UP' };

/** The calls of the device tab over the fake calls of one device: its containers, newer images, log and details. */
export function deviceHosting(calls: FakeCalls): HostingCalls {
  return { machines: () => Promise.resolve(ok([])), containers: () => calls.list(), updates: () => calls.updates(), machine: () => calls, storage: () => calls.storage() };
}

export interface FakeHosting extends HostingCalls {
  readonly containersAsked: string[];
  readonly updatesAsked: string[];
  readonly storageAsked: string[];
  /** A machine whose containers and storage answer with this error. */
  failing: Record<string, string>;
  /** The log and the details of each machine, and the machines they were asked of. */
  readonly logs: FakeCalls;
  readonly logMachines: string[];
}

const ok = <T>(value: T): HubResult<T> => ({ ok: true, value });

export function fakeHosting(updates: readonly ImageUpdate[] = []): FakeHosting {
  const fake: FakeHosting = {
    containersAsked: [],
    updatesAsked: [],
    storageAsked: [],
    failing: {},
    logs: fakeCalls(),
    logMachines: [],
    machines: () => Promise.resolve(ok(MACHINES)),
    containers: (id) => {
      fake.containersAsked.push(id);
      const problem = fake.failing[id];
      const result: HubResult<readonly Container[]> = problem === undefined ? ok(CONTAINERS[id] ?? []) : { ok: false, error: { code: 'CONFLICT', message: problem } };
      return Promise.resolve(result);
    },
    machine: (id) => {
      fake.logMachines.push(id);
      return fake.logs;
    },
    updates: (id) => {
      fake.updatesAsked.push(id);
      return Promise.resolve(ok(updates));
    },
    storage: (id) => {
      fake.storageAsked.push(id);
      const problem = fake.failing[id];
      const result: HubResult<ContainerStorage> = problem === undefined ? ok({ images: [], volumes: VOLUMES[id] ?? [], usage: id === 'm-pop' ? [{ type: 'Local Volumes', totalCount: 3, activeCount: 2, size: '39.6GB', reclaimable: '38.4GB (96%)' }] : [] }) : { ok: false, error: { code: 'CONFLICT', message: problem } };
      return Promise.resolve(result);
    },
  };
  return fake;
}
