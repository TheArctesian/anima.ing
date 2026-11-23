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

export async function publishAccess(hub: Hub, access: Access): Promise<void> {
  return publishForKey(hub, access, await checkAccessHub(hub));
}

async function publishForKey(hub: Hub, access: Access, key: string): Promise<void> {
  const linked = await linkApi(hub).devices();
  if (!linked.ok) throw new Error('The hub could not find this computer. Retry.');
  const host = linked.value.find((device) => device.key === key)?.address;
  const api = devicesApi(hub);
  const hosts = await api.deviceHosts();
  const id = hosts.ok ? hosts.value.find((device) => device.host === host)?.id : undefined;
  if (id === undefined) throw new Error('Enroll this computer in Devices, then retry remote access setup.');
  const found = await api.get(id);
  if (!found.ok || found.value === null) throw new Error('The hub could not read this computer. Retry.');
  const device = found.value;
  const remote = remoteValues(device);
  const result = await api.update(id, {
    ssh: { enabled: access.open.includes(22), port: 22, username: device.ssh.username || access.username, auth: device.ssh.auth },
    rdp: rdpInput({ ...remote.rdp, enabled: access.open.includes(3389), port: '3389', username: remote.rdp.username || access.username }),
    vnc: vncInput({ ...remote.vnc, enabled: access.open.includes(5900), port: '5900', username: device.os === 'MACOS' ? access.username : remote.vnc.username }),
  });
  if (!result.ok) throw new Error(result.error.message);
}
