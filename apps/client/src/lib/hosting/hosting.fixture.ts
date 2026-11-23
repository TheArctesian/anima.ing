import { container } from '$lib/containers/containers.fixture';
import type { Container, ImageUpdate } from '$lib/containers/api';
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

export interface FakeHosting extends HostingCalls {
  readonly containersAsked: string[];
  readonly updatesAsked: string[];
  /** A machine whose containers answer with this error. */
  failing: Record<string, string>;
}

const ok = <T>(value: T): HubResult<T> => ({ ok: true, value });

export function fakeHosting(updates: readonly ImageUpdate[] = []): FakeHosting {
  const fake: FakeHosting = {
    containersAsked: [],
    updatesAsked: [],
    failing: {},
    machines: () => Promise.resolve(ok(MACHINES)),
    containers: (id) => {
      fake.containersAsked.push(id);
      const problem = fake.failing[id];
      const result: HubResult<readonly Container[]> = problem === undefined ? ok(CONTAINERS[id] ?? []) : { ok: false, error: { code: 'CONFLICT', message: problem } };
      return Promise.resolve(result);
    },
    updates: (id) => {
      fake.updatesAsked.push(id);
      return Promise.resolve(ok(updates));
    },
  };
  return fake;
}
