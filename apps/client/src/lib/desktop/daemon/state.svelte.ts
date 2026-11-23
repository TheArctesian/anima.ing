import type { DaemonHere } from './here';
import { describeDaemonProblem } from './shell';

/** The daemon install on this computer (HMD-053): known only on the computer that runs the app; no hub field holds it. */
export type DaemonHereState =
  | { readonly kind: 'idle' }
  | { readonly kind: 'installing' }
  | { readonly kind: 'installed' }
  | { readonly kind: 'needs-you' }
  | { readonly kind: 'failed'; readonly reason: string };

const NOT_FOUND = 'The application did not find this computer among the devices of the hub.';
const NO_CODE = 'The hub made no enrolment code.';

export function stateAfter(did: DaemonHere): DaemonHereState {
  switch (did.kind) {
    case 'installed':
    case 'present':
      return { kind: 'installed' };
    case 'needs-you':
      return { kind: 'needs-you' };
    case 'not-ready':
      return { kind: 'failed', reason: NOT_FOUND };
    case 'failed':
      return { kind: 'failed', reason: did.step === 'enrol' ? NO_CODE : describeDaemonProblem(did.step) };
  }
}

/** R-322: `null` names the daemon of the computer that shows the page, else the device seen from another. */
export function keychainNeedLine(deviceName: string | null): string {
  if (deviceName === null) {
    return 'The daemon of this computer needs you: macOS asks whether it may read its key in the Keychain. Type your password in that window and click Always Allow. Metrics and counts appear here some minutes later.';
  }
  return `The daemon of ${deviceName} needs you: on that Mac, macOS asks whether it may read its key in the Keychain. Allow it there.`;
}

/** One sentence for a page of a hosted hub that waits for the daemon of this computer (AIS-020, HMD-057). */
export function daemonHereLine(state: DaemonHereState): string {
  switch (state.kind) {
    case 'idle':
      return 'Open the Anima.ing application on a computer and sign in: it installs the daemon there.';
    case 'installing':
      return 'The Anima.ing application is installing the daemon on this computer. Counts appear here after its first report.';
    case 'installed':
      return 'The daemon of this computer is installed. Counts appear here some minutes after you use Claude Code or Codex on it.';
    case 'needs-you':
      return keychainNeedLine(null);
    case 'failed':
      return `The daemon was not installed on this computer. ${state.reason} Quit the application and open it again to try once more.`;
  }
}

/** Pages read it; only the install writes it. */
export const daemonHere = $state<{ current: DaemonHereState }>({ current: { kind: 'idle' } });

/** The device of the hub that is the computer the app runs on, once the watch of its daemon found it; `null` before. */
export const thisComputerDevice = $state<{ id: string | null }>({ id: null });
