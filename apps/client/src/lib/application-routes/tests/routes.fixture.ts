import type { HubError, HubResult } from '$lib/hub/hub';
import type { ApplicationRoute, RouteCalls, RouteList } from '../api';

// Test support: routes in the hub's shape, and calls that answer with them and record what the section asked.

/** The time of each test: the certificates below end 9 and 89 days after it. */
export const NOW = new Date('2026-11-21T12:00:00Z');

export const ATLAS = 'device-atlas';
export const NAS = 'device-nas';

export function route(change: Partial<ApplicationRoute>): ApplicationRoute {
  return {
    id: 'route-vault',
    name: 'vault.example.com',
    kind: 'WEB',
    access: 'PRIVATE',
    applicationId: 'app-vaultwarden',
    deviceId: ATLAS,
    port: 80,
    listenPort: null,
    protocol: null,
    redirectTo: null,
    proof: 'PROVEN',
    failedReads: 0,
    proofRecordName: '_anima.vault.example.com',
    proofRecordValue: 'anima-route=hub_1.token',
    machineConsent: false,
    certificateNotAfter: '2027-02-18T12:00:00Z',
    waitsFor: 'MACHINE_CONSENT',
    checkedAt: '2026-11-21T11:58:00Z',
    requests: 220,
    bytesIn: 1_000_000,
    bytesOut: 9_000_000,
    refused: 0,
    createdAt: '2026-11-20T09:00:00Z',
    ...change,
  };
}

/** The routes of a hosted hub, as in Frame 1: one proven, one with a certificate that ends soon, one waiting, one lost, one redirect. */
export const ROUTES: readonly ApplicationRoute[] = [
  route({}),
  route({ id: 'route-tracks', name: 'tracks.example.com', deviceId: NAS, port: 3000, certificateNotAfter: '2026-11-30T13:00:00Z', requests: 1900, bytesOut: 84_000_000 }),
  route({ id: 'route-wiki', name: 'wiki.example.com', deviceId: NAS, port: 3000, proof: 'WAITING', proofRecordName: '_anima.wiki.example.com', proofRecordValue: 'anima-route=hub_1.wiki', certificateNotAfter: null, waitsFor: 'PROOF', checkedAt: null, requests: 0, bytesOut: 0 }),
  route({ id: 'route-media', name: 'media.example.org', access: 'PUBLIC', deviceId: NAS, port: 8096, proof: 'LOST', failedReads: 3, waitsFor: 'PROOF', refused: 12 }),
  route({ id: 'route-code', name: 'code.example.com', kind: 'REDIRECT', access: 'PUBLIC', port: null, redirectTo: 'https://git.example.com' }),
];

export const DEVICE_NAMES: ReadonlyMap<string, string> = new Map([
  [ATLAS, 'atlas'],
  [NAS, 'nas'],
]);

export interface FakeCalls extends RouteCalls {
  listAsked: number;
  checked: string[];
  removed: string[];
  routes: readonly ApplicationRoute[];
  /** When set, the next list fails with it. */
  listFails?: HubError;
  /** When set, each check fails with it. */
  checkFails?: HubError;
  /** When set, each removal fails with it. */
  removeFails?: HubError;
  /** What a check answers, by the ID of the route; the route as it was by default. */
  checkAnswers: Map<string, ApplicationRoute>;
}

export function fakeCalls(routes: readonly ApplicationRoute[] = ROUTES): FakeCalls {
  const calls: FakeCalls = {
    listAsked: 0,
    checked: [],
    removed: [],
    routes,
    checkAnswers: new Map(),
    list(): Promise<HubResult<RouteList>> {
      calls.listAsked += 1;
      if (calls.listFails !== undefined) return Promise.resolve({ ok: false, error: calls.listFails });
      return Promise.resolve({ ok: true, value: { routes: calls.routes, deviceNames: DEVICE_NAMES } });
    },
    check(id: string): Promise<HubResult<ApplicationRoute>> {
      calls.checked.push(id);
      if (calls.checkFails !== undefined) return Promise.resolve({ ok: false, error: calls.checkFails });
      const answer = calls.checkAnswers.get(id) ?? calls.routes.find((each) => each.id === id);
      return Promise.resolve(answer === undefined ? { ok: false, error: { code: 'NOT_FOUND', message: 'No such route.' } } : { ok: true, value: answer });
    },
    remove(id: string): Promise<HubResult<string>> {
      calls.removed.push(id);
      if (calls.removeFails !== undefined) return Promise.resolve({ ok: false, error: calls.removeFails });
      return Promise.resolve({ ok: true, value: id });
    },
  };
  return calls;
}

/** The hub's refusal on a hub that is not on Linux (R-366). */
export const NOT_LINUX_ERROR: HubError = { code: 'UNAVAILABLE', message: 'Routing needs a hub on Linux.' };

/** The hub's refusal for an account with no second factor (SF-232). */
export const NO_SECOND_FACTOR: HubError = { code: 'FORBIDDEN', message: 'Turn on the second factor of the account before an application is made public.' };

/** The hub's answer to a route that is gone already. */
export const NO_SUCH_ROUTE: HubError = { code: 'NOT_FOUND', message: 'No such route.' };

/** The hub's refusal of a removal by an AI agent. */
export const AGENT_REFUSED: HubError = { code: 'FORBIDDEN', message: 'A route is removed only by a person, never by an AI agent.' };
