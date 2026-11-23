import type { SessionEndReason } from '../ending';

export type SocketConstructor = typeof WebSocket;

/** The token is used once, to connect, and never kept (K4 §1). */
export interface TunnelTicket {
  readonly tunnelPath: string;
  readonly token: string;
}

export type TunnelPhase = 'connecting' | 'connected' | 'disconnected';

/** What a tunnel knows; it holds no token, no URL and no gateway text (SEC-002, LOG-007). */
export interface TunnelState {
  readonly phase: TunnelPhase;
  readonly opened: boolean;
  readonly closeCode: number | undefined;
  readonly endReason?: SessionEndReason;
}

/** The message is fixed text: it never holds the token, the URL or a library error. */
export interface TunnelRefusal {
  readonly problem: 'bad-address' | 'not-opened';
  readonly message: string;
}

const REFUSALS: Readonly<Record<TunnelRefusal['problem'], string>> = {
  'bad-address': 'The hub gave a tunnel address that Anima.ing does not accept.',
  'not-opened': 'The session tunnel could not be opened.',
};

export function refusal(problem: TunnelRefusal['problem']): { readonly ok: false; readonly error: TunnelRefusal } {
  return { ok: false, error: { problem, message: REFUSALS[problem] } };
}
