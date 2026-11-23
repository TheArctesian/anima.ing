import { latestBeats, type Beat } from '$lib/checks/heartbeats';
import { graphql } from '$lib/hub/generated';
import type { Hub, HubResult } from '$lib/hub/hub';

const DeviceBeats = graphql(`
  query DeviceBeats($deviceId: ID!) {
    device(id: $deviceId) {
      id
      checks {
        id
        heartbeats(first: 50) {
          edges {
            node {
              at
              status
              responseTimeMs
            }
          }
        }
      }
    }
  }
`);

export async function deviceBeats(hub: Hub, deviceId: string): Promise<HubResult<readonly Beat[]>> {
  const result = await hub.request(DeviceBeats, { deviceId });
  return result.ok ? { ok: true, value: latestBeats(result.value.device?.checks ?? []) } : result;
}
