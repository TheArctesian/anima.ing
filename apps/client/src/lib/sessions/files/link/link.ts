import { refusal, type SocketConstructor, type TunnelPhase, type TunnelRefusal, type TunnelTicket } from '../../tunnel/state';
import { tunnelUrl } from '../../tunnel/url';
import { endOfClose, type FilesEnd } from './closing';
import { CANCEL_MESSAGE, FILES_SUBPROTOCOL, PING_MESSAGE, encodeRequest, isByteCount, type FilesRequest, type Listing } from './codec';
import { abandon, hearFrame, stopTransfer, type FilesResult, type InFlight, type Step } from './inflight';
import { pumpUpload, type Progress, type UploadPort, type UploadSource } from './upload';

/** The client sends `ping` after this long without sending anything, also during a download (K4 §9, §8.4). */
export const PING_AFTER_MS = 5000;
/** No frame of any kind from the hub for this long counts as a lost connection (K4 §8.4). */
export const SILENCE_LIMIT_MS = 15_000;

const OPEN = 1;

/** What a files link knows; it holds no token, no URL, no path and no device text (SEC-002, LOG-007). */
export interface FilesLinkState {
  readonly phase: TunnelPhase;
  readonly opened: boolean;
  /** The WebSocket close code, when the socket closed before the client gave up on it. */
  readonly closeCode: number | undefined;
  readonly end: FilesEnd | undefined;
}

export interface FilesLinkOptions {
  /** The hub origin, `https://host[:port]` (`hub.address.origin`). */
  readonly origin: string;
  /** From `openFiles` (K4 §9). Its token is used once, to connect, and never kept. */
  readonly ticket: TunnelTicket;
  /** The WebSocket constructor; defaults to the global one (CQ-021). */
  readonly WebSocket?: SocketConstructor;
  /** The upload's wait while the socket drains; defaults to a timer (CQ-021). */
  readonly wait?: UploadPort['wait'];
  /** Hears every change of the state after `openFilesLink` returned. */
  readonly onchange?: (state: FilesLinkState) => void;
  /** Hears the final state once. */
  readonly onend?: (state: FilesLinkState) => void;
}

/** One files tunnel (K4 §9). One list or transfer at a time; results are typed values, never thrown. */
export interface FilesLink {
  state(): FilesLinkState;
  list(path: string): Promise<FilesResult<Listing>>;
  /** The file at `path` as one `Blob` of exactly the size the hub announced; `progress` hears received bytes. */
  download(path: string, progress?: Progress): Promise<FilesResult<Blob>>;
  /** A new file at `path` with the bytes of `source`; `progress` hears the bytes the socket has taken. */
  upload(path: string, source: UploadSource, progress?: Progress): Promise<FilesResult<undefined>>;
  /** Stop the transfer in flight and tell the hub; its result settles `cancelled` once the hub confirms. */
  cancel(): boolean;
  /** Close the socket (1000); a request in flight settles `ended`. Safe to call more than once. */
  close(): void;
}

export type FilesLinkOpening = { readonly ok: true; readonly value: FilesLink } | { readonly ok: false; readonly error: TunnelRefusal };

type Events = Pick<FilesLinkOptions, 'onchange' | 'onend'> & { readonly wait: UploadPort['wait'] };

/** A timer of `ms` that an abort ends at once, clearing the timer and its listener, so a stopped upload leaves none. */
const timer: UploadPort['wait'] = (ms, signal) =>
  new Promise<void>((resolve) => {
    if (signal.aborted) {
      resolve();
      return;
    }
    const over = () => {
      clearTimeout(handle);
      signal.removeEventListener('abort', over);
      resolve();
    };
    const handle = setTimeout(over, ms);
    signal.addEventListener('abort', over);
  });

/** Opens the files tunnel (K4 §9, §8.1); the token lives only in the socket URL, never in the link. */
export function openFilesLink(options: FilesLinkOptions): FilesLinkOpening {
  const { origin, ticket, WebSocket: Socket = globalThis.WebSocket, wait = timer, onchange, onend } = options;
  const url = tunnelUrl(origin, ticket.tunnelPath);
  if (url === undefined) return refusal('bad-address');
  let socket: WebSocket;
  try {
    socket = new Socket(`${url}?token=${encodeURIComponent(ticket.token)}`, FILES_SUBPROTOCOL);
  } catch {
    // The constructor's error names the URL, which holds the token; only the fact is kept (LOG-007).
    return refusal('not-opened');
  }
  socket.binaryType = 'arraybuffer';
  return { ok: true, value: linkOver(socket, { onchange, onend, wait }) };
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

const refusedLocally = <T>(kind: 'busy' | 'offline' | 'bad-request'): Promise<FilesResult<T>> => Promise.resolve({ ok: false, error: { kind } });

interface Channel {
  /** Send one frame if the socket is open; false when it was dropped. */
  readonly transmit: (frame: string | ArrayBuffer) => boolean;
  readonly buffered: () => number;
  readonly wait: UploadPort['wait'];
  current: InFlight | undefined;
}

function requests(channel: Channel): Pick<FilesLink, 'list' | 'download' | 'upload' | 'cancel'> {
  const ask = <T>(request: FilesRequest, begin: (settle: (result: FilesResult<T>) => void) => InFlight): Promise<FilesResult<T>> => {
    if (channel.current !== undefined) return refusedLocally('busy');
    const text = encodeRequest(request);
    if (text === undefined) return refusedLocally('bad-request');
    if (!channel.transmit(text)) return refusedLocally('offline');
    return new Promise((settle) => {
      channel.current = begin(settle);
    });
  };
  return {
    list: (path) => ask({ type: 'list', path }, (settle) => ({ kind: 'list', settle })),
    download: (path, progress = () => undefined) =>
      ask({ type: 'download', path }, (settle) => ({ kind: 'download', settle, progress, size: undefined, received: 0, chunks: [], stopped: undefined })),
    upload: (path, source, progress = () => undefined) => {
      if (!isByteCount(source.size)) return refusedLocally('bad-request');
      return ask({ type: 'upload', path, size: source.size }, (settle) => {
        const unreadable = () => {
          if (stopTransfer(channel.current, 'unreadable')) channel.transmit(CANCEL_MESSAGE);
        };
        const pump = pumpUpload(source, { send: channel.transmit, buffered: channel.buffered, wait: channel.wait }, progress, unreadable);
        return { kind: 'upload', settle, progress, size: source.size, pump, stopped: undefined };
      });
    },
    cancel: () => stopTransfer(channel.current, 'cancelled') && channel.transmit(CANCEL_MESSAGE),
  };
}

/** An open socket is the one check: an ended link always closes its socket. */
function channelOn(socket: WebSocket, sent: () => void, wait: Channel['wait']): Channel {
  return {
    transmit: (frame) => {
      if (socket.readyState !== OPEN) return false;
      socket.send(frame);
      sent();
      return true;
    },
    buffered: () => socket.bufferedAmount,
    wait,
    current: undefined,
  };
}

function linkOver(socket: WebSocket, events: Events): FilesLink {
  let state: FilesLinkState = { phase: 'connecting', opened: false, closeCode: undefined, end: undefined };
  const channel = channelOn(socket, () => beats.sent(), events.wait);

  const publish = (change: Partial<FilesLinkState>) => {
    state = { ...state, ...change };
    events.onchange?.(state);
  };
  const finish = (end: FilesEnd, closeCode?: number) => {
    if (state.phase === 'disconnected') return;
    beats.stop();
    const left = channel.current;
    channel.current = undefined;
    publish({ phase: 'disconnected', end, closeCode });
    abandon(left);
    events.onend?.(state);
  };
  /** The client gives up on the socket: the end is reported now, the close follows. */
  const giveUp = (end: FilesEnd) => {
    finish(end);
    socket.close(1000);
  };
  const beats = liveness(
    () => channel.transmit(PING_MESSAGE),
    () => {
      giveUp('lost');
    },
  );
  const step = (result: Step) => {
    if (result === 'breach') giveUp('misread');
    else if (result !== 'running') channel.current = undefined;
    if (result === 'over-cancel') channel.transmit(CANCEL_MESSAGE);
  };

  socket.onopen = () => {
    beats.heard();
    beats.sent();
    publish({ phase: 'connected', opened: true });
  };
  socket.onmessage = (event: MessageEvent) => {
    if (state.phase !== 'connected') return;
    beats.heard();
    step(hearFrame(channel.current, event.data));
  };
  socket.onclose = (event: CloseEvent) => {
    finish(endOfClose(event.code, state.opened), event.code);
  };

  return {
    state: () => state,
    ...requests(channel),
    close: () => {
      if (state.phase !== 'disconnected') giveUp('closed');
    },
  };
}
