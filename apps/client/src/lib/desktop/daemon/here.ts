import { readCloudDevice, type CloudDevice } from '$lib/desktop/cloud/device';
import { devicesApi } from '$lib/devices/client';
import { comparable } from '$lib/hosted/this-computer';
import type { MetricsSource } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';
import { linkApi } from '$lib/link/api';
import { metricsApi } from '$lib/metrics/api';
import { warnToProcessLog } from '../process-log';
import { daemonBundled, daemonKeychainWaits, installDaemon, type DaemonShellProblem, type DaemonShellResult } from './shell';

const ONE_PAGE = 200;

export interface HubDevice {
  readonly id: string;
  readonly host: string;
  readonly metricsSource: MetricsSource;
}

export interface DaemonHereParts {
  readonly bundled: () => Promise<boolean>;
  readonly device: () => Promise<CloudDevice | null>;
  readonly linkDevices: () => Promise<HubResult<readonly { readonly name: string; readonly address: string }[]>>;
  /** A hub with more than one page of devices gives the first page. */
  readonly devices: () => Promise<HubResult<readonly HubDevice[]>>;
  /** The code works once. */
  readonly enrol: (deviceId: string) => Promise<HubResult<{ readonly code: string }>>;
  readonly install: (code: string, origin: string) => Promise<DaemonShellResult>;
  readonly origin: string;
  /** No part means the daemon never waits for the person in the macOS Keychain. */
  readonly keychainWaits?: () => Promise<boolean>;
  readonly wait?: (milliseconds: number) => Promise<void>;
  /** Each line is fixed text and a fixed code: never a name, an address or the code. */
  readonly log?: (line: string) => void;
}

export type NotFound = 'shell-does-not-read-this-computer' | 'link-list-not-read' | 'not-on-the-link' | 'device-list-not-read' | 'not-a-device-yet';

/** The install can start before this computer is a device (HMD-044) or the link has a daemon socket (HMD-058). */
const TRIES = 5;
const WAIT_MS = 30_000;
const timer = (milliseconds: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, milliseconds));

export type DaemonHere =
  | { readonly kind: 'installed' }
  | { readonly kind: 'present' }
  | { readonly kind: 'needs-you' }
  | { readonly kind: 'not-ready' }
  | { readonly kind: 'failed'; readonly step: 'enrol' | DaemonShellProblem };

export type FindHereParts = Pick<DaemonHereParts, 'device' | 'linkDevices' | 'devices'>;

export async function deviceHere(parts: FindHereParts): Promise<HubDevice | NotFound> {
  const computer = await parts.device();
  if (computer === null) return 'shell-does-not-read-this-computer';
  const linked = await parts.linkDevices();
  if (!linked.ok) return 'link-list-not-read';
  const address = linked.value.find((link) => link.name === computer.name)?.address;
  if (address === undefined || address === '') return 'not-on-the-link';
  const devices = await parts.devices();
  if (!devices.ok) return 'device-list-not-read';
  const mine = comparable(address);
  return devices.value.find((device) => comparable(device.host) === mine) ?? 'not-a-device-yet';
}

interface Turn {
  readonly did: DaemonHere;
  readonly again: boolean;
  readonly note: string;
}

async function turn(parts: DaemonHereParts): Promise<Turn> {
  const device = await deviceHere(parts);
  if (typeof device === 'string') return { did: { kind: 'not-ready' }, again: true, note: `not ready (${device})` };
  if (device.metricsSource === 'DAEMON') return { did: { kind: 'present' }, again: false, note: 'the daemon reports already' };
  // The code goes to the shell and nowhere else (LOG-007); a code the shell does not use expires at the hub.
  const made = await parts.enrol(device.id);
  if (!made.ok) return { did: { kind: 'failed', step: 'enrol' }, again: false, note: 'the hub made no enrolment code' };
  const done = await parts.install(made.value.code, parts.origin);
  if (done.ok) return { did: { kind: 'installed' }, again: false, note: 'installed' };
  const waiting = done.problem === 'origin';
  const note = waiting ? 'the link has no socket for the daemon yet' : `the shell did not install it (${done.problem})`;
  return { did: { kind: 'failed', step: done.problem }, again: waiting, note };
}

async function install(parts: DaemonHereParts): Promise<DaemonHere> {
  const log = parts.log ?? ((): void => undefined);
  if (!(await parts.bundled())) {
    log('the daemon of this computer: this build carries no daemon');
    return { kind: 'not-ready' };
  }
  for (let attempt = 1; ; attempt += 1) {
    const { did, again, note } = await turn(parts);
    const last = !again || attempt >= TRIES;
    // Each turn is in the log, so that a start that ends with no metrics shows where it stopped.
    log(`the daemon of this computer: try ${String(attempt)} of ${String(TRIES)}: ${note}${last ? '' : '; it looks again in 30 s'}`);
    if (last) return await waitsForYou(did, parts, log);
    await (parts.wait ?? timer)(WAIT_MS);
  }
}

async function waitsForYou(did: DaemonHere, parts: DaemonHereParts, log: (line: string) => void): Promise<DaemonHere> {
  if (did.kind !== 'installed' && did.kind !== 'present') return did;
  if (!(await parts.keychainWaits?.())) return did;
  log('the daemon of this computer waits for you to allow it in the Keychain');
  return { kind: 'needs-you' };
}

/** A second page that opens meanwhile waits for this install, so one code is made, not two. */
let running: Promise<DaemonHere> | null = null;
/** The app tries once for each start (HMD-053). */
let tried = false;

/** Installs with no question and fails quietly (HMD-053, R-240); the enrolment code is never shown, stored or logged. */
export function installDaemonHere(parts: DaemonHereParts): Promise<DaemonHere> {
  if (tried && running === null) return Promise.resolve({ kind: 'not-ready' });
  running ??= install(parts)
    .catch((): DaemonHere => ({ kind: 'failed', step: 'failed' }))
    .finally(() => {
      tried = true;
      running = null;
    });
  return running;
}

export function installDaemonOnThisComputer(hub: Hub, show: (did: DaemonHere | 'installing') => void): Promise<DaemonHere> {
  show('installing');
  const done = installDaemonHere({
    bundled: daemonBundled,
    ...findHereParts(hub),
    enrol: (deviceId) => metricsApi(hub).enrolDaemon(deviceId),
    install: installDaemon,
    origin: hub.address.origin,
    keychainWaits: daemonKeychainWaits,
    log: warnToProcessLog,
  });
  void done.then(show);
  return done;
}

export function findHereParts(hub: Hub): FindHereParts {
  return {
    device: readCloudDevice,
    linkDevices: () => linkApi(hub).devices(),
    devices: async () => {
      const page = await devicesApi(hub).list({ first: ONE_PAGE });
      return page.ok ? { ok: true, value: page.value.edges.map((edge) => edge.node) } : page;
    },
  };
}

export function forgetDaemonHere(): void {
  tried = false;
  running = null;
}
