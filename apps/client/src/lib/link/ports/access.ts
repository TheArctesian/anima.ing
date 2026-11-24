import type { DeviceDetail } from '$lib/devices/api';
import { readLink } from '$lib/desktop/link/shell';
import { devicesApi } from '$lib/devices/client';
import { remoteValues } from '$lib/devices/form/remote/values';
import { rdpInput, vncInput } from '$lib/devices/form/remote/input';
import type { Hub } from '$lib/hub/hub';
import { linkApi } from '$lib/link/api';
import { deviceLinkAddress } from '$lib/link/devices';

import type { Access } from '$lib/desktop/link/access';

/** Updates only this link's device on this hub, never a same-named computer on another hub. */
export async function checkAccessHub(hub: Hub): Promise<string> {
  const local = await readLink();
  if (!local.ok || !local.value.publicKey || local.value.port === undefined || deviceLinkAddress(local.value.port)?.origin !== hub.address.origin) throw new Error('Connect this computer to this hub before setting up remote access.');
  return local.value.publicKey;
}

export async function publishAccess(hub: Hub, access: Access, target?: number): Promise<void> {
  const { api, id, device } = await linkedDevice(hub);
  const result = await api.update(id, connectionChanges(device, access, target));
  if (!result.ok) throw new Error(result.error.message);
}

/** Reads hub registration without changing local services or saved choices. */
export async function publishedAccess(hub: Hub): Promise<readonly number[]> {
  const { device } = await linkedDevice(hub);
  return ([[22, device.ssh], [3389, device.rdp], [5900, device.vnc]] as const)
    .filter(([port, connection]) => connection?.enabled && connection.port === port)
    .map(([port]) => port);
}

async function linkedDevice(hub: Hub) {
  const key = await checkAccessHub(hub);
  const linked = await linkApi(hub).devices();
  if (!linked.ok) throw new Error('The hub could not find this computer. Retry.');
  const host = linked.value.find((device) => device.key === key)?.address;
  const api = devicesApi(hub);
  const hosts = await api.deviceHosts();
  const id = hosts.ok ? hosts.value.find((device) => device.host === host)?.id : undefined;
  if (id === undefined) throw new Error('Enroll this computer in Devices, then retry remote access setup.');
  const found = await api.get(id);
  if (!found.ok || found.value === null) throw new Error('The hub could not read this computer. Retry.');
  return { api, id, device: found.value };
}

function connectionChanges(device: DeviceDetail, access: Access, target?: number) {
  const remote = remoteValues(device);
  return {
    ...(target === undefined || target === 22 ? { ssh: { enabled: access.open.includes(22), port: 22, username: device.ssh.username || access.username, auth: device.ssh.auth } } : {}),
    ...(target === undefined || target === 3389 ? { rdp: rdpInput({ ...remote.rdp, enabled: access.open.includes(3389), port: '3389', username: remote.rdp.username || access.username }) } : {}),
    ...(target === undefined || target === 5900 ? { vnc: vncInput({ ...remote.vnc, enabled: access.open.includes(5900), port: '5900', username: remote.vnc.username || (device.os === 'MACOS' ? access.username : '') }) } : {}),
  };
}
