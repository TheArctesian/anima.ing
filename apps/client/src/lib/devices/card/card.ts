import type { CardDevice } from '$lib/devices/api';

export type DeviceStatus = CardDevice['status'];

const LABELS: Record<DeviceStatus, string> = {
  UP: 'Up',
  DOWN: 'Down',
  DEGRADED: 'Degraded',
  MAINTENANCE: 'Maintenance',
  UNKNOWN: 'Unknown',
};

/** The status as text, so it never relies on colour alone. */
export function statusLabel(status: DeviceStatus): string {
  return LABELS[status];
}

/** Without an accepted host key the hub refuses to start a session (CONFLICT). */
export function offersSsh(device: Pick<CardDevice, 'ssh'>): boolean {
  return device.ssh.enabled && device.ssh.hostKey != null;
}

const IPV4 = /^\d{1,3}(?:\.\d{1,3}){3}$/;

function isAddress(host: string): boolean {
  return IPV4.test(host) || host.includes(':');
}

export function hostLine(device: Pick<CardDevice, 'host' | 'ip'>): { host: string; ip: string | null } {
  const showIp = device.ip !== null && device.ip !== '' && !isAddress(device.host);
  return { host: device.host, ip: showIp ? device.ip : null };
}

/** A long IPv6 address keeps its first two groups and its last two, which name the network and the device. */
export function shortHost(host: string): string {
  const groups = host.split(':');
  if (groups.length < 6 || host.includes('.')) return host;
  return `${groups.slice(0, 2).join(':')}:…:${groups.slice(-2).join(':')}`;
}
