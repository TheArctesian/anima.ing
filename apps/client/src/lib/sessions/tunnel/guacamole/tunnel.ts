// RDP and VNC use guacd through this tunnel; SSH uses the terminal tunnel and imports none of it (K4 Section 8).
import Guacamole from '../vendor/guacamole.js';
import { withObservedSocket } from './socket';
import { refusal, type SocketConstructor, type TunnelPhase, type TunnelRefusal, type TunnelState, type TunnelTicket } from '../state';
import { tunnelUrl } from '../url';

export type { SocketConstructor, TunnelRefusal, TunnelTicket } from '../state';

/** The 1.5.5 client states as phases: `waiting` lasts until the gateway's first frame (`sync`). */
export type GuacamolePhase = TunnelPhase | 'waiting';

export interface DisplayChoices {
  readonly active: string;
  readonly monitors: readonly string[];
}

/** What the Guacamole tunnel knows; it holds no token, no URL and no gateway text (SEC-002, LOG-007). */
export interface GuacamoleState extends Omit<TunnelState, 'phase'> {
  readonly phase: GuacamolePhase;
  /** The code of the last gateway `error` instruction; its text is never kept (R-035). */
  readonly gatewayError: number | undefined;
  /** The last gateway `required` names (GW-015), kept so that one that arrives before the start answered is not lost. */
  readonly required?: readonly string[] | undefined;
  readonly displays?: DisplayChoices;
  readonly displayPending?: boolean;
}

export interface TunnelOptions {
  readonly origin: string;
  readonly ticket: TunnelTicket;
  /** The hub sent a Wake-on-LAN packet, so opening may take up to `WAKE_OPENING_LIMIT_MS` (K4 §1). */
  readonly waking: boolean;
  readonly onchange?: (state: GuacamoleState) => void;
  readonly onend?: (state: GuacamoleState) => void;
  readonly WebSocket?: SocketConstructor;
}

/** The tunnel owns `client.onstatechange` and `client.onerror`; set any other hook on the client directly. */
export interface SessionTunnel {
  readonly client: Guacamole.Client;
  state(): GuacamoleState;
  selectDisplay?(id: string): void;
  /** Safe to call more than once. */
  close(): void;
}

export type TunnelOpening =
  | { readonly ok: true; readonly value: SessionTunnel }
  | { readonly ok: false; readonly error: TunnelRefusal };

/** The library's own limit on silence, in ms: it also bounds the opening, from `connect` to the first instruction. */
const RECEIVE_LIMIT_MS = 15_000;
export const WAKE_OPENING_LIMIT_MS = 120_000;

/** The 1.5.5 client states (private in the library), as phases; 0 IDLE follows a failed `connect`. */
const PHASES: readonly GuacamolePhase[] = ['disconnected', 'connecting', 'waiting', 'connected', 'disconnected', 'disconnected'];

interface Events {
  readonly onchange?: (state: GuacamoleState) => void;
  readonly onend?: (state: GuacamoleState) => void;
}

/** Nothing in this scope sees the token. */
function trackedTunnel(url: string, events: Events, waking: boolean) {
  const tunnel = new Guacamole.WebSocketTunnel(url);
  // A waking device may take the hub longer than `receiveTimeout` to open the socket; the wider limit lasts until it is open.
  tunnel.receiveTimeout = waking ? WAKE_OPENING_LIMIT_MS : RECEIVE_LIMIT_MS;
  const client = new Guacamole.Client(tunnel);
  const initial: GuacamoleState = { phase: 'connecting', opened: false, gatewayError: undefined, closeCode: undefined };
  let state = initial;
  // `onchange` is held back while `connect` runs (the global WebSocket is swapped then, see `socket.ts`).
  let connected = false;
  const update = (change: Partial<GuacamoleState>) => {
    if (Object.entries(change).every(([key, value]) => state[key as keyof GuacamoleState] === value)) return;
    state = { ...state, ...change };
    if (connected) events.onchange?.(state);
  };
  const release = () => {
    connected = true;
    if (state !== initial) events.onchange?.(state);
  };
  const instruction = tunnel.oninstruction;
  tunnel.oninstruction = (opcode, args) => {
    if (opcode === 'displays') {
      const [active, ...monitors] = args;
      if (active === undefined || monitors.length > 64 || monitors.some((id) => !/^\d{1,10}$/.test(id))) return;
      if (active !== 'all' && !monitors.includes(active)) return;
      client.getDisplay().flush(() => {
        update({ displays: { active, monitors }, displayPending: false });
      });
    } else {
      instruction?.(opcode, args);
    }
  };
  client.onstatechange = (clientState) => {
    update({ phase: PHASES[clientState] ?? 'disconnected' });
  };
  client.onerror = (status) => {
    update({ gatewayError: status.code });
  };
  client.onrequired = (parameters) => {
    update({ required: [...parameters] });
  };
  tunnel.onstatechange = (tunnelState) => {
    if (tunnelState === Guacamole.Tunnel.State.OPEN) {
      tunnel.receiveTimeout = RECEIVE_LIMIT_MS;
      update({ opened: true });
    }
    // The 1.5.5 client does not watch its tunnel; stop it so that its keep-alive timer ends too.
    if (tunnelState === Guacamole.Tunnel.State.CLOSED) client.disconnect();
  };
  const socketClosed = (code: number) => {
    // A socket closed after a failed `connect` belongs to a tunnel that was refused, not ended.
    if (!connected) return;
    update({ closeCode: code });
    // The listener runs before the library handles the close, so the end is reported after it.
    queueMicrotask(() => {
      events.onend?.(state);
    });
  };
  const handle: SessionTunnel = {
    client,
    state: () => state,
    selectDisplay: (id) => {
      const displays = state.displays;
      if (displays === undefined || id === displays.active || state.displayPending === true) return;
      if (id !== 'all' && !displays.monitors.includes(id)) return;
      update({ displayPending: true });
      tunnel.sendMessage('display', id);
    },
    close: () => {
      client.disconnect();
    },
  };
  // After a `connect` that threw there is no socket and 1.5.5's `disconnect` would call `close` on null; a closed tunnel only clears its timers.
  const abandon = () => {
    tunnel.setState(Guacamole.Tunnel.State.CLOSED);
  };
  return { handle, socketClosed, abandon, release };
}

/** The connect data is `token=<token>` and nothing else (K4 §2, GW-013). */
export function openTunnel(options: TunnelOptions): TunnelOpening {
  const url = tunnelUrl(options.origin, options.ticket.tunnelPath);
  if (url === undefined) return refusal('bad-address');
  const { handle, socketClosed, abandon, release } = trackedTunnel(
    url,
    { onchange: options.onchange, onend: options.onend },
    options.waking,
  );
  try {
    withObservedSocket(options.WebSocket ?? globalThis.WebSocket, socketClosed, () => {
      handle.client.connect(`token=${encodeURIComponent(options.ticket.token)}`);
    });
  } catch {
    // The WebSocket constructor's error names the URL, which holds the token; only the fact is kept (LOG-007).
    abandon();
    return refusal('not-opened');
  }
  release();
  return { ok: true, value: handle };
}
