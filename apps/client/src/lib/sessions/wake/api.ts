import { graphql } from '$lib/hub/generated';
import type { WakeDeviceMutation } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

const WakeDevice = graphql(`
  mutation WakeDevice($deviceId: ID!) {
    wakeDevice(deviceId: $deviceId) {
      sentAt
      relayName
    }
  }
`);

export type WakeSent = WakeDeviceMutation['wakeDevice'];

/** Failures stay the hub's typed K7 errors (CQ-045). */
export interface WakeApi {
  wakeDevice(deviceId: string): Promise<HubResult<WakeSent>>;
}

export function wakeApi(hub: Hub): WakeApi {
  return {
    wakeDevice: async (deviceId) => {
      const result = await hub.request(WakeDevice, { deviceId });
      return result.ok ? { ok: true, value: result.value.wakeDevice } : result;
    },
  };
}
