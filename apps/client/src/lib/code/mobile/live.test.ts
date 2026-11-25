// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { SvelteURL } from 'svelte/reactivity';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { createHub, NO_HUB_ADDRESS, type Hub } from '$lib/hub/hub';
import { acknowledge, FakeSocket, FAKE_WEBSOCKET, settle } from '$lib/hub/socket/fake';
import MobileCode from './MobileCode.svelte';
import type { ThreadSnapshot } from './api';

const url = new SvelteURL('https://hub.test/code?thread=thread-a');
let hub: Hub;
let app: ReturnType<typeof mount> | undefined;

vi.mock('$app/state', () => ({ page: { get url() { return url; } } }));
vi.mock('$lib/hub/context', () => ({ useHub: () => hub }));

beforeEach(() => {
  FakeSocket.opened = [];
  url.search = '?thread=thread-a';
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
});

afterEach(async () => {
  await leave();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function prompt(sequence: string, text: string) {
  return { sequence, content: { change: 'prompt', text } };
}

function snapshot(id: string, sequence: string, text: string): ThreadSnapshot {
  return {
    summary: {
      id, title: id, deviceId: 'device-a', program: 'codex', state: 'idle',
      activityAt: '2026-11-21T00:00:00Z', repository: null,
    },
    entries: [prompt(sequence, text)],
    lastSequence: sequence,
  };
}

async function open(snapshots: readonly ThreadSnapshot[], token = 'account-a'): Promise<FakeSocket> {
  hub = createHub({
    address: NO_HUB_ADDRESS,
    token: () => Promise.resolve(token),
    WebSocket: FAKE_WEBSOCKET,
    fetch: (_url, init) => {
      const { query, variables } = JSON.parse(String(init?.body)) as { query: string; variables: { id?: string } };
      if (query.startsWith('query DeviceHosts')) {
        return Promise.resolve(Response.json({ data: { devices: { edges: [], pageInfo: { hasNextPage: false } } } }));
      }
      if (query.startsWith('query MobileThreadSnapshot')) {
        const thread = snapshots.find(({ summary }) => summary.id === variables.id);
        return Promise.resolve(Response.json({ data: { agentThread: thread ?? null } }));
      }
      throw new Error('Unexpected GraphQL operation');
    },
  });
  app = mount(MobileCode, { target: document.body });
  await settle();
  const socket = FakeSocket.opened.at(-1);
  if (socket === undefined) throw new Error('No socket opened');
  await acknowledge(socket);
  return socket;
}

async function leave(): Promise<void> {
  if (app === undefined) return;
  await unmount(app);
  app = undefined;
  await hub.close();
}

function currentSubscription(socket: FakeSocket, id: string, afterSequence: string): string {
  const subscription = socket.sent.findLast(({ type }) => type === 'subscribe');
  expect(subscription).toMatchObject({ payload: { variables: { id, afterSequence } } });
  if (typeof subscription?.id !== 'string') throw new Error('Missing subscription ID');
  return subscription.id;
}

function deliver(socket: FakeSocket, id: string, entry: ReturnType<typeof prompt>): void {
  socket.receive({ id, type: 'next', payload: { data: { agentThreadEntries: entry } } });
}

function messages(): string[] {
  return [...document.querySelectorAll('ol[aria-label="Recent messages"] li p')].map((line) => line.textContent);
}

test('returning online resumes the open thread at its latest cursor and renders replayed messages once', async () => {
  const socket = await open([snapshot('thread-a', '64', 'Before the drop')]);
  const first = currentSubscription(socket, 'thread-a', '64');
  deliver(socket, first, prompt('65', 'Last seen before reconnect'));

  window.dispatchEvent(new Event('online'));
  await settle();
  const resumed = currentSubscription(socket, 'thread-a', '65');
  expect(socket.sent).toContainEqual({ id: first, type: 'complete' });
  deliver(socket, first, prompt('66', 'From the replaced subscription'));
  deliver(socket, resumed, prompt('65', 'Last seen before reconnect'));
  deliver(socket, resumed, prompt('66', 'Caught up after reconnect'));
  await settle();

  expect(messages()).toEqual(['Before the drop', 'Last seen before reconnect', 'Caught up after reconnect']);
});

test('opening another thread retires the old subscription and resumes only the selected thread', async () => {
  const socket = await open([
    snapshot('thread-a', '64', 'First conversation'),
    snapshot('thread-b', '9', 'Second conversation'),
  ]);
  const first = currentSubscription(socket, 'thread-a', '64');

  flushSync(() => { url.search = '?thread=thread-b'; });
  await settle();
  const second = currentSubscription(socket, 'thread-b', '9');
  expect(socket.sent).toContainEqual({ id: first, type: 'complete' });
  deliver(socket, first, prompt('65', 'A late message from the first conversation'));
  deliver(socket, second, prompt('10', 'A new message from the second conversation'));

  const beforeReturn = socket.sent.length;
  window.dispatchEvent(new Event('online'));
  await settle();
  const returnFrames = socket.sent.slice(beforeReturn);
  expect(returnFrames).toEqual([
    { id: second, type: 'complete' },
    expect.objectContaining({ type: 'subscribe', payload: expect.objectContaining({ variables: { id: 'thread-b', afterSequence: '10' } }) }),
  ]);
  await settle();
  expect(messages()).toEqual(['Second conversation', 'A new message from the second conversation']);
});

test('a fresh client session opens the same URL with the new account snapshot and cursor', async () => {
  const oldSocket = await open([snapshot('thread-a', '64', 'Previous account message')]);
  const previous = currentSubscription(oldSocket, 'thread-a', '64');
  expect(messages()).toEqual(['Previous account message']);
  await leave();
  expect(oldSocket.sent).toContainEqual({ id: previous, type: 'complete' });

  const newSocket = await open([snapshot('thread-a', '3', 'Current account message')], 'account-b');
  expect(newSocket.sent).toContainEqual({ type: 'connection_init', payload: { authorization: 'Bearer account-b' } });
  const current = currentSubscription(newSocket, 'thread-a', '3');
  deliver(oldSocket, previous, prompt('65', 'Late previous account message'));
  deliver(newSocket, current, prompt('4', 'New current account message'));
  await settle();
  expect(messages()).toEqual(['Current account message', 'New current account message']);
});
