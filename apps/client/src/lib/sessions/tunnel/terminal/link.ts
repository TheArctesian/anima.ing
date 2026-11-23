import { refusal, type SocketConstructor, type TunnelRefusal, type TunnelState, type TunnelTicket } from '../state';
import { tunnelUrl } from '../url';
import { TERMINAL_SUBPROTOCOL, clampSize, decodeHubMessage, encodePing, encodeResize, inputFrames, type CellSize } from './codec';

/** The client sends `ping` after this long without sending anything (K4 §8.4). */
export const PING_AFTER_MS = 5000;
/** No frame of any kind from the hub for this long counts as a lost connection (K4 §8.5, same limit as the hub's). */
export const SILENCE_LIMIT_MS = 15_000;

export const RESIZE_SETTLE_MS = 150;

const OPEN = 1;

export interface LinkEvents {
  /** The fitted size in cells, asked once when the socket opens: the first message must be `resize` (K4 §8.4). */
  readonly size: () => CellSize;
  readonly onbytes: (bytes: Uint8Array) => void;
  readonly onchange?: (state: TunnelState) => void;
  readonly onend?: (state: TunnelState) => void;
}

export interface LinkOptions extends LinkEvents {
  /** The hub origin, `https://host[:port]` (`hub.address.origin`). */
  readonly origin: string;
  readonly ticket: TunnelTicket;
  /** The WebSocket constructor; defaults to the global one (CQ-021). */
  readonly WebSocket?: SocketConstructor;
}

/** One open terminal tunnel (K4 §8). It holds no token and no URL (SEC-002, LOG-007). */
export interface TerminalLink {
  state(): TunnelState;
  /** Dropped unless the socket is open. */
  send(bytes: Uint8Array<ArrayBuffer>): void;
  resize(size: CellSize): void;
  /** Safe to call more than once. */
  close(): void;
}

export type LinkOpening =
  | { readonly ok: true; readonly value: TerminalLink }
  | { readonly ok: false; readonly error: TunnelRefusal };

/** The token lives only in the socket URL expression; the link keeps the events, not the ticket (K4 §8.1). */
export function openTerminalLink(options: LinkOptions): LinkOpening {
  const { origin, ticket, WebSocket: Socket = globalThis.WebSocket, ...events } = options;
  const url = tunnelUrl(origin, ticket.tunnelPath);
  if (url === undefined) return refusal('bad-address');
  let socket: WebSocket;
  try {
    socket = new Socket(`${url}?token=${encodeURIComponent(ticket.token)}`, TERMINAL_SUBPROTOCOL);
  } catch {
    // The constructor's error names the URL, which holds the token; only the fact is kept (LOG-007).
    return refusal('not-opened');
  }
  socket.binaryType = 'arraybuffer';
  return { ok: true, value: linkOver(socket, events) };
}

function liveness(ping: () => void, lost: () => void) {
  let pingTimer: ReturnType<typeof setTimeout> | undefined;
  let silenceTimer: ReturnType<typeof setTimeout> | undefined;
  return {
    sent: () => {
      clearTimeout(pingTimer);
      pingTimer = setTimeout(ping, PING_AFTER_MS);
    },
    heard: () => {
      clearTimeout(silenceTimer);
      silenceTimer = setTimeout(lost, SILENCE_LIMIT_MS);
    },
    stop: () => {
      clearTimeout(pingTimer);
      clearTimeout(silenceTimer);
    },
  };
}

/** A size the hub already holds is not sent again; `later` waits for `RESIZE_SETTLE_MS` of no change (K4 §8.4). */
function sizing(send: (frame: string) => boolean) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let sent: CellSize | undefined;
  const now = (size: CellSize) => {
    const clamped = clampSize(size);
    if (sent?.cols === clamped.cols && sent.rows === clamped.rows) return;
    if (send(encodeResize(clamped))) sent = clamped;
  };
  return {
    now,
    later: (size: CellSize) => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        now(size);
      }, RESIZE_SETTLE_MS);
    },
    stop: () => {
      clearTimeout(timer);
    },
  };
}

/** Only an `end` control message changes the state; a ping or an unknown message changes nothing (K4 §8.4). */
function fromHub(data: unknown, onbytes: LinkEvents['onbytes']): Partial<TunnelState> | undefined {
  if (data instanceof ArrayBuffer) {
    onbytes(new Uint8Array(data));
    return undefined;
  }
  const message = typeof data === 'string' ? decodeHubMessage(data) : undefined;
  return message?.kind === 'end' ? { endReason: message.reason } : undefined;
}

function linkOver(socket: WebSocket, events: LinkEvents): TerminalLink {
  let state: TunnelState = { phase: 'connecting', opened: false, closeCode: undefined };
  let finished = false;
  let closing = false;

  const publish = (change: Partial<TunnelState>) => {
    state = { ...state, ...change };
    events.onchange?.(state);
  };
  const finish = (change: Partial<TunnelState>) => {
    if (finished) return;
    finished = true;
    beats.stop();
    size.stop();
    publish({ phase: 'disconnected', ...change });
    events.onend?.(state);
  };
  const transmit = (frame: string | Uint8Array<ArrayBuffer>): boolean => {
    if (closing || finished || socket.readyState !== OPEN) return false;
    socket.send(frame);
    beats.sent();
    return true;
  };
  const beats = liveness(
    () => transmit(encodePing()),
    () => {
      // Nothing came for too long: report the end now; a dead connection may take long to answer a close.
      finish({});
      socket.close();
    },
  );
  const size = sizing(transmit);

  socket.onopen = () => {
    size.now(events.size());
    beats.heard();
    publish({ phase: 'connected', opened: true });
  };
  socket.onmessage = (event: MessageEvent) => {
    beats.heard();
    const change = fromHub(event.data, events.onbytes);
    if (change !== undefined) publish(change);
  };
  socket.onclose = (event: CloseEvent) => {
    finish({ closeCode: event.code });
  };

  return {
    state: () => state,
    send: (bytes) => {
      for (const frame of inputFrames(bytes)) transmit(frame);
    },
    resize: size.later,
    close: () => {
      closing = true;
      beats.stop();
      size.stop();
      socket.close(1000);
    },
  };
}
