import type { HubError } from '$lib/hub/hub';
import type { Protocol, RemoteProtocol, SessionDisplay, SessionTicket, SessionsApi } from '../api';
import type { TunnelRefusal } from './state';
import { openTerminalLink, type LinkOptions, type TerminalLink } from './terminal/link';

export interface SessionTarget {
  readonly deviceId: string;
  readonly display: SessionDisplay;
}

export interface ProtocolTarget extends SessionTarget {
  readonly protocol: Protocol;
  readonly viewOnly?: boolean;
}

export interface RemoteTarget extends ProtocolTarget {
  readonly protocol: RemoteProtocol;
}

/** The ticket is not part of a started session (SEC-002). */
export interface Started<Tunnel> {
  readonly sessionId: string;
  readonly tunnel: Tunnel;
  /** A Wake-on-LAN packet was sent for this start (K4 §1): the opening may take up to 120 s. */
  readonly wakeSent: boolean;
}

export type StartResult<Tunnel> =
  | { readonly ok: true; readonly value: Started<Tunnel> }
  | {
      readonly ok: false;
      readonly error: HubError | TunnelRefusal;
      readonly wakeSent?: boolean;
    };

export type TunnelStart = StartResult<TerminalLink>;

export type TerminalSetup = Omit<LinkOptions, 'ticket'>;

export type Opening<Tunnel> =
  | { readonly ok: true; readonly value: Tunnel }
  | { readonly ok: false; readonly error: TunnelRefusal };

/** Every call asks for a new single-use ticket, which goes straight to `open`; the caller never holds the token (D-3, D-5). */
export async function startWith<Tunnel>(
  api: Pick<SessionsApi, 'startSession' | 'endSession'>,
  target: ProtocolTarget,
  open: (ticket: SessionTicket) => Opening<Tunnel>,
): Promise<StartResult<Tunnel>> {
  const { deviceId, protocol, display, viewOnly } = target;
  const started = await (viewOnly === true
    ? api.startSession(deviceId, protocol, display, true)
    : api.startSession(deviceId, protocol, display));
  if (!started.ok) return started;
  const { sessionId, wakeSent } = started.value;
  const opening = open(started.value);
  if (opening.ok) return { ok: true, value: { sessionId, tunnel: opening.value, wakeSent } };
  // The session row will never get a tunnel; the refusal is reported, not a failed `endSession`.
  await api.endSession(sessionId);
  return { ...opening, wakeSent };
}

export function startTerminal(
  api: Pick<SessionsApi, 'startSession' | 'endSession'>,
  target: SessionTarget,
  setup: TerminalSetup,
): Promise<TunnelStart> {
  return startWith(api, { ...target, protocol: 'SSH' }, (ticket) => openTerminalLink({ ...setup, ticket }));
}
