import type { TypedDocumentNode } from '@graphql-typed-document-node/core';
import { parse } from 'graphql';
import { vi } from 'vitest';
import { createHub, NO_HUB_ADDRESS, type HubError, type Sink, type TokenProvider } from '../hub';

export const STATUS_CHANGED = parse('subscription S($deviceId: ID) { deviceStatusChanged(deviceId: $deviceId) { status } }') as TypedDocumentNode<
  { deviceStatusChanged: { status: string } },
  { deviceId?: string }
>;
export const NOW = new Date('2026-11-04T10:00:00Z');
export const DEFAULT_TIMEOUT_MS = 10_000;
export const FIRST_RETRY_MS = 1000;

export class FakeSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static opened: FakeSocket[] = [];

  readyState = FakeSocket.CONNECTING;
  closedWith: number | undefined;
  readonly sent: Record<string, unknown>[] = [];
  onopen: ((event: unknown) => void) | null = null;
  onclose: ((event: unknown) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;

  constructor(
    readonly url: string,
    readonly protocol: string,
  ) {
    FakeSocket.opened.push(this);
  }

  send(text: string): void {
    this.sent.push(JSON.parse(text) as Record<string, unknown>);
  }

  closeFailure: Error | undefined;

  close(code = 1000, reason = ''): void {
    if (this.closeFailure !== undefined) throw this.closeFailure;
    if (this.readyState === FakeSocket.CLOSED) return;
    this.readyState = FakeSocket.CLOSED;
    this.closedWith = code;
    this.onclose?.({ code, reason, wasClean: code === 1000 });
  }

  accept(): void {
    this.readyState = FakeSocket.OPEN;
    this.onopen?.({});
  }

  receive(message: Record<string, unknown>): void {
    this.onmessage?.({ data: JSON.stringify(message) });
  }

  subscribeId(): string {
    const message = this.sent.find((sent) => sent.type === 'subscribe');
    return typeof message?.id === 'string' ? message.id : '';
  }
}

export function recordingSink() {
  const events: { next: unknown[]; errors: HubError[]; completed: number } = { next: [], errors: [], completed: 0 };
  const sink: Sink<unknown> = {
    next: (value) => events.next.push(value),
    error: (error) => events.errors.push(error),
    complete: () => {
      events.completed += 1;
    },
  };
  return { events, sink };
}

export const FAKE_WEBSOCKET = FakeSocket as unknown as typeof WebSocket;

export function newHub(token: TokenProvider = () => Promise.resolve('s3cret-token')) {
  return createHub({ address: NO_HUB_ADDRESS, token, WebSocket: FAKE_WEBSOCKET, now: () => NOW });
}

export async function settle(ms = 0): Promise<void> {
  await vi.advanceTimersByTimeAsync(ms);
}

export function onlySocket(): FakeSocket {
  const [socket] = FakeSocket.opened;
  if (socket === undefined) throw new Error('no socket was opened');
  return socket;
}

export async function acknowledge(socket: FakeSocket): Promise<void> {
  socket.accept();
  await settle();
  socket.receive({ type: 'connection_ack' });
  await settle();
}

export async function liveSubscription(hub: ReturnType<typeof newHub>, sink: Sink<unknown>) {
  const stop = hub.subscribe(STATUS_CHANGED, { deviceId: 'd1' }, sink);
  await settle();
  const socket = onlySocket();
  await acknowledge(socket);
  return { socket, stop };
}
