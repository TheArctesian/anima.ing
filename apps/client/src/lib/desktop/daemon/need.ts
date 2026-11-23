import type { DaemonNeed } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';
import { metricsApi } from '$lib/metrics/api';
import { warnToProcessLog } from '../process-log';
import { deviceHere, findHereParts, type FindHereParts } from './here';
import { daemonKeychainMark, type KeychainMark } from './shell';
import { thisComputerDevice } from './state.svelte';

export const NEED_EVERY_MS = 60_000;

/** The longest one turn of the watch may take; a shell or a hub that does not answer ends the turn, not the watch. */
export const NEED_TURN_MS = 30_000;

export interface DaemonNeedParts extends FindHereParts {
  readonly keychainMark: () => Promise<KeychainMark>;
  readonly report: (deviceId: string, need: DaemonNeed | null) => Promise<HubResult<DaemonNeed | null>>;
  /** No part means always signed in; signed out, the app asks the hub nothing. */
  readonly signedIn?: () => boolean;
  /** Takes one line for the log of the app: fixed text, never a name or an address. */
  readonly log?: (line: string) => void;
  readonly found?: (deviceId: string) => void;
}

export type Reported = DaemonNeed | null | undefined;

export interface NeedState {
  readonly reported: Reported;
  readonly deviceId?: string;
}

/** Answers of the hub that the same call would get again: no daemon token for the device, or a token that may not set it. */
const SETTLED = new Set<string>(['NOT_FOUND', 'FORBIDDEN']);

type Lookup = { readonly kind: 'found'; readonly id: string } | { readonly kind: 'not-daemon' } | { readonly kind: 'again' };

async function lookUp(parts: DaemonNeedParts): Promise<Lookup> {
  const device = await deviceHere(parts);
  if (device === 'not-a-device-yet') return { kind: 'not-daemon' };
  if (typeof device === 'string') return { kind: 'again' };
  return device.metricsSource === 'DAEMON' ? { kind: 'found', id: device.id } : { kind: 'not-daemon' };
}

/** The daemon has no token while it waits, so the signed-in app tells the hub for it (R-322). */
export async function reportDaemonNeed(parts: DaemonNeedParts, state: NeedState): Promise<NeedState> {
  const mark = await parts.keychainMark();
  if (mark === 'unknown') return state;
  const need: DaemonNeed | null = mark === 'waits' ? 'KEYCHAIN' : null;
  if (need === state.reported || parts.signedIn?.() === false) return state;
  const lookup: Lookup = state.deviceId === undefined ? await lookUp(parts) : { kind: 'found', id: state.deviceId };
  if (lookup.kind === 'again') return state;
  if (lookup.kind === 'not-daemon') return { reported: need };
  if (state.deviceId === undefined) parts.found?.(lookup.id);
  const answer = await parts.report(lookup.id, need);
  const [taken, line] = outcome(answer, need);
  (parts.log ?? ((): void => undefined))(line);
  return { deviceId: lookup.id, reported: taken ? need : state.reported };
}

function outcome(answer: HubResult<DaemonNeed | null>, need: DaemonNeed | null): readonly [boolean, string] {
  if (!answer.ok && SETTLED.has(answer.error.code)) {
    return [true, `the daemon of this computer: the hub refused what it waits for (${answer.error.code}); the app asks again when the mark changes`];
  }
  if (!answer.ok) return [false, 'the daemon of this computer: the hub did not take what it waits for; the app tries again in a minute'];
  // A batch of the daemon came just before: the hub keeps no need now, and the next reading tells it again.
  if (answer.value !== need) return [false, 'the daemon of this computer: the hub kept no need, as a batch came just now; the app tries again in a minute'];
  return [true, need === null ? 'the daemon of this computer waits for nothing: the hub knows' : 'the daemon of this computer waits for you in the Keychain: the hub knows'];
}

async function within<T>(turn: Promise<T>, limit: number, fallback: T): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<T>((resolve) => {
    timer = setTimeout(() => resolve(fallback), limit);
  });
  try {
    return await Promise.race([turn, late]);
  } finally {
    clearTimeout(timer);
  }
}

export function watchDaemonNeed(parts: DaemonNeedParts, every = NEED_EVERY_MS, limit = NEED_TURN_MS): () => void {
  let state: NeedState = { reported: undefined };
  let busy = false;
  const log = parts.log ?? ((): void => undefined);
  const read = async (): Promise<void> => {
    if (busy) return;
    busy = true;
    try {
      const before = state;
      state = await within(reportDaemonNeed(parts, before), limit, before);
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

export function watchDaemonNeedHere(hub: Hub, signedIn: () => boolean): () => void {
  return watchDaemonNeed({
    ...findHereParts(hub),
    keychainMark: daemonKeychainMark,
    report: (deviceId, need) => metricsApi(hub).setDaemonNeed(deviceId, need),
    signedIn,
    log: warnToProcessLog,
    found: (deviceId) => {
      thisComputerDevice.id = deviceId;
    },
  });
}
