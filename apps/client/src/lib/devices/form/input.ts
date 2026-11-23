import type { CreateDeviceInput, SshCredentialInput, SshSettingsInput, UpdateDeviceInput } from '$lib/hub/generated/graphql';
import { createAdditions, updateAdditions } from './remote/input';
import { createPolicyInput } from './policy/policy';
import { isVpnAddress } from './site';
import { normalizedValues, portToSend, type DeviceFormValues } from './values';

type NormalizedSsh = ReturnType<typeof normalizedValues>['ssh'];

export function sshInput(ssh: NormalizedSsh, initialPort?: string): SshSettingsInput {
  return { enabled: ssh.enabled, port: portToSend(ssh, initialPort), username: ssh.username, auth: ssh.auth };
}

export function sameSsh(initial: NormalizedSsh, now: NormalizedSsh): boolean {
  const a = sshInput(initial);
  const b = sshInput(now, initial.port);
  return a.enabled === b.enabled && a.port === b.port && a.username === b.username && a.auth === b.auth;
}

function sameTags(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((tag, index) => tag === b[index]);
}

/** A VPN address is reached only on its grant's TCP ports (R-061, R-063) and macOS drops pings, so SSH replaces ICMP; null keeps the hub's default. */
export function tcpDefaultCheckPort(values: DeviceFormValues): number | null {
  const now = normalizedValues(values);
  if (!now.ssh.enabled || !isVpnAddress(now.host)) return null;
  return portToSend(now.ssh, undefined);
}

/** DEV-010: the hub adds the default ICMP check (Section 6.2) unless R-063 applies. */
export function createInput(values: DeviceFormValues, siteId: string): CreateDeviceInput {
  const now = normalizedValues(values);
  return {
    siteId,
    name: now.name,
    host: now.host,
    kind: now.kind,
    os: now.os,
    tags: [...now.tags],
    ...(now.ssh.enabled ? { ssh: sshInput(now.ssh) } : {}),
    ...createAdditions(values),
    ...(values.policy === undefined ? {} : createPolicyInput(values.policy)),
    addDefaultCheck: tcpDefaultCheckPort(values) === null,
  };
}

/** DEV-010: only the fields that differ from `initial`; an unchanged form gives `{}`. SSH goes whole, as the hub input needs all its parts. */
export function updateInput(initial: DeviceFormValues, values: DeviceFormValues): UpdateDeviceInput {
  const before = normalizedValues(initial);
  const now = normalizedValues(values);
  return {
    ...(now.name !== before.name ? { name: now.name } : {}),
    ...(now.host !== before.host ? { host: now.host } : {}),
    ...(now.kind !== before.kind ? { kind: now.kind } : {}),
    ...(now.os !== before.os ? { os: now.os } : {}),
    ...(sameTags(now.tags, before.tags) ? {} : { tags: [...now.tags] }),
    ...(sameSsh(before.ssh, now.ssh) ? {} : { ssh: sshInput(now.ssh, before.ssh.port) }),
    ...updateAdditions(initial, values),
  };
}

/** DEV-012: null when the chosen auth's fields are empty, so the stored credential is kept (D-4). */
export function credentialInput(values: DeviceFormValues): SshCredentialInput | null {
  const { credential, ssh } = values;
  if (!ssh.enabled) return null;
  if (ssh.auth === 'PASSWORD') return credential.password === '' ? null : { password: credential.password };
  if (credential.privateKey.trim() === '') return null;
  return {
    privateKey: {
      key: credential.privateKey,
      ...(credential.passphrase === '' ? {} : { passphrase: credential.passphrase }),
    },
  };
}
