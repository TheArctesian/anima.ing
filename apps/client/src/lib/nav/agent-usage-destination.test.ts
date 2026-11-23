// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, it, onTestFinished, vi } from 'vitest';
import Nav from './Nav.svelte';

vi.mock('$lib/platform/platform', async () => {
  const { BROWSER } = await import('$lib/platform/answers.fixture');
  return { capabilities: () => BROWSER };
});
vi.mock('$lib/hub/context', () => ({
  useHub: () => ({
    address: { origin: 'https://hub.example' },
    status: { current: { connection: 'connected', lastContactAt: null } },
    request: () => new Promise(() => undefined),
    subscribe: () => () => undefined,
  }),
}));
vi.mock('motion', async (importOriginal) => ({
  ...(await importOriginal<typeof import('motion')>()),
  animate: (await import('$lib/motion/engine.fixture')).fakeRun,
}));

let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

function show(iconOnly = false): HTMLElement {
  app = mount(Nav, { target: document.body, props: { pathname: '/devices', now: new Date('2026-11-12T12:00:00Z'), iconOnly } });
  flushSync();
  return document.querySelector('nav') ?? expect.fail('no nav');
}

it('AIS-020: a wide window has Agent usage between Devices and Settings', () => {
  expect([...show().querySelectorAll('a')].map((link) => link.getAttribute('href'))).toEqual(['/devices', '/hosting', '/agents/usage', '/extensions', '/settings']);
});

it('UI-015, AIS-020, MOB-010: phone destinations include Code and show their names as tooltips', () => {
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: (query: string) => ({ matches: query.includes('max-width'), addEventListener: () => undefined, removeEventListener: () => undefined }),
  });
  onTestFinished(() => {
    Reflect.deleteProperty(window, 'matchMedia');
    document.documentElement.removeAttribute('data-nav');
  });
  const nav = show(true);
  expect([...nav.querySelectorAll('a')].map((link) => [link.getAttribute('href'), link.textContent?.trim(), link.title])).toEqual([
    ['/devices', 'Devices', 'Devices'],
    ['/code', 'Code', 'Code'],
    ['/logs', 'Logs', 'Logs'],
    ['/settings', 'Settings', 'Settings'],
  ]);
  expect(nav.querySelector('.status')?.getAttribute('title')).toBe('Connected');
  expect(nav.querySelector('.live')).toBeNull();
});
