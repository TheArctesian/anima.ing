import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Hub } from '$lib/hub/hub';
import { capabilities } from '$lib/platform/platform';
import { readThisMachine, type ThisMachine } from '../this-machine';

/** The shell's error codes (MET-019, D-10 d, HMD-056); any other answer is `failed`. */
export type DaemonShellProblem =
  | 'caller'
  | 'unavailable'
  | 'not-bundled'
  | 'busy'
  | 'code'
  | 'origin'
  | 'no-home'
  | 'untrusted'
  | 'copy'
  | 'enrol'
  | 'install'
  | 'uninstall'
  | 'remove'
  | 'timed-out'
  | 'unchecked'
  | 'unhealthy'
  | 'failed';

const MESSAGES: Record<DaemonShellProblem, string> = {
  caller: 'Only the main window can install the daemon.',
  unavailable: 'The daemon cannot be installed on this system from the app.',
  'not-bundled': 'This build carries no daemon. Use the install command.',
  busy: 'An install is already running.',
  code: 'The hub made a code in a form the daemon does not accept.',
  origin: 'The hub address is not a plain https origin on port 443.',
  'no-home': 'The app could not find your home folder.',
  untrusted:
    'The daemon installation folder or an ancestor can be changed by another user.',
  copy: 'The daemon could not be copied to its installation folder.',
  enrol: 'The daemon could not enrol. Retry to make a new code.',
  install: 'The daemon enrolled but did not start. Run `anima-daemon install` in a terminal to see why.',
  uninstall: 'The daemon did not take its unit and its token off this computer.',
  remove: 'The copy of the daemon in its installation folder could not be deleted.',
  'timed-out': 'The daemon did not finish in time.',
  unchecked: 'The new copy of the daemon did not pass its check.',
  unhealthy: 'The new daemon did not answer after its start.',
  failed: 'The install did not finish.',
};

const PROBLEMS = new Set<string>(Object.keys(MESSAGES));

/** The sentence for a problem of the shell, for a place that shows it later than the call (HMD-053). */
export function describeDaemonProblem(problem: DaemonShellProblem, agent = navigator.userAgent): string {
  if (problem !== 'untrusted') return MESSAGES[problem];
  const fix = /Windows NT/.test(agent)
    ? "Open the folder's Properties → Security and remove other users' write access."
    : 'Run `chmod g-w,o-w` on it, and `setfacl -b` for an ACL.';
  return `${MESSAGES.untrusted} ${fix}`;
}

export type DaemonShellResult = { readonly ok: true } | { readonly ok: false; readonly problem: DaemonShellProblem; readonly message: string };

/** The shell's own text is never kept. */
function problemOf(reason: unknown): DaemonShellProblem {
  const code = typeof reason === 'object' && reason !== null && 'code' in reason ? reason.code : undefined;
  return typeof code === 'string' && PROBLEMS.has(code) ? (code as DaemonShellProblem) : 'failed';
}

export async function daemonBundled(): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    return (await invoke('daemon_bundled')) === true;
  } catch {
    return false;
  }
}

/** Whether the daemon of this computer waits in the macOS Keychain (R-322). */
export type KeychainMark = 'waits' | 'free' | 'unknown';

export async function daemonKeychainMark(): Promise<KeychainMark> {
  if (!isTauri()) return 'free';
  try {
    return (await invoke('daemon_keychain_waits')) === true ? 'waits' : 'free';
  } catch {
    return 'unknown';
  }
}

export async function daemonKeychainWaits(): Promise<boolean> {
  return (await daemonKeychainMark()) === 'waits';
}

/** MET-019: the code goes to the shell and nowhere else; no answer and no message holds it (LOG-007). */
export async function installDaemon(code: string, origin: string): Promise<DaemonShellResult> {
  if (!isTauri()) return { ok: false, problem: 'unavailable', message: MESSAGES.unavailable };
  try {
    await invoke('daemon_install', { code, origin });
    return { ok: true };
  } catch (reason) {
    const problem = problemOf(reason);
    return { ok: false, problem, message: describeDaemonProblem(problem) };
  }
}

export type DaemonRemoval = { readonly ok: true; readonly wasHere: boolean } | { readonly ok: false; readonly problem: DaemonShellProblem; readonly message: string };

/** Removes the daemon's unit, token and copy (HMD-056). */
export async function removeDaemon(): Promise<DaemonRemoval> {
  if (!isTauri()) return { ok: true, wasHere: false };
  try {
    return { ok: true, wasHere: (await invoke('daemon_remove')) === true };
  } catch (reason) {
    const problem = problemOf(reason);
    return { ok: false, problem, message: describeDaemonProblem(problem) };
  }
}

export interface ThisComputerShell {
  readonly thisMachine: () => Promise<ThisMachine | null>;
  readonly bundled: () => Promise<boolean>;
  readonly install: (code: string, origin: string) => Promise<DaemonShellResult>;
}

/** `null` outside the desktop app (WP-42 D-4). */
export function thisComputerShell(hub: Hub): ThisComputerShell | null {
  if (!capabilities().thisMachine) return null;
  return { thisMachine: () => readThisMachine(hub), bundled: daemonBundled, install: installDaemon };
}
