import { graphql } from '$lib/hub/generated';
import type { BatteryChargesAddedSubscription } from '$lib/hub/generated/graphql';
import type { Hub, Sink } from '$lib/hub/hub';

// The hub keeps the charge only as the series BATTERY_PERCENT (MET-032).
const BatteryChargesAdded = graphql(`
  subscription BatteryChargesAdded {
    metricsAdded(deviceId: null) {
      deviceId
      samples {
        kind
        label
        value
      }
    }
  }
`);

export type ChargeBatch = BatteryChargesAddedSubscription['metricsAdded'];

export function batteryApi(hub: Pick<Hub, 'subscribe'>) {
  return {
    watch: (sink: Sink<ChargeBatch>): (() => void) =>
      hub.subscribe(BatteryChargesAdded, {}, {
        ...sink,
        next: (value) => {
          sink.next(value.metricsAdded);
        },
      }),
  };
}
