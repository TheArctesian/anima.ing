import type { DaemonNeed } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';
import { metricsApi } from '$lib/metrics/api';
import { warnToProcessLog } from '../process-log';
import { deviceHere, findHereParts, type FindHereParts } from './here';
import { daemonKeychainWaits } from './shell';
import { thisComputerDevice } from './state.svelte';

/** How often the app reads the mark of the daemon of this computer while it runs. */
export const NEED_EVERY_MS = 60_000;

/** What the report of the need of the daemon of this computer needs; each part is injected, so that tests need no shell and no hub. */
export interface DaemonNeedParts extends FindHereParts {
  /** Whether the daemon of this computer waits for the person in the macOS Keychain: its mark file (R-322). */
  readonly keychainWaits: () => Promise<boolean>;
  /** Tells the hub what the daemon of the device waits for; `null` clears it. */
  readonly report: (deviceId: string, need: DaemonNeed | null) => Promise<HubResult<unknown>>;
  /** Takes one line for the log of the app: fixed text, never a name or an address. */
  readonly log?: (line: string) => void;
  /** Takes the device of this computer each time the report finds it, so that its page can say "this computer". */
  readonly found?: (deviceId: string) => void;
}

/** What the hub holds as far as this start of the app knows: `undefined` before the first report. */
export type Reported = DaemonNeed | null | undefined;

/**
 * Reads the mark once and tells the hub only when it differs from `reported`; gives what the hub holds after it. The
 * daemon has no token while it waits, so the app, which is signed in, tells the hub for it (R-322). A computer that is
 * not a daemon device of the hub yet, or a refused call, leaves `reported` as it was: the next reading tries again.
 */
export async function reportDaemonNeed(parts: DaemonNeedParts, reported: Reported): Promise<Reported> {
  const need: DaemonNeed | null = (await parts.keychainWaits()) ? 'KEYCHAIN' : null;
  if (need === reported) return reported;
  const device = await deviceHere(parts);
  if (typeof device === 'string' || device.metricsSource !== 'DAEMON') return reported;
  parts.found?.(device.id);
  const log = parts.log ?? ((): void => undefined);
  const done = await parts.report(device.id, need);
  if (!done.ok) {
    log('the daemon of this computer: the hub did not take what it waits for; the app tries again in a minute');
    return reported;
  }
  log(need === null ? 'the daemon of this computer waits for nothing: the hub knows' : 'the daemon of this computer waits for you in the Keychain: the hub knows');
  return need;
}

/**
 * Reads the mark of the daemon of this computer at once and then every `every` ms, and tells the hub when it appears or
 * goes: the first reading of each start, then each change, never each minute. Gives the function that stops it.
 */
export function watchDaemonNeed(parts: DaemonNeedParts, every = NEED_EVERY_MS): () => void {
  let reported: Reported = undefined;
  let busy = false;
  const log = parts.log ?? ((): void => undefined);
  const read = async (): Promise<void> => {
    if (busy) return;
    busy = true;
    try {
      reported = await reportDaemonNeed(parts, reported);
    } catch {
      log('the daemon of this computer: its mark was not read; the app tries again in a minute');
    } finally {
      busy = false;
    }
  };
  void read();
  const timer = setInterval(() => void read(), every);
  return () => {
    clearInterval(timer);
  };
}

/** The watch for the computer the app runs on, with the calls of `hub` and of the desktop shell. */
export function watchDaemonNeedHere(hub: Hub): () => void {
  return watchDaemonNeed({
    ...findHereParts(hub),
    keychainWaits: daemonKeychainWaits,
    report: (deviceId, need) => metricsApi(hub).setDaemonNeed(deviceId, need),
    log: warnToProcessLog,
    found: (deviceId) => {
      thisComputerDevice.id = deviceId;
    },
  });
}
