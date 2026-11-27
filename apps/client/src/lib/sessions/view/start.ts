import type { HubError } from '$lib/hub/hub';
import type { RemoteProtocol, SessionDisplay, SessionsApi } from '../api';
import { startTunnel } from '../tunnel/guacamole/start';
import type { GuacamoleState, SessionTunnel } from '../tunnel/guacamole/tunnel';
import { startTerminal, type Started, type StartResult } from '../tunnel/start';
import type { SocketConstructor, TunnelRefusal, TunnelState } from '../tunnel/state';
import type { CellSize } from '../tunnel/terminal/codec';
import type { TerminalLink } from '../tunnel/terminal/link';
import { readPolicy, type SessionPolicy } from '../transfer';

export interface TunnelEvents {
  /** The fitted terminal size in cells, read when the socket opens (K4 §8.4). */
  readonly size: () => CellSize;
  readonly onbytes: (bytes: Uint8Array) => void;
  readonly onchange: (state: TunnelState) => void;
  readonly onend: (state: TunnelState) => void;
}

export interface RemoteEvents {
  readonly onchange: (state: GuacamoleState) => void;
  readonly onend: (state: GuacamoleState) => void;
}

export interface PolicedSession<Tunnel> extends Started<Tunnel> {
  readonly policy: SessionPolicy;
}

export type ViewStartResult<Tunnel> =
  | { readonly ok: true; readonly value: PolicedSession<Tunnel> }
  | { readonly ok: false; readonly error: HubError | TunnelRefusal; readonly wakeSent?: boolean };

export type TerminalStartResult = ViewStartResult<TerminalLink>;

export type RemoteStartResult = ViewStartResult<SessionTunnel>;

/** Each call asks for a new single-use ticket (D-5); the view never sees the ticket or its token (SEC-002). */
export type SessionStart = (display: SessionDisplay, events: TunnelEvents) => Promise<TerminalStartResult>;

export type RemoteStart = (display: SessionDisplay, events: RemoteEvents) => Promise<RemoteStartResult>;

export interface DeviceStarts {
  readonly terminal: SessionStart;
  readonly remote: (protocol: RemoteProtocol, viewOnly?: boolean) => RemoteStart;
}

async function policed<Tunnel>(policy: Promise<SessionPolicy>, start: Promise<StartResult<Tunnel>>): Promise<ViewStartResult<Tunnel>> {
  const [read, result] = await Promise.all([policy, start]);
  return result.ok ? { ok: true, value: { ...result.value, policy: read } } : result;
}

export function sessionStart(
  api: Pick<SessionsApi, 'startSession' | 'endSession' | 'transferPolicy'>,
  deviceId: string,
  origin: string,
  WebSocket?: SocketConstructor,
): DeviceStarts {
  return {
    terminal: (display, events) =>
      policed(readPolicy(api, deviceId), startTerminal(api, { deviceId, display }, { origin, WebSocket, ...events })),
    remote: (protocol, viewOnly = false) => (display, events) => {
      const policy = viewOnly ? Promise.resolve({ read: true, copy: false, paste: false }) : readPolicy(api, deviceId);
      return policed(policy, startTunnel(api, { deviceId, protocol, display, viewOnly }, { origin, WebSocket, ...events }));
    },
  };
}
