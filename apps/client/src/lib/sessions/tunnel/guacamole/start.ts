import type { SessionsApi } from '../../api';
import { startWith, type RemoteTarget, type Started, type StartResult } from '../start';
import { openTunnel, type SessionTunnel, type TunnelOptions } from './tunnel';

export type StartedTunnel = Started<SessionTunnel>;

export type TunnelSetup = Omit<TunnelOptions, 'ticket' | 'waking'>;

/** RDP and VNC only; no SSH session uses it (R-062). */
export function startTunnel(
  api: Pick<SessionsApi, 'startSession' | 'endSession'>,
  target: RemoteTarget,
  setup: TunnelSetup,
): Promise<StartResult<SessionTunnel>> {
  return startWith(api, target, (ticket) => openTunnel({ ...setup, ticket, waking: ticket.wakeSent }));
}
