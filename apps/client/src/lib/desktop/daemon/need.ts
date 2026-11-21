import type { DaemonNeed } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';
import { metricsApi } from '$lib/metrics/api';
import { warnToProcessLog } from '../process-log';
import { deviceHere, findHereParts, type FindHereParts } from './here';
import { daemonKeychainMark, type KeychainMark } from './shell';
import { thisComputerDevice } from './state.svelte';

/** How often the app reads the mark of the daemon of this computer while it runs. */
export const NEED_EVERY_MS = 60_000;

/** The longest one turn of the watch may take; a shell or a hub that does not answer ends the turn, not the watch. */
export const NEED_TURN_MS = 30_000;

/** What the report of the need of the daemon of this computer needs; each part is injected, so that tests need no shell and no hub. */
export interface DaemonNeedParts extends FindHereParts {
  /** Whether the daemon of this computer waits for the person in the macOS Keychain: its mark file (R-322), or unknown. */
  readonly keychainMark: () => Promise<KeychainMark>;
  /** Tells the hub what the daemon of the device waits for (`null` clears it); gives what the hub keeps after it. */
  readonly report: (deviceId: string, need: DaemonNeed | null) => Promise<HubResult<DaemonNeed | null>>;
  /** Whether the person is signed in to the hub; no part means always. Signed out, the app asks the hub nothing. */
  readonly signedIn?: () => boolean;
  /** Takes one line for the log of the app: fixed text, never a name or an address. */
  readonly log?: (line: string) => void;
  /** Takes the device of this computer when the report finds it, so that its page can say "this computer". */
  readonly found?: (deviceId: string) => void;
}

/** What the hub holds as far as this start of the app knows: `undefined` before the first report. */
export type Reported = DaemonNeed | null | undefined;

/** What one start of the app knows: what the hub holds, and the daemon device of this computer once it was found. */
export interface NeedState {
  readonly reported: Reported;
  readonly deviceId?: string;
}

/** Answers of the hub that the same call would get again: no daemon token for the device, or a token that may not set it. */
const SETTLED = new Set<string>(['NOT_FOUND', 'FORBIDDEN']);

/** Where the daemon device of this computer stands: found, not a daemon device (the hub would refuse), or not known yet. */
type Lookup = { readonly kind: 'found'; readonly id: string } | { readonly kind: 'not-daemon' } | { readonly kind: 'again' };

async function lookUp(parts: DaemonNeedParts): Promise<Lookup> {
  const device = await deviceHere(parts);
  if (device === 'not-a-device-yet') return { kind: 'not-daemon' };
  if (typeof device === 'string') return { kind: 'again' };
  return device.metricsSource === 'DAEMON' ? { kind: 'found', id: device.id } : { kind: 'not-daemon' };
}

/**
 * Reads the mark once and tells the hub only when it differs from what the hub holds; gives what this start knows after
 * it. The daemon has no token while it waits, so the app, which is signed in, tells the hub for it (R-322). A mark that
 * was not read, a sign-out, a lookup that did not end or a hub that did not answer leave the state as it was: the next
 * reading tries again. A computer that is no daemon device, and a refusal that would come again, settle the need: the
 * app asks again only when the mark changes.
 */
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

/** Whether the hub settled `need` with its `answer` (it holds it, or would refuse it again), and the line for the log. */
function outcome(answer: HubResult<DaemonNeed | null>, need: DaemonNeed | null): readonly [boolean, string] {
  if (!answer.ok && SETTLED.has(answer.error.code)) {
    return [true, `the daemon of this computer: the hub refused what it waits for (${answer.error.code}); the app asks again when the mark changes`];
  }
  if (!answer.ok) return [false, 'the daemon of this computer: the hub did not take what it waits for; the app tries again in a minute'];
  // A batch of the daemon came just before: the hub keeps no need now, and the next reading tells it again.
  if (answer.value !== need) return [false, 'the daemon of this computer: the hub kept no need, as a batch came just now; the app tries again in a minute'];
  return [true, need === null ? 'the daemon of this computer waits for nothing: the hub knows' : 'the daemon of this computer waits for you in the Keychain: the hub knows'];
}

/** `turn`, or `fallback` once `limit` ms passed with no answer. */
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

/**
 * Reads the mark of the daemon of this computer at once and then every `every` ms, and tells the hub when it appears or
 * goes: the first reading of each start, then each change, never each minute. A turn that takes longer than `limit` ms
 * ends, and the next one starts on time. Gives the function that stops it.
 */
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

/** The watch for the computer the app runs on, with the calls of `hub` and of the desktop shell. */
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
