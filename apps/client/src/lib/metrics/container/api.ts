import { graphql } from '$lib/hub/generated';
import type { ContainerSamplesAddedSubscription } from '$lib/hub/generated/graphql';
import type { Hub, Sink } from '$lib/hub/hub';

// K1 gives the bytes of memory in use only as the series MEMORY_BYTES.
const ContainerSamplesAdded = graphql(`
  subscription ContainerSamplesAdded($deviceId: ID!) {
    metricsAdded(deviceId: $deviceId) {
      deviceId
      samples {
        kind
        label
        value
      }
    }
  }
`);

export type ContainerBatch = ContainerSamplesAddedSubscription['metricsAdded'];

export function containerApi(hub: Pick<Hub, 'subscribe'>) {
  return {
    watch: (deviceId: string, sink: Sink<ContainerBatch>): (() => void) =>
      hub.subscribe(ContainerSamplesAdded, { deviceId }, {
        ...sink,
        next: (value) => {
          sink.next(value.metricsAdded);
        },
      }),
  };
}
