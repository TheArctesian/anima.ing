import { graphql } from '$lib/hub/generated';
import type { OsUpdateAction } from '$lib/hub/generated/graphql';
import type { Hub } from '$lib/hub/hub';

const Status = graphql(`
  query DeviceOsUpdateStatus($deviceId: ID!) {
    deviceOsUpdate(deviceId: $deviceId)
  }
`);

const Request = graphql(`
  mutation DeviceOsUpdateRequest($deviceId: ID!, $action: OsUpdateAction!, $confirmed: Boolean!, $reviewRevision: String) {
    requestDeviceOsUpdate(deviceId: $deviceId, input: { action: $action, confirmed: $confirmed, reviewRevision: $reviewRevision })
  }
`);

export function osUpdatesApi(hub: Hub) {
  return {
    status: (deviceId: string) => hub.request(Status, { deviceId }),
    request: (deviceId: string, action: OsUpdateAction, confirmed: boolean, reviewRevision: string | null) => hub.request(Request, { deviceId, action, confirmed, reviewRevision }),
  };
}

export type OsUpdateCalls = ReturnType<typeof osUpdatesApi>;
