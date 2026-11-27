import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createHub, NO_HUB_ADDRESS } from '../hub';
import { acknowledge, FAKE_WEBSOCKET, FakeSocket, FIRST_RETRY_MS, NOW, settle } from './fake';
import type { Visibility } from './socket';

beforeEach(() => {
  FakeSocket.opened = [];
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
});

afterEach(() => {
  vi.useRealTimers();
});

/** A page that goes to the background and comes back, as the app on a phone does. */
function phonePage() {
  const listeners = new Set<() => void>();
  const page = {
    visibilityState: 'visible' as DocumentVisibilityState,
    addEventListener: (_type: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_type: string, listener: () => void) => listeners.delete(listener),
  };
  const turn = (state: DocumentVisibilityState): void => {
    page.visibilityState = state;
    for (const listener of [...listeners]) listener();
  };
  return { page: page as unknown as Visibility, turn };
}

function newest(): FakeSocket {
  const socket = FakeSocket.opened.at(-1);
  if (socket === undefined) throw new Error('no socket was opened');
  return socket;
}

/** Each attempt fails before it opens, so the wait doubles: 1, 2, 4 and 8 s, and then the socket waits 16 s. */
async function failUntilTheWaitIsLong(): Promise<void> {
  for (const waitMs of [1000, 2000, 4000, 8000]) {
    newest().close(1006);
    await settle(waitMs);
  }
  newest().close(1006);
  await settle();
}

describe('MOB-036: the socket after a time in the background', () => {
  it('a socket that waits to retry retries at once when the page shows again, and then waits from the first wait', async () => {
    const { page, turn } = phonePage();
    const hub = createHub({ address: NO_HUB_ADDRESS, token: () => Promise.resolve('s3cret-token'), WebSocket: FAKE_WEBSOCKET, now: () => NOW, visibility: page });
    await settle();
    turn('hidden');
    await failUntilTheWaitIsLong();
    const waiting = FakeSocket.opened.length;
    turn('visible');
    await settle();
    expect(FakeSocket.opened).toHaveLength(waiting + 1);
    newest().close(1006);
    await settle(FIRST_RETRY_MS - 1);
    expect(FakeSocket.opened).toHaveLength(waiting + 1);
    await settle(1);
    expect(FakeSocket.opened).toHaveLength(waiting + 2);
    await acknowledge(newest());
    expect(hub.status.current.connection).toBe('connected');
    void hub.close();
  });

  it('a page that goes to the background, and a hub that is closed, do not retry', async () => {
    const { page, turn } = phonePage();
    const hub = createHub({ address: NO_HUB_ADDRESS, token: () => Promise.resolve('s3cret-token'), WebSocket: FAKE_WEBSOCKET, now: () => NOW, visibility: page });
    await settle();
    await failUntilTheWaitIsLong();
    const waiting = FakeSocket.opened.length;
    turn('hidden');
    await settle();
    expect(FakeSocket.opened).toHaveLength(waiting);
    await hub.close();
    turn('visible');
    await settle(60_000);
    expect(FakeSocket.opened).toHaveLength(waiting);
  });
});
