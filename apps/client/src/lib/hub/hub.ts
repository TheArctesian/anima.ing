import type { TypedDocumentNode } from '@graphql-typed-document-node/core';
import type { HubAddress } from './address';
import type { HubResult } from './error';
import { sendRequest, type TokenProvider } from './request';
import { createConnection, type LiveStatus } from './status/connection';
import { createSocket, type Sink, type Visibility } from './socket/socket';
import type { CopyPlace } from './recent/copy';
import { createRecentCopy } from './recent/recent';

export type { HubAddress } from './address';
export { NO_HUB_ADDRESS, isOnThisComputer, parseHubAddress } from './address';
export type { FieldError, HubCode, HubError, HubResult } from './error';
export { TOKEN_UNREADABLE, UNREACHABLE, needsNewerHub, noAnswer } from './error';
export { GATEWAY_ENTER_PATH, gatewayInFront, gatewaySignOut, mayEnterGatewayNow } from './gateway/gateway';
export type { TokenProvider } from './request';
export type { LiveStatus } from './status/connection';
export type { Connection, HubStatus } from './status/status';
export { dataAge } from './status/age';
export type { Sink } from './socket/socket';
export { desktopRecentCopy, forgetRecentCopy } from './recent/desktop';
export type { GatewayAnswer } from './gateway';
export { readGateway } from './gateway';

export type Variables = Record<string, unknown>;

const HUB_TIMEOUT_MS = 10_000;

/**
 * The client's one GraphQL transport (CQ-045). It never rejects or throws for a hub, network or token failure: failures
 * are `HubResult` values (NET-004), and a token provider that rejects gives `UNAUTHENTICATED`.
 */
export interface Hub {
  readonly address: HubAddress;
  /** Run a query or mutation over HTTP; no answer within the timeout is `unreachable`. */
  request<TResult, TVariables extends Variables>(
    document: TypedDocumentNode<TResult, TVariables>,
    variables: TVariables,
  ): Promise<HubResult<TResult>>;
  /**
   * Returns the function that stops it (API-002). A refused (close 4403) or unreadable token gives the sink `error`
   * (UNAUTHENTICATED), and the subscription is not sent again.
   */
  subscribe<TResult, TVariables extends Variables>(
    document: TypedDocumentNode<TResult, TVariables>,
    variables: TVariables,
    sink: Sink<TResult>,
  ): () => void;
  /** `connected` on `connection_ack` or a refused token, `unreachable` on failure or no ack in time (NAV-003, NET-004). */
  readonly status: LiveStatus;
  /**
   * Ends every subscription and the socket; a later `subscribe` completes at once, and `request` keeps working. Rejects
   * only when an open socket could not be closed.
   */
  close(): Promise<void>;
}

export interface HubOptions {
  readonly address: HubAddress;
  readonly token: TokenProvider;
  /** The socket reconnects by itself, so a reader that checks the address each time (F-10) goes here; `token` when absent. */
  readonly socketToken?: TokenProvider;
  readonly fetch?: typeof fetch;
  readonly WebSocket?: typeof WebSocket;
  readonly now?: () => Date;
  readonly timeoutMs?: number;
  /** The page whose return to the screen retries the socket at once (MOB-036); `document` when absent. */
  readonly visibility?: Visibility;
  /** When given, a copied query that the hub does not answer gives its last answer (FND-007). */
  readonly recent?: CopyPlace;
}

/** A hub at `address`; its socket connects at once, so the status is true before any request (NAV-003, NET-004). */
export function createHub(options: HubOptions): Hub {
  // A frozen copy: a caller that changes its own object later changes neither the endpoints nor what the token provider hears (CLI-002).
  const address: HubAddress = Object.freeze({ ...options.address });
  const { token } = options;
  const timeoutMs = options.timeoutMs ?? HUB_TIMEOUT_MS;
  const now = options.now ?? (() => new Date());
  const connection = createConnection(now);
  const http = {
    address,
    fetch: options.fetch ?? globalThis.fetch.bind(globalThis),
    token,
    signal: connection.signal,
    timeoutMs,
  };
  const socket = createSocket({
    address,
    WebSocket: options.WebSocket ?? globalThis.WebSocket,
    token: options.socketToken ?? token,
    signal: connection.signal,
    timeoutMs,
    visibility: options.visibility ?? (typeof document === 'undefined' ? undefined : document),
  });
  const recent = options.recent === undefined ? undefined : createRecentCopy({ ...options.recent, address, now, served: connection.copyShown });
  return {
    address,
    request: (document, variables) => {
      const send = () => sendRequest(http, document, variables);
      return recent === undefined ? send() : recent.answer(document, variables, send);
    },
    subscribe: (document, variables, sink) => socket.subscribe(document, variables, sink),
    status: connection.status,
    close: () => socket.close(),
  };
}

export type RequestHub = Pick<Hub, 'address' | 'request'>;

/** For a token that opens no socket, such as the builder token (BLD-022, D-1); its failures leave the connection status alone. */
export function createRequestHub(options: Omit<HubOptions, 'WebSocket' | 'now' | 'socketToken'>): RequestHub {
  const address: HubAddress = Object.freeze({ ...options.address });
  const http = {
    address,
    fetch: options.fetch ?? globalThis.fetch.bind(globalThis),
    token: options.token,
    signal: () => undefined,
    timeoutMs: options.timeoutMs ?? HUB_TIMEOUT_MS,
  };
  return { address, request: (document, variables) => sendRequest(http, document, variables) };
}
