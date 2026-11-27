import { graphql } from '$lib/hub/generated';
import type {
  ApplicationApplyInput,
  HostingApplyStatusQuery,
  HostingApplyTargetsQuery,
  HostingCheckComposeQuery,
} from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';

const QUEUE = graphql(`
  mutation HostingQueueApply($input: ApplicationApplyInput!) {
    queueApplicationApply(input: $input) { id applicationId deviceId state reason health }
  }
`);
const STATUS = graphql(`
  query HostingApplyStatus($id: ID!) {
    applicationApply(id: $id) { id applicationId deviceId state reason health }
  }
`);
const TARGETS = graphql(`
  query HostingApplyTargets {
    applicationApplyTargets { id name }
  }
`);
const CHECK = graphql(`
  query HostingCheckCompose($file: String!) {
    composeFileCheck(file: $file) {
      syntaxValid
      problems { code severity line column }
    }
  }
`);

export type ApplyStatus = NonNullable<HostingApplyStatusQuery['applicationApply']>;
export type ApplyTarget = HostingApplyTargetsQuery['applicationApplyTargets'][number];
export type ComposeCheck = HostingCheckComposeQuery['composeFileCheck'];

export interface ApplyCalls {
  targets(): Promise<HubResult<readonly ApplyTarget[]>>;
  queue(input: ApplicationApplyInput): Promise<HubResult<ApplyStatus>>;
  status(id: string): Promise<HubResult<ApplyStatus | null>>;
  check(file: string): Promise<HubResult<ComposeCheck>>;
}

export function composeApplyApi(hub: Pick<Hub, 'request'>): ApplyCalls {
  return {
    targets: async () => {
      const result = await hub.request(TARGETS, {});
      return result.ok ? { ok: true, value: result.value.applicationApplyTargets } : result;
    },
    queue: async (input) => {
      const result = await hub.request(QUEUE, { input });
      return result.ok ? { ok: true, value: result.value.queueApplicationApply } : result;
    },
    status: async (id) => {
      const result = await hub.request(STATUS, { id });
      return result.ok ? { ok: true, value: result.value.applicationApply ?? null } : result;
    },
    check: async (file) => {
      const result = await hub.request(CHECK, { file });
      return result.ok ? { ok: true, value: result.value.composeFileCheck } : result;
    },
  };
}

export function ended(apply: ApplyStatus): boolean {
  return apply.state === 'done' || apply.state === 'failed' || apply.state === 'stopped';
}

export function statusMessage(apply: ApplyStatus): string {
  switch (apply.state) {
    case 'queued': return 'Queued for the device.';
    case 'claimed':
    case 'preparing': return 'Preparing the deployment.';
    case 'running': return 'Applying Compose on the device.';
    case 'finishing': return 'Checking application health.';
    case 'stopped': return 'The apply stopped.';
    case 'failed': return failureMessage(apply.reason);
    case 'done':
      if (apply.health === 'healthy') return 'Applied. The application is healthy.';
      if (apply.health === 'unhealthy') return 'Applied. The application reported an unhealthy service.';
      return apply.health === 'no_check' ? 'Applied. No health checks are configured.' : 'Applied. Health is unconfirmed.';
    default: return 'Waiting for an update from the hub.';
  }
}

function failureMessage(cause: ApplyStatus['reason']): string {
  switch (cause) {
    case 'secret_failed': return 'The device did not receive both files. Submit a fresh apply with the Compose file and any environment file.';
    case 'file_not_permitted': return 'The Compose or environment file failed validation on the device.';
    case 'name_taken': return 'The Compose project belongs to another folder on this device.';
    case 'busy': return 'The device is busy with a container change or local maintenance. Submit a fresh apply when it is ready.';
    case 'migration_held': return 'Finish the local Podman cutover checks on this device, then submit a fresh apply.';
    case 'no_digest': return 'An image digest is unavailable for this application.';
    case 'orphan': return 'The device could not confirm the apply result. Check the application before submitting another apply.';
    default: return cause == null ? 'The apply failed.' : `The apply failed (${cause.replaceAll('_', ' ')}).`;
  }
}
