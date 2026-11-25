// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { fakeHub } from '$lib/devices/hub.fixture';
import Page from './+page.svelte';

const calls = vi.hoisted(() => ({ deviceLink: true, keepFresh: vi.fn() }));
const { hub } = fakeHub({
  DeviceHosts: { devices: { edges: [], pageInfo: { hasNextPage: false } } },
  MobileThreadInbox: {
    agentThreads: {
      nodes: [{
        id: 'thread-a', title: 'Inspect the fixtures', deviceId: 'device-a',
        program: 'codex', state: 'idle', activityAt: '2026-11-21T00:00:00Z', repository: null,
      }],
      pageInfo: { hasNextPage: false },
    },
  },
});
let app: ReturnType<typeof mount> | undefined;

vi.mock('$app/state', () => ({ page: { url: new URL('https://hub.test/code') } }));
vi.mock('$lib/hub/context', () => ({ useHub: () => hub }));
vi.mock('$lib/platform/platform', () => ({ capabilities: () => ({ agentThreads: false, deviceLink: calls.deviceLink }) }));
vi.mock('$lib/agent-threads/watch.svelte', () => ({ threadsStore: () => ({}), keepFresh: calls.keepFresh }));

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  vi.restoreAllMocks();
});

it.each([
  ['iPad desktop mode', 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', 5],
  ['Android', 'Mozilla/5.0 (Linux; Android 16)', 5],
])('shows the read-only hub inbox in the native %s app', async (_device, userAgent, touchPoints) => {
  calls.deviceLink = true;
  vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(userAgent);
  vi.stubGlobal('navigator', Object.create(navigator, { maxTouchPoints: { value: touchPoints } }));
  try {
    app = mount(Page, { target: document.body });
    await vi.waitFor(() => {
      expect(document.querySelector('a[href="/code?thread=thread-a"]')?.textContent ?? '').toContain('Inspect the fixtures');
    });
    expect(calls.keepFresh).not.toHaveBeenCalled();
  } finally {
    vi.unstubAllGlobals();
  }
});

it('shows the local Anima guidance in a mobile browser', () => {
  calls.deviceLink = false;
  vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue('Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X)');
  app = mount(Page, { target: document.body });
  expect(document.body.textContent).toContain('Code currently shows threads started by Anima on this computer.');
  expect(calls.keepFresh).not.toHaveBeenCalled();
});
