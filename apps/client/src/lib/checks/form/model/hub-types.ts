import type { DeviceCheck } from '$lib/checks/api';
import type { CheckType } from '$lib/hub/generated/graphql';

/** Check shapes from the generated types (API-008). */
export type {
  CheckConfigInput,
  CreateCheckInput,
  DnsRecordType,
  HttpAuthInput,
  HttpAuthKind,
  HttpConfigInput,
  HttpMethod,
  IpVersion,
  CheckType,
  UpdateCheckInput,
} from '$lib/hub/generated/graphql';

export type CheckConfigFields = DeviceCheck['config'];

export type CheckFields = Pick<
  DeviceCheck,
  'id' | 'name' | 'intervalSeconds' | 'retries' | 'retryIntervalSeconds' | 'degradedThresholdMs' | 'invert' | 'domainDaysLeft'
> & {
  type: CheckType;
  config: CheckConfigFields;
};
