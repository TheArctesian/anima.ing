import type { CloudDevice } from '$lib/desktop/cloud/device';
import { normalizeHost } from '$lib/devices/form/host';
import { LINK_SITE, type NewSite } from '$lib/devices/form/site';
import type { CreateDeviceInput } from '$lib/hub/generated/graphql';
import type { HubResult } from '$lib/hub/hub';

export interface ThisComputerParts {
  readonly device: () => Promise<CloudDevice | null>;
  readonly linkDevices: () => Promise<HubResult<readonly { readonly name: string; readonly address: string }[]>>;
  readonly hosts: () => Promise<HubResult<readonly string[]>>;
  readonly sites: () => Promise<HubResult<readonly { readonly id: string; readonly name: string; readonly subnet: string }[]>>;
  readonly createSite: (name: string, subnet: string) => Promise<HubResult<{ readonly id: string }>>;
  readonly createDevice: (input: CreateDeviceInput) => Promise<HubResult<unknown>>;
}

const GROUP = /^[0-9a-f]{1,4}$/;

/** An IPv6 host compares as its eight lower-case groups without leading zeros or brackets; any other host as its lower-case text. */
export function comparable(host: string): string {
  const text = normalizeHost(host).toLowerCase();
  const [head = '', tail, extra] = text.split('::');
  if (!text.includes(':') || extra !== undefined) return text;
  const before = head === '' ? [] : head.split(':');
  const after = tail === undefined || tail === '' ? [] : tail.split(':');
  const missing = 8 - before.length - after.length;
  if (tail === undefined ? missing !== 0 : missing < 1) return text;
  const groups = [...before, ...Array.from({ length: missing }, () => '0'), ...after];
  if (!groups.every((group) => GROUP.test(group))) return text;
  return groups.map((group) => Number.parseInt(group, 16).toString(16)).join(':');
}

export async function networkSite(parts: Pick<ThisComputerParts, 'sites' | 'createSite'>, network: NewSite): Promise<string | null> {
  const sites = await parts.sites();
  if (!sites.ok) return null;
  const known = sites.value.find((site) => site.subnet === network.subnet || site.name === network.name);
  if (known !== undefined) return known.id;
  const created = await parts.createSite(network.name, network.subnet);
  return created.ok ? created.value.id : null;
}

async function addressOnLink(device: CloudDevice, parts: ThisComputerParts): Promise<string | null> {
  const linked = await parts.linkDevices();
  if (!linked.ok) return null;
  const address = linked.value.find((link) => link.name === device.name)?.address;
  return address || null;
}

export type DeviceEnrolment = 'added' | 'already-added' | 'not-linked' | 'failed' | 'metrics-failed';

export async function enrolAtAddress(parts: Pick<ThisComputerParts, 'hosts' | 'sites' | 'createSite' | 'createDevice'>, network: NewSite, input: Omit<CreateDeviceInput, 'siteId'>): Promise<DeviceEnrolment> {
  const hosts = await parts.hosts();
  if (!hosts.ok) return 'failed';
  if (hosts.value.some((host) => comparable(host) === comparable(input.host))) return 'already-added';
  const siteId = await networkSite(parts, network);
  if (siteId === null) return 'failed';
  const created = await parts.createDevice({ ...input, siteId });
  return created.ok ? 'added' : 'failed';
}

async function add(parts: ThisComputerParts): Promise<DeviceEnrolment> {
  const device = await parts.device();
  if (device === null) return 'not-linked';
  const host = await addressOnLink(device, parts);
  if (host === null) return 'not-linked';
  const name = device.computer.trim() === '' ? device.name : device.computer.trim();
  // No default check: the hub cannot reach a link address yet, so an ICMP check would always say Down (HM-D11).
  return enrolAtAddress(parts, LINK_SITE, { name, host, kind: 'workstation', os: device.os, tags: [], addDefaultCheck: false });
}

/** The add that runs now; a second page that opens meanwhile waits for it, so it cannot add the computer twice. */
let running: Promise<DeviceEnrolment> | null = null;

/** Adds this computer with no question (HMD-044, R-136); every failure is quiet and the next start tries again. */
export function enrolThisComputer(parts: ThisComputerParts): Promise<DeviceEnrolment> {
  running ??= add(parts)
    .catch((): DeviceEnrolment => 'failed')
    .finally(() => {
      running = null;
    });
  return running;
}

export async function addThisComputer(parts: ThisComputerParts): Promise<boolean> {
  return (await enrolThisComputer(parts)) === 'added';
}
