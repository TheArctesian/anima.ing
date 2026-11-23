// Test files mock `$lib/sessions/api` and `$lib/sessions/sessions.svelte` so the layout uses `fake.api`, `fake.port` and `fake.sessions`.
import { createRawSnippet, flushSync, mount, unmount, type Component } from 'svelte';
import { vi } from 'vitest';
import type { SessionsApi } from '../api';
import type { PopOutPort, SessionPort, Sessions } from '../sessions.svelte';
import { FakeTerminalSocket } from '../tunnel/terminal/fake/socket.fixture';
import { FakeResizeObserver, fakeApi, type ApiCalls, type HubAnswers } from '../view/view.fixture.svelte';

interface SpiedPort extends SessionPort {
  readonly start: ReturnType<typeof vi.fn<SessionPort['start']>>;
  readonly end: ReturnType<typeof vi.fn<SessionPort['end']>>;
}

export const fake = {
  api: undefined as unknown as SessionsApi,
  calls: undefined as unknown as ApiCalls,
  answers: undefined as unknown as HubAnswers,
  port: undefined as unknown as SpiedPort,
  popOut: undefined as PopOutPort | undefined,
  sessions: undefined as unknown as Sessions,
};

export function spiedSessions(create: (port: SessionPort, popOut?: PopOutPort) => Sessions, port: SessionPort): Sessions {
  fake.port = { start: vi.fn((deviceId: string) => port.start(deviceId)), end: vi.fn((id: string) => port.end(id)) };
  fake.sessions = create(fake.port, fake.popOut);
  return fake.sessions;
}

export const keyframes: Keyframe[][] = [];

let app: ReturnType<typeof mount> | undefined;

/** jsdom has no Web Animations; this one finishes at once so outros complete. */
function finishingAnimate(frames: Keyframe[]) {
  keyframes.push(frames);
  return {
    cancel: () => undefined,
    currentTime: 0,
    playState: 'finished',
    effect: null,
    set onfinish(done: () => void) {
      queueMicrotask(done);
    },
  };
}

const stubs: Record<string, unknown> = {
  animate: finishingAnimate,
  setPointerCapture: () => undefined,
  releasePointerCapture: () => undefined,
  hasPointerCapture: () => true,
};

let restore: (() => void)[] = [];

export function prepare(): void {
  ({ api: fake.api, calls: fake.calls, answers: fake.answers } = fakeApi());
  keyframes.length = 0;
  for (const [name, value] of Object.entries(stubs)) {
    Object.defineProperty(Element.prototype, name, { value, configurable: true });
  }
  FakeTerminalSocket.reset();
  FakeResizeObserver.observers = [];
  const saved = { WebSocket: globalThis.WebSocket, ResizeObserver: globalThis.ResizeObserver };
  globalThis.WebSocket = FakeTerminalSocket as unknown as typeof WebSocket;
  globalThis.ResizeObserver = FakeResizeObserver as unknown as typeof ResizeObserver;
  restore = [
    () => {
      Object.assign(globalThis, saved);
    },
  ];
}

export async function cleanup(): Promise<void> {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.innerHTML = '';
  for (const name of Object.keys(stubs)) Reflect.deleteProperty(Element.prototype, name);
  for (const undo of restore.reverse()) undo();
  restore = [];
}

export async function settle(): Promise<void> {
  for (let tick = 0; tick < 10; tick += 1) {
    await Promise.resolve();
    flushSync();
  }
}

/** Comes after mount: the layout's motion preference reads `matchMedia` once at mount, and the strip tests rely on its absence. */
function giveMatchMedia(): void {
  const saved = window.matchMedia;
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
  })) as unknown as typeof window.matchMedia;
  restore.push(() => {
    window.matchMedia = saved;
  });
}

export function show(layout: Component<{ children: ReturnType<typeof createRawSnippet> }>, popOut?: PopOutPort): Sessions {
  fake.popOut = popOut;
  const children = createRawSnippet(() => ({ render: () => '<p id="page">page</p>' }));
  app = mount(layout, { target: document.body, props: { children } });
  flushSync();
  giveMatchMedia();
  return fake.sessions;
}

export const request = { deviceId: 'web-1', deviceName: 'web-1', protocol: 'SSH', origin: null } as const;

export const tabs = () => [...document.querySelectorAll<HTMLElement>('[role="tab"]')];

export const visiblePanels = () =>
  [...document.querySelectorAll('[role="tabpanel"]')].filter((panel) => !panel.closest('[hidden]'));

/** A pane's first measure starts the session, as in a browser. */
export async function openStarted(sessions: Sessions, origin: DOMRect | null = null): Promise<string> {
  const id = sessions.openSession({ ...request, origin });
  flushSync();
  FakeResizeObserver.resize(800, 600);
  await settle();
  return id;
}
