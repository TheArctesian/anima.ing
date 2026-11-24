import type { DeviceDetail } from '$lib/devices/api';
import type { ThisMachine } from '$lib/desktop/this-machine';
import { normalizeHost } from '$lib/devices/form/host';
import type { ConnectionTestInput, OperatingSystem, UpdateDeviceInput } from '$lib/hub/generated/graphql';
import type { HubResult } from '$lib/hub/hub';
import type { ConnectionTest } from '$lib/sessions/api';

/** The port the flow tests and turns on: every Screen Sharing and VNC server listens on 5900 (R-079). */
export const SCREEN_SHARING_PORT = 5900;

export const RETEST_INTERVAL_MS = 3000;

export const NO_ANSWER_TEXT = `No answer on port ${String(SCREEN_SHARING_PORT)}. Screen Sharing (or the VNC server) is off, or the VPN rule blocks the port.`;

export const SHARING_PROMPT = 'Turn on Screen Sharing';

export interface SetupDevice {
  readonly id: string;
  readonly name: string;
  readonly host: string;
  readonly os: OperatingSystem;
  readonly vnc: { readonly enabled: boolean; readonly username?: string | null };
}

export function offersScreenSharing(device: Pick<SetupDevice, 'os' | 'vnc'>): boolean {
  return (device.os === 'MACOS' || device.os === 'LINUX') && !device.vnc.enabled;
}

/** R-060: the device whose host is this computer's VPN address is this computer. */
export function isThisComputer(device: Pick<SetupDevice, 'host'>, machine: ThisMachine | null): machine is ThisMachine {
  return machine !== null && normalizeHost(device.host) === machine.vpnAddress;
}

/** A Mac's Screen Sharing signs in an account; a Linux VNC server such as wayvnc refuses a name it does not expect (R-079, GH-53). */
export function screenSharingUser(machine: Pick<ThisMachine, 'os' | 'user'>): string | null {
  return machine.os === 'MACOS' ? machine.user : null;
}

/** The unsaved test of the VNC port (WP-35 D-1). It carries no credential, and a stale stored port cannot change it. */
export function screenSharingTest(device: Pick<SetupDevice, 'host'>): ConnectionTestInput {
  return { host: device.host, protocol: 'VNC', vnc: { enabled: true, port: SCREEN_SHARING_PORT } };
}

/** The hub replaces the whole VNC settings, so the update keeps a stored username; no password. */
export function enableVnc(user: string | null, stored: string | null | undefined): UpdateDeviceInput {
  const username = [user, stored].find((name) => name !== null && name !== undefined && name !== '') ?? null;
  return { vnc: { enabled: true, port: SCREEN_SHARING_PORT, username } };
}

export interface CardScreenSharing {
  readonly calls: ScreenSharingCalls;
  readonly onReady: (device: DeviceDetail, button: HTMLElement) => void;
}

export type SetupState =
  | { readonly kind: 'idle' }
  | { readonly kind: 'testing' }
  | { readonly kind: 'waiting'; readonly problem: string | null }
  | { readonly kind: 'starting' }
  | { readonly kind: 'failed'; readonly message: string };

/** Failures have their own alert, so they add nothing to the live text. */
export function liveText(state: SetupState): string {
  switch (state.kind) {
    case 'testing':
      return 'Checking port 5900.';
    case 'waiting':
      return `${SHARING_PROMPT} in System Settings → General → Sharing. Click the ⓘ next to Screen Sharing and allow your Mac account. Anima checks again automatically.`;
    case 'starting':
      return 'Turning on screen sharing.';
    default:
      return '';
  }
}

export function isBusy(state: SetupState): boolean {
  return state.kind !== 'idle' && state.kind !== 'failed';
}

export type After = (ms: number, run: () => void) => () => void;

export interface ScreenSharingCalls {
  test: (input: ConnectionTestInput) => Promise<HubResult<ConnectionTest>>;
  update: (id: string, input: UpdateDeviceInput) => Promise<HubResult<DeviceDetail>>;
  thisMachine: () => Promise<ThisMachine | null>;
  sharingAvailable: () => boolean;
  /** Opens System Settings at Sharing; rejects with a `SystemSettingsError`. */
  openSharing: () => Promise<void>;
  after: After;
  intervalMs: number;
}
