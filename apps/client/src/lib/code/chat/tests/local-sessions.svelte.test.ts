// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installAnimations } from '$lib/forms/animations.fixture';
import { ThreadsStore } from '$lib/agent-threads/store.svelte';
import { LiveThreads } from '$lib/agent-threads/live/live.svelte';
import { outcomeFrom } from '$lib/agent-threads/live/run';
import type { Thread } from '$lib/agent-threads/thread';
import CodePage from '../CodePage.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({ resolve: (path: string) => path }));
const NOW = new Date('2026-11-23T12:00:00Z');
let app: ReturnType<typeof mount> | undefined;
beforeEach(() => {
  installAnimations();
  Element.prototype.getAnimations = () => [];
  localStorage.clear();
  vi.stubGlobal('matchMedia', (media: string) => ({ media, matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }));
  vi.stubGlobal('ResizeObserver', class { observe = vi.fn(); disconnect = vi.fn(); unobserve = vi.fn(); });
  Element.prototype.scrollIntoView = vi.fn();
  Element.prototype.scrollTo = vi.fn();
  CSS.escape ??= (value) => value;
});
afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
  vi.unstubAllGlobals();
});
async function settle(): Promise<void> { flushSync(); await tick(); await tick(); flushSync(); }
function thread(id: string): Thread {
  return { id, program: 'codex', title: `${id} title`, folder: `/projects/${id}`, branch: null, model: null, started: NOW, last: NOW, tail: 'turn_ended', prompts: 1, dropped: 0, entries: [{ kind: 'reply', at: NOW, text: `${id} private conversation` }] };
}
async function open(chosen: string | null = null) {
  const rows = [thread('local'), thread('imported')];
  const read = vi.fn(async (id: string) => rows.find((row) => row.id === id) ?? expect.fail('Unknown fixture thread'));
  const live = new LiveThreads({ call: async () => {
    const result = outcomeFrom({ outcome: 'listings', listings: [{ thread: 'local', started_by_anima: true }] });
    if (result === null) throw new Error('Invalid fixture');
    return result;
  }, watch: async () => () => undefined, notify: async () => undefined }, () => ({ front: true, chosen: null }));
  await live.readListings();
  const store = new ThreadsStore({ list: async () => rows, read }, localStorage, () => NOW, live);
  await store.loadList();
  if (chosen !== null) await store.open(chosen);
  app = mount(CodePage, { target: document.body, props: { store } });
  await settle();
  return { store, read };
}
it('does not reveal a previously selected imported session when Code reopens', async () => {
  const { store } = await open('imported');
  expect(store.rows?.map((row) => row.id)).toEqual(['local', 'imported']);
  expect(document.body.textContent).not.toContain('imported title');
  expect(document.body.textContent).not.toContain('imported private conversation');
  expect(document.querySelector('[data-thread="local"]')).not.toBeNull();
});
it('keeps persisted Anima sessions visible after reopening without an active runner thread', async () => {
  const { store } = await open('local');
  expect(store.live?.threads.size).toBe(0);
  expect(document.body.textContent).toContain('local private conversation');
  expect(document.body.textContent).not.toContain('imported title');
  expect(store.needsYou).toBe(0);
  if (app) await unmount(app);
  app = undefined;
  await open('local');
  expect(document.body.textContent).toContain('local private conversation');
});
it('offers no imported-history entry and excludes imported projects and search hits', async () => {
  const { store } = await open();
  expect(store.needsYou).toBe(1);
  document.querySelector<HTMLButtonElement>('[aria-label="Filter threads by project"]')?.click();
  await settle();
  expect(document.body.textContent).not.toContain('Browse imported history');
  expect(document.body.textContent).not.toContain('imported');
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  const search = document.querySelector<HTMLInputElement>('[aria-label="Search threads"]') ?? expect.fail('No search');
  search.value = 'imported'; search.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
  expect(document.querySelector('[data-thread="imported"]')).toBeNull();
  expect(document.body.textContent).not.toContain('imported title');
});

it('keeps imported sessions out of palette search and recent chats', async () => {
  await open();
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', altKey: true, shiftKey: true, bubbles: true, cancelable: true }));
  await settle();
  const palette = document.querySelector('[aria-label="Command palette"]') ?? expect.fail('No palette');
  expect(palette.textContent).toContain('local title');
  expect(palette.textContent).not.toContain('imported title');
  const input = palette.querySelector('input') ?? expect.fail('No palette input');
  input.value = 'imported'; input.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
  expect(palette.textContent).not.toContain('imported title');
});
