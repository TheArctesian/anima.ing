import { graphql } from '$lib/hub/generated';
import type { ContainerActionStatusQuery, ContainerActionVerb } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

const QUEUE = graphql(`
  mutation QueueContainerAction($input: ContainerActionInput!) {
    queueContainerAction(input: $input) { id state reason }
  }
`);

const STATUS = graphql(`
  query ContainerActionStatus($id: ID!) {
    containerAction(id: $id) { id state reason }
  }
`);

export type Action = ContainerActionVerb;
export type ActionStatus = NonNullable<ContainerActionStatusQuery['containerAction']>;

export interface ContainerActionCalls {
  queue(deviceId: string, containerId: string, action: Action): Promise<HubResult<ActionStatus>>;
  status(id: string): Promise<HubResult<ActionStatus | null>>;
}

export function containerActionApi(hub: Pick<Hub, 'request'>): ContainerActionCalls {
  return {
    queue: async (deviceId, containerId, action) => {
      const answer = await hub.request(QUEUE, { input: { deviceId, containerId, action } });
      return answer.ok ? { ok: true, value: answer.value.queueContainerAction } : answer;
    },
    status: async (id) => {
      const answer = await hub.request(STATUS, { id });
      return answer.ok ? { ok: true, value: answer.value.containerAction ?? null } : answer;
    },
  };
}

export const ACTIONS = [
  { value: 'START', label: 'Start' },
  { value: 'STOP', label: 'Stop' },
  { value: 'RESTART', label: 'Restart' },
] as const satisfies readonly { value: Action; label: string }[];

const REASONS: Readonly<Record<string, string>> = {
  applications_off: 'Container actions are off on this device.',
  no_container: 'The device no longer has this container.',
  protected: 'This container is protected by the device administrator.',
  not_adopted: 'This container is outside the device’s hosting policy.',
  start_not_permitted: 'The device’s hosting policy does not allow this container to start.',
  migration_held: 'Finish the local Podman cutover checks on this device before starting the container.',
  busy: 'The device is busy with a container change or local maintenance.',
  limit_seconds: 'The action reached its time limit.',
  orphan: 'The device could not confirm the container result. Check it before retrying.',
};

export function actionMessage(status: ActionStatus): string {
  switch (status.state) {
    case 'queued': return 'Queued. Waiting for the device.';
    case 'claimed':
    case 'preparing': return 'Accepted by the device.';
    case 'running':
    case 'finishing': return 'Running on the device.';
    case 'done': return 'Completed.';
    case 'stopped': return 'The action was stopped.';
    case 'failed': return status.reason == null ? 'The device could not complete the action.' : (REASONS[status.reason] ?? 'The device could not complete the action.');
    default: return 'Checking the action.';
  }
}

export function actionEnded(status: ActionStatus): boolean {
  return status.state === 'done' || status.state === 'failed' || status.state === 'stopped';
}
