// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { expect, it, vi } from 'vitest';
import Page from './+page.svelte';

const calls = vi.hoisted(() => ({ keepFresh: vi.fn() }));
vi.mock('$lib/platform/platform', () => ({ capabilities: () => ({ agentThreads: false, deviceLink: true }) }));
vi.mock('$lib/agent-threads/watch.svelte', () => ({ threadsStore: () => ({}), keepFresh: calls.keepFresh }));

it('keeps the remote Code surface unavailable when this client has no local runner', async () => {
  const app = mount(Page, { target: document.body });
  flushSync();
  expect(document.body.textContent).toContain('Code currently shows threads started by Anima on this computer.');
  expect(calls.keepFresh).not.toHaveBeenCalled();
  expect(document.querySelector('[data-code-page]')).toBeNull();
  await unmount(app);
  document.body.replaceChildren();
});
