import type { TypedDocumentNode } from '@graphql-typed-document-node/core';
import { print } from 'graphql';
import { createClient, type Client, type Sink as ClientSink } from 'graphql-ws';
import type { HubAddress } from '../address';
import { MALFORMED, noAnswer, parseErrors, parseResponse, TOKEN_UNREADABLE, type HubError } from '../error';
import { readToken, type TokenProvider } from '../request';
import type { StatusSignal } from '../status/status';

/** After `error` or `complete`, nothing more arrives. */
export interface Sink<T> {
  next(value: T): void;
  error(error: HubError): void;
  complete(): void;
}

/** Every part is injected so that tests need no network (CQ-021). */
interface SocketTransport {
  readonly address: HubAddress;
  readonly WebSocket: typeof WebSocket;
  readonly token: TokenProvider;
  readonly signal: (signal: StatusSignal) => void;
  /** The wait for `connection_ack` before closing and retrying (NET-004). */
  readonly timeoutMs: number;
}

export interface Socket {
  subscribe<TResult, TVariables extends Record<string, unknown>>(
    document: TypedDocumentNode<TResult, TVariables>,
    variables: TVariables,
    sink: Sink<TResult>,
  ): () => void;
  /** Completes every subscription, also later ones; rejects only when an open socket could not be closed. */
  close(): Promise<void>;
}

/** The hub closes with 4403 when the token is missing or invalid (graphql-transport-ws `Forbidden`). */
const REFUSED_CLOSE_CODE = 4403;
const NORMAL_CLOSE_CODE = 1000;
/** graphql-transport-ws `InternalClientError`: the client closed its own socket. */
const CLIENT_CLOSE_CODE = 4005;
const SERVER_ERROR_CLOSE_CODE = 4500;
const REFUSED: HubError = { code: 'UNAUTHENTICATED', message: 'The hub refused the access token. Log in again.' };
const FIRST_RETRY_MS = 1000;
const LONGEST_RETRY_MS = 30_000;

function retryDelayMs(retries: number): number {
  return Math.min(FIRST_RETRY_MS * 2 ** retries, LONGEST_RETRY_MS);
}

function closeCode(value: unknown): number | undefined {
  if (typeof value !== 'object' || value === null || !('code' in value)) return undefined;
  return typeof value.code === 'number' ? value.code : undefined;
}

function closeSignal(event: unknown): StatusSignal | undefined {
  const code = closeCode(event);
  if (code === NORMAL_CLOSE_CODE || code === CLIENT_CLOSE_CODE) return undefined;
  return code === REFUSED_CLOSE_CODE ? 'refused' : 'closed';
}

function subscriptionError(reason: unknown, tokenUnreadable: boolean, origin: string): HubError {
  if (tokenUnreadable) return TOKEN_UNREADABLE;
  if (Array.isArray(reason)) return parseErrors(reason);
  const code = closeCode(reason);
  if (code === undefined) return noAnswer(origin);
  if (code === REFUSED_CLOSE_CODE) return REFUSED;
  if (code === SERVER_ERROR_CLOSE_CODE || code === CLIENT_CLOSE_CODE) {
    return { code: 'INTERNAL', message: 'The live connection to the hub failed.' };
  }
  return code >= 4000 && code <= 4999 ? MALFORMED : noAnswer(origin);
}

type Phase = 'opening' | 'live' | 'waiting';

interface SocketState {
  phase: Phase;
  closed: boolean;
  tokenUnreadable: boolean;
  /** Fires `failed` when a connection attempt has no `connection_ack` within the timeout, even if it never opens. */
  deadline: ReturnType<typeof setTimeout> | undefined;
  retryTimer: ReturnType<typeof setTimeout> | undefined;
}

function clearDeadline(state: SocketState): void {
  clearTimeout(state.deadline);
  state.deadline = undefined;
}

function isHubCloseCode(code: number | undefined): boolean {
  return code !== undefined && code >= 4000 && code <= 4999;
}

/** An unreadable token says nothing about the network; a 4xxx close means the hub answered. */
function stoppedSignal(reason: unknown, state: SocketState): StatusSignal | undefined {
  if (state.tokenUnreadable) return undefined;
  return isHubCloseCode(closeCode(reason)) ? 'answered' : 'failed';
}

function lifecycle(transport: SocketTransport, state: SocketState, refuse: () => void) {
  const signal = (value: StatusSignal | undefined): void => {
    if (value !== undefined && !state.closed) transport.signal(value);
  };
  return {
    connecting: () => {
      state.phase = 'opening';
      state.tokenUnreadable = false;
      clearDeadline(state);
      state.deadline = setTimeout(() => signal('failed'), transport.timeoutMs);
    },
    connected: () => {
      state.phase = 'live';
      clearDeadline(state);
      signal('acknowledged');
    },
    closed: (event: unknown) => {
      state.phase = 'waiting';
      clearDeadline(state);
      // The socket keeps probing after a refusal; the subscriptions end, so they are not sent again.
      if (closeCode(event) === REFUSED_CLOSE_CODE) refuse();
      signal(closeSignal(event));
    },
    stopped: (reason: unknown) => signal(stoppedSignal(reason, state)),
  };
}

function connect(transport: SocketTransport, state: SocketState, refuse: () => void): Client {
  const events = lifecycle(transport, state, refuse);
  return createClient({
    url: transport.address.socket,
    webSocketImpl: transport.WebSocket,
    // Connect at once and stay connected, so the status is true without any subscription (NAV-003, NET-004).
    lazy: false,
    onNonLazyError: events.stopped,
    // Read per connection, so a new login applies at the next reconnect; never a query parameter (LOG-007).
    connectionParams: async () => {
      const read = await readToken(transport.token, transport.address);
      if (!read.ok) {
        state.tokenUnreadable = true;
        // graphql-ws closes with 4005; the message is ours, never the provider's.
        throw new Error(read.error.message);
      }
      return read.value === null ? {} : { authorization: `Bearer ${read.value}` };
    },
    connectionAckWaitTimeout: transport.timeoutMs,
    retryAttempts: Number.POSITIVE_INFINITY,
    retryWait: (retries) =>
      new Promise((resolve) => {
        state.retryTimer = setTimeout(resolve, retryDelayMs(retries));
      }),
    // A refused token retries on the backoff, which resets only on `connection_ack`; each probe rereads the token (NET-004).
    shouldRetry: () => !state.tokenUnreadable,
    on: { connecting: events.connecting, connected: events.connected, closed: events.closed },
  });
}

function deliverTo<T>(sink: Sink<T>, tokenUnreadable: () => boolean, origin: string): ClientSink {
  return {
    next: (payload) => {
      const result = parseResponse(payload);
      // The document was checked against the schema at build time (API-008); the hub answers in that shape.
      if (result.ok) sink.next(result.value as T);
      else sink.error(result.error);
    },
    error: (reason) => sink.error(subscriptionError(reason, tokenUnreadable(), origin)),
    complete: () => sink.complete(),
  };
}

/** The single graphql-transport-ws socket of one hub (API-002). */
export function createSocket(transport: SocketTransport): Socket {
  const state: SocketState = { phase: 'opening', closed: false, tokenUnreadable: false, deadline: undefined, retryTimer: undefined };
  const live = new Set<(error?: HubError) => void>();
  const client = connect(transport, state, () => {
    for (const finish of [...live]) finish(REFUSED);
  });
  return {
    subscribe(document, variables, sink) {
      if (state.closed) {
        sink.complete();
        return () => undefined;
      }
      let ended = false;
      let stop = (): void => undefined;
      const end = (deliver: () => void): void => {
        if (ended) return;
        ended = true;
        live.delete(finish);
        stop();
        deliver();
      };
      const finish = (error?: HubError): void => end(() => (error === undefined ? sink.complete() : sink.error(error)));
      const guarded: typeof sink = {
        next: (value) => {
          if (!ended) sink.next(value);
        },
        error: finish,
        complete: () => finish(),
      };
      live.add(finish);
      stop = client.subscribe({ query: print(document), variables }, deliverTo(guarded, () => state.tokenUnreadable, transport.address.origin));
      return () => end(() => undefined);
    },
    close() {
      const { phase } = state;
      state.closed = true;
      clearDeadline(state);
      clearTimeout(state.retryTimer);
      for (const finish of [...live]) finish();
      // dispose waits for a pending attempt, so an opening one is terminated and a parked one is not awaited (CQ-024).
      if (phase === 'opening') client.terminate();
      const disposing = Promise.resolve(client.dispose());
      return phase === 'waiting' ? Promise.resolve() : disposing;
    },
  };
}
