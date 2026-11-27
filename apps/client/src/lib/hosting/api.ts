import { composeApplyApi, type ApplyCalls } from './compose/api';
import { containerActionApi, type ContainerActionCalls } from '$lib/containers/action/api';
import { graphql } from '$lib/hub/generated';
import { containersApi, type Container, type ContainerCalls, type ContainerStorage, type ImageUpdate } from '$lib/containers/api';
import type { HostingMachinesQuery } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

const MACHINES = graphql(`
  query HostingMachines($after: String) {
    devices(first: 100, after: $after) {
      pageInfo {
        hasNextPage
        endCursor
      }
      edges {
        node {
          id
          name
          status
          ssh {
            enabled
          }
        }
      }
    }
  }
`);

/** A machine of the hub that can hold containers: the hub reads Docker on it through SSH (HOST-001). */
export interface Machine {
  readonly id: string;
  readonly name: string;
  /** The status of the device in the hub: Up, Down, Degraded, Maintenance or Unknown. */
  readonly state: HostingMachinesQuery['devices']['edges'][number]['node']['status'];
}

/** What the section Applications asks of the hub (HSE-002). Each call reads the machine at that time; the hub keeps nothing. */
export interface HostingCalls {
  readonly actions: ContainerActionCalls;
  readonly apply: ApplyCalls;
  /** The machines with SSH on, by name. A device with SSH off has no containers that the hub can read. */
  machines(): Promise<HubResult<readonly Machine[]>>;
  containers(machineId: string): Promise<HubResult<readonly Container[]>>;
  /** HOST-021: whether a newer image exists for each container of one machine. */
  updates(machineId: string): Promise<HubResult<readonly ImageUpdate[]>>;
  /** HOST-001 and HOST-017: the log and the details of the containers of one machine. */
  machine(machineId: string): Pick<ContainerCalls, 'log' | 'detail'>;
  /** HOST-020 and HSE-015: the images and the volumes of one machine, with their sizes. */
  storage(machineId: string): Promise<HubResult<ContainerStorage>>;
}

/** The calls of the section Applications: the device list of the hub, then the container calls of each machine (task H42). */
export function hostingApi(hub: Pick<Hub, 'request'>): HostingCalls {
  return {
    actions: containerActionApi(hub),
    apply: composeApplyApi(hub),
    machines: async () => {
      const machines: Machine[] = [];
      let after: string | null = null;
      for (;;) {
        const result: HubResult<HostingMachinesQuery> = await hub.request(MACHINES, { after });
        if (!result.ok) return result;
        const page: HostingMachinesQuery['devices'] = result.value.devices;
        for (const { node } of page.edges) if (node.ssh.enabled) machines.push({ id: node.id, name: node.name, state: node.status });
        if (!page.pageInfo.hasNextPage || page.pageInfo.endCursor == null) return { ok: true, value: machines };
        after = page.pageInfo.endCursor;
      }
    },
    containers: (machineId) => containersApi(hub, machineId).list(),
    updates: (machineId) => containersApi(hub, machineId).updates(),
    machine: (machineId) => containersApi(hub, machineId),
    storage: (machineId) => containersApi(hub, machineId).storage(),
  };
}
