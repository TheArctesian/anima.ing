import { hubIsHosted } from '$lib/hosted/mark';
import type { Hub, HubResult } from '$lib/hub/hub';
import { metricsApi } from '$lib/metrics/api';
import { capabilities } from '$lib/platform/platform';
import { warnToProcessLog } from '../process-log';
import { deviceHere, findHereParts, type FindHereParts, type NotFound } from './here';
import { removeDaemon, type DaemonRemoval } from './shell';

export const HUB_STILL_LISTS =
  'The daemon is removed from this computer, but the hub could not be reached and still lists it. Revoke the daemon on the page of this computer in Devices when the hub answers.';
export const NOT_REMOVED = 'The daemon was not removed from this computer.';
export const HUB_NOT_TOLD = 'The hub still lists it too.';

export interface DaemonLeaveParts extends FindHereParts {
  readonly remove: () => Promise<DaemonRemoval>;
  readonly revoke: (deviceId: string) => Promise<HubResult<unknown>>;
  /** Takes fixed text and fixed codes only. */
  readonly log?: (line: string) => void;
}

/** This computer is not a device of the hub, so the hub lists no daemon of it. */
const NOT_LISTED: ReadonlySet<NotFound> = new Set(['not-on-the-link', 'not-a-device-yet']);

async function revokedAtHub(parts: DaemonLeaveParts, log: (line: string) => void): Promise<boolean> {
  const device = await deviceHere(parts);
  if (typeof device === 'string') {
    log(`the removal of the daemon: this computer was not found at the hub (${device})`);
    return NOT_LISTED.has(device);
  }
  const revoked = await parts.revoke(device.id);
  log(`the removal of the daemon: ${revoked.ok ? 'the hub revoked it' : `the hub did not revoke it (${revoked.error.code})`}`);
  return revoked.ok;
}

/** The removal on this computer runs first, whatever the hub does (HMD-056); `null` when nothing stays. */
export async function leaveDaemonHere(parts: DaemonLeaveParts): Promise<string | null> {
  const log = parts.log ?? ((): void => undefined);
  const removed = await parts.remove();
  log(`the removal of the daemon: ${removed.ok ? (removed.wasHere ? 'removed from this computer' : 'no copy was here') : `not removed (${removed.problem})`}`);
  const told = await revokedAtHub(parts, log).catch(() => false);
  if (!removed.ok) return [NOT_REMOVED, removed.message, ...(told ? [] : [HUB_NOT_TOLD])].join(' ');
  return removed.wasHere && !told ? HUB_STILL_LISTS : null;
}

/** Only for a hosted hub, where the app installed the daemon (HMD-053); it runs before the sign-out, while the token can revoke. */
export async function leaveDaemonOnThisComputer(hub: Hub): Promise<string | null> {
  if (!capabilities().thisMachine || !hubIsHosted(hub.address.origin)) return null;
  return leaveDaemonHere({
    ...findHereParts(hub),
    remove: removeDaemon,
    revoke: (deviceId) => metricsApi(hub).revokeDaemonToken(deviceId),
    log: warnToProcessLog,
  });
}
