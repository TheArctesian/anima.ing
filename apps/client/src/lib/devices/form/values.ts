import type { AcceptHostKeyMutation, DeviceQuery, OperatingSystem, SitesQuery, SshAuth } from '$lib/hub/generated/graphql';
import type { ThisMachine } from '$lib/desktop/this-machine';
import { normalizeHost } from './host';
import type { PolicyValues } from './policy/policy';
import {
  defaultRemoteValues,
  emptyRemoteSecrets,
  normalizedRemote,
  remoteValues,
  type RemoteFields,
  type RemoteSecrets,
  type RemoteValues,
} from './remote/values';

export type DeviceFields = Pick<NonNullable<DeviceQuery['device']>, 'name' | 'host' | 'kind' | 'os' | 'tags'> & {
  readonly ssh: Pick<NonNullable<DeviceQuery['device']>['ssh'], 'enabled' | 'port' | 'username' | 'auth'>;
} & RemoteFields;

export type SiteOption = SitesQuery['sites'][number];

export type HostKey = AcceptHostKeyMutation['acceptHostKey'];

/** Text fields stay text so that "abc" in the port box is reportable. */
export interface DeviceFormValues extends RemoteValues {
  readonly name: string;
  readonly host: string;
  readonly kind: string;
  readonly os: OperatingSystem;
  readonly tags: readonly string[];
  readonly ssh: {
    readonly enabled: boolean;
    readonly port: string;
    readonly username: string;
    readonly auth: SshAuth;
  };
  /** Never filled from the hub (D-4); empty means "keep the stored credential". */
  readonly credential: {
    readonly password: string;
    readonly privateKey: string;
    readonly passphrase: string;
  };
  /** Never filled from the hub (DEV-011). */
  readonly remoteSecrets: RemoteSecrets;
  /** The session limit a new device is created with (RA-036); an edit saves it apart (F-3). */
  readonly policy?: PolicyValues;
}

function emptyCredential(): DeviceFormValues['credential'] {
  return { password: '', privateKey: '', passphrase: '' };
}

export function formValues(device: DeviceFields | null): DeviceFormValues {
  if (device === null) {
    return {
      name: '',
      host: '',
      kind: 'server',
      os: 'OTHER',
      tags: [],
      ssh: { enabled: false, port: '22', username: '', auth: 'PASSWORD' },
      credential: emptyCredential(),
      remoteSecrets: emptyRemoteSecrets(),
      ...defaultRemoteValues(),
    };
  }
  return {
    name: device.name,
    host: device.host,
    kind: device.kind,
    os: device.os,
    tags: [...device.tags],
    ssh: {
      enabled: device.ssh.enabled,
      port: String(device.ssh.port),
      username: device.ssh.username,
      auth: device.ssh.auth,
    },
    credential: emptyCredential(),
    remoteSecrets: emptyRemoteSecrets(),
    ...remoteValues(device),
  };
}

/** R-058, R-060: `workstation` covers a laptop and a desktop, which the shell cannot tell apart. */
export function thisMachineValues(machine: ThisMachine): DeviceFormValues {
  return {
    name: machine.name,
    host: machine.vpnAddress,
    kind: 'workstation',
    os: machine.os,
    tags: [],
    ssh: { enabled: machine.sshListening, port: '22', username: machine.user, auth: 'PASSWORD' },
    credential: emptyCredential(),
    remoteSecrets: emptyRemoteSecrets(),
    ...defaultRemoteValues(),
  };
}

const MAX_PORT = 65535;
const DEFAULT_SSH_PORT = 22;

/** DEV-010: whether `text` is an integer port from 1 to 65535. */
export function isValidPort(text: string): boolean {
  return /^\d{1,5}$/.test(text) && Number(text) >= 1 && Number(text) <= MAX_PORT;
}

/** With SSH off, an unusable port falls back to the last valid one, else 22, so the hub never receives garbage. */
export function portToSend(ssh: { readonly enabled: boolean; readonly port: string }, initialPort: string | undefined): number {
  const typed = ssh.port.trim();
  if (isValidPort(typed) || ssh.enabled) return Number(typed);
  const earlier = initialPort?.trim() ?? '';
  return isValidPort(earlier) ? Number(earlier) : DEFAULT_SSH_PORT;
}

export function normalizeTags(tags: readonly string[]): string[] {
  return [...new Set(tags.map((tag) => tag.trim()).filter((tag) => tag !== ''))];
}

export function normalizedValues(values: DeviceFormValues): Omit<DeviceFormValues, 'credential' | 'remoteSecrets'> {
  return {
    name: values.name.trim(),
    host: normalizeHost(values.host),
    kind: values.kind,
    os: values.os,
    tags: normalizeTags(values.tags),
    ssh: { ...values.ssh, port: values.ssh.port.trim(), username: values.ssh.username.trim() },
    ...normalizedRemote(values),
  };
}
