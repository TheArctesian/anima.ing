import { invoke, isTauri } from '@tauri-apps/api/core';
import type { OperatingSystem } from '$lib/hub/generated/graphql';
import { isOnThisComputer, type Hub } from '$lib/hub/hub';
import { vpnApi } from '$lib/vpn/api';

/** The computer the desktop app runs on, as the shell reads it (R-058, R-060). Nothing here is logged (LOG-007). */
export interface ThisMachine {
  readonly name: string;
  readonly user: string;
  readonly os: Extract<OperatingSystem, 'MACOS' | 'LINUX' | 'WINDOWS'>;
  /** From the VPN client logged in to the hub's login server (NET-001, ADR-005). */
  readonly vpnAddress: string;
  readonly sshListening: boolean;
}

const OCTET = '(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)';
const IPV4 = new RegExp(`^${OCTET}(?:\\.${OCTET}){3}$`);

/** Whether `text` is an IPv4 address inside 100.64.0.0/10, the Anima.ing VPN's range (NET-001). */
function isVpnAddress(text: string): boolean {
  if (!IPV4.test(text)) return false;
  const [first, second] = text.split('.').map(Number);
  return first === 100 && second !== undefined && second >= 64 && second <= 127;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/** The shell's answer crosses a process boundary, so every field is checked. */
function thisMachineFrom(answer: unknown): ThisMachine | null {
  if (!isRecord(answer)) return null;
  const { name, user, os, vpnAddress, sshListening } = answer;
  if (typeof name !== 'string' || typeof user !== 'string' || typeof sshListening !== 'boolean') return null;
  if (os !== 'MACOS' && os !== 'LINUX' && os !== 'WINDOWS') return null;
  if (typeof vpnAddress !== 'string' || !isVpnAddress(vpnAddress)) return null;
  return { name, user, os, vpnAddress, sshListening };
}

/** A hub on this computer is not reached over the VPN, and the shell's read would start Tailscale on a Mac (DEF-005). */
export async function readThisMachine(hub: Hub): Promise<ThisMachine | null> {
  if (!isTauri() || isOnThisComputer(hub.address)) return null;
  const loginServer = await vpnApi(hub).loginServer();
  if (!loginServer.ok) return null;
  try {
    return thisMachineFrom(await invoke('this_machine_read', { loginServer: loginServer.value }));
  } catch {
    return null;
  }
}
