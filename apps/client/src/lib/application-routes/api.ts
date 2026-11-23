import { graphql } from '$lib/hub/generated';
import type { ApplicationRoutesQuery } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

// The fragment is only read by codegen, which inlines it into the documents that spread it.
void graphql(`
  fragment RouteFields on ApplicationRoute {
    id
    name
    kind
    access
    applicationId
    deviceId
    port
    listenPort
    protocol
    redirectTo
    proof
    failedReads
    proofRecordName
    proofRecordValue
    machineConsent
    certificateNotAfter
    waitsFor
    checkedAt
    requests
    bytesIn
    bytesOut
    refused
    createdAt
  }
`);

// A route names its device only by its ID; 200 is the hub's page size.
const APPLICATION_ROUTES = graphql(`
  query ApplicationRoutes {
    applicationRoutes {
      ...RouteFields
    }
    devices(first: 200) {
      edges {
        node {
          id
          name
        }
      }
    }
  }
`);

const CHECK_APPLICATION_ROUTE = graphql(`
  mutation CheckApplicationRoute($id: ID!) {
    checkApplicationRoute(id: $id) {
      ...RouteFields
    }
  }
`);

const REMOVE_APPLICATION_ROUTE = graphql(`
  mutation RemoveApplicationRoute($id: ID!) {
    removeApplicationRoute(id: $id)
  }
`);

export type ApplicationRoute = ApplicationRoutesQuery['applicationRoutes'][number];

export interface RouteList {
  readonly routes: readonly ApplicationRoute[];
  readonly deviceNames: ReadonlyMap<string, string>;
}

export interface RouteCalls {
  /** A read key may read them: no field holds a certificate or a key. */
  list(): Promise<HubResult<RouteList>>;
  /** A person with a browser token and a second factor only (SF-232). */
  check(id: string): Promise<HubResult<ApplicationRoute>>;
  /** Removes the name with its certificate and its key. A signed-in person; no second factor (ruling of 2026-11-22). */
  remove(id: string): Promise<HubResult<string>>;
}

export function routesApi(hub: Pick<Hub, 'request'>): RouteCalls {
  return {
    list: async () => {
      const result = await hub.request(APPLICATION_ROUTES, {});
      if (!result.ok) return result;
      const deviceNames = new Map(result.value.devices.edges.map(({ node }) => [node.id, node.name]));
      return { ok: true, value: { routes: result.value.applicationRoutes, deviceNames } };
    },
    check: async (id) => {
      const result = await hub.request(CHECK_APPLICATION_ROUTE, { id });
      return result.ok ? { ok: true, value: result.value.checkApplicationRoute } : result;
    },
    remove: async (id) => {
      const result = await hub.request(REMOVE_APPLICATION_ROUTE, { id });
      return result.ok ? { ok: true, value: result.value.removeApplicationRoute } : result;
    },
  };
}
