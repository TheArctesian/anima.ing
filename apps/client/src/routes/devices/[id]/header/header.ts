import type { DeviceCheck } from '$lib/checks/api';
import type { CardDevice, DeviceDetail } from '$lib/devices/api';
import { hasWaysIn } from '$lib/devices/actions/actions';
import { reasonOf } from '$lib/devices/list/look';
import { offersScreenSharing } from '$lib/devices/screen-sharing/setup';
import { osIcon } from '$lib/devices/os/os';
import type { IconName } from '$lib/icons/icons';
import type { SessionProtocol } from '$lib/sessions/tabs';
import type { SettingsSection } from '../tabs/tabs';

/** What the detail bar reads from a device (DEV-020, D16). */
export type HeaderDevice = Pick<
  DeviceDetail,
  'id' | 'name' | 'status' | 'host' | 'ip' | 'os' | 'distribution' | 'accessWarnings' | 'webInterfaces'
> &
  Partial<Pick<DeviceDetail, 'kind'>> & {
  readonly ssh: Pick<DeviceDetail['ssh'], 'enabled' | 'hostKey'>;
  readonly rdp: Pick<DeviceDetail['rdp'], 'enabled'>;
  readonly vnc: Pick<DeviceDetail['vnc'], 'enabled' | 'username'>;
};

/** SSH opens the connection test when the hub has no accepted host key, since the hub refuses a session then (DEV-022). */
export type SshAction = 'connect' | 'test' | 'set-up';

export type ProtocolMode = 'connect' | 'test' | 'set-up';

export function protocolButton(
  protocol: SessionProtocol,
  mode: ProtocolMode,
  deviceName: string,
): { readonly label: string; readonly section: SettingsSection | null } {
  if (mode === 'connect') return { label: `${protocol} to ${deviceName}`, section: null };
  if (mode === 'test') return { label: `Test the ${protocol} connection`, section: 'test' };
  return { label: `Set up ${protocol}`, section: protocol === 'RDP' ? 'rdp' : 'ssh' };
}

export interface HeaderLine {
  readonly ip: string | null;
  readonly osIcon: IconName;
  readonly ssh: SshAction;
  readonly rdp: 'connect' | 'set-up';
  readonly vnc: boolean;
  /** "Set up screen sharing" stands in for VNC on a macOS or Linux device with VNC off (R-079). */
  readonly screenSharing: boolean;
  /** False for the hub's own device: the bar offers no protocol (HMD-046). */
  readonly waysIn: boolean;
}

function sshAction(ssh: HeaderDevice['ssh']): SshAction {
  if (!ssh.enabled) return 'set-up';
  return ssh.hostKey == null ? 'test' : 'connect';
}

export function headerLine(device: HeaderDevice): HeaderLine {
  const ip = device.ip ?? '';
  return {
    ip: ip !== '' && ip !== device.host ? ip : null,
    osIcon: osIcon(device.os, device.distribution),
    ssh: sshAction(device.ssh),
    rdp: device.rdp.enabled ? 'connect' : 'set-up',
    vnc: device.vnc.enabled,
    screenSharing: hasWaysIn(device) && offersScreenSharing(device),
    waysIn: hasWaysIn(device),
  };
}

export interface ProtocolAction {
  readonly protocol: SessionProtocol;
  readonly mode: ProtocolMode;
  readonly label: string;
  readonly section: SettingsSection | null;
}

/** The connect split button (D16, DEV-022); `null` for the hub's own device (HMD-046). */
export function connectActions(line: HeaderLine, deviceName: string): { readonly main: ProtocolAction; readonly others: readonly ProtocolAction[] } | null {
  if (!line.waysIn) return null;
  const action = (protocol: SessionProtocol, mode: ProtocolMode): ProtocolAction => ({ protocol, mode, ...protocolButton(protocol, mode, deviceName) });
  const ssh = action('SSH', line.ssh);
  const actions = [ssh, action('RDP', line.rdp), ...(line.vnc ? [action('VNC', 'connect')] : [])];
  const main = actions.find((each) => each.mode === 'connect') ?? ssh;
  const rest = actions.filter((each) => each !== main);
  return { main, others: [...rest.filter((each) => each.mode === 'connect'), ...rest.filter((each) => each.mode !== 'connect')] };
}

export function actionWords(action: Pick<ProtocolAction, 'protocol' | 'mode'>): string {
  if (action.mode === 'connect') return action.protocol;
  return action.mode === 'test' ? `Test ${action.protocol}` : `Set up ${action.protocol}`;
}

function lookCheck(check: DeviceCheck): CardDevice['checks'][number] {
  const beat = check.lastHeartbeat;
  return { id: check.id, name: check.name, status: check.status, paused: check.paused, heartbeats: { edges: beat == null ? [] : [{ node: beat }] } };
}

/** Why the device is not up, as the list's reason (D16, D18); `null` while the checks it needs have not arrived. */
export function statusReason(device: Pick<HeaderDevice, 'id' | 'status'>, checks: readonly DeviceCheck[] | null): string | null {
  if (device.status === 'UP' || (checks === null && device.status !== 'MAINTENANCE')) return null;
  const reason = reasonOf({ id: device.id, status: device.status, checks: (checks ?? []).map(lookCheck) }, undefined, 0);
  if (reason === null) return null;
  const text = reason.text.charAt(0).toUpperCase() + reason.text.slice(1);
  return /[.!?]$/.test(text) ? text : `${text}.`;
}
