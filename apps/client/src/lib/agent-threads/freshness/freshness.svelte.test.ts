// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { watchThreadFiles } from '$lib/desktop/agent-threads/shell';
import { REFRESH_MS } from '../freshness';
import { ThreadsStore } from '../store.svelte';
import { keepFresh } from '../watch.svelte';

vi.mock('$lib/desktop/agent-threads/shell', () => ({ desktopThreadsShell: {}, watchThreadFiles: vi.fn() }));
vi.mock('$lib/desktop/agent-threads/runner', () => ({ desktopRunnerShell: null }));

const watch = vi.mocked(watchThreadFiles);
let changed: Parameters<typeof watchThreadFiles>[0];
let stops: (() => void)[];
let unlisten = vi.fn<() => void>();

beforeEach(() => {
  vi.useFakeTimers();
  stops = [];
  unlisten = vi.fn<() => void>();
  watch.mockReset().mockImplementation((callback) => { changed = callback; return Promise.resolve(unlisten); });
});
afterEach(() => { stops.forEach((stop) => stop()); vi.useRealTimers(); });

function fixture() {
  const list = vi.fn().mockResolvedValue([]);
  const store = new ThreadsStore({ list, read: () => Promise.reject(new Error('no chosen thread')) }, null);
  const target = Object.assign(new EventTarget(), { visibilityState: 'visible' as DocumentVisibilityState });
  return { list, store, target };
}

it('shares one subscription between the page and count, and releases it after the last consumer', async () => {
  const { list, store, target } = fixture();
  const page = keepFresh(store, target);
  const count = keepFresh(store, target);
  stops.push(page, count);
  await vi.advanceTimersByTimeAsync(0);
  expect(list).toHaveBeenCalledTimes(1);
  expect(watch).toHaveBeenCalledTimes(1);
  page();
  changed('changed');
  await vi.advanceTimersByTimeAsync(0);
  expect(list).toHaveBeenCalledTimes(2);
  count();
  changed('changed');
  await vi.advanceTimersByTimeAsync(REFRESH_MS);
  expect(list).toHaveBeenCalledTimes(2);
  expect(unlisten).toHaveBeenCalledTimes(1);
});

it('keeps the silence fallback after ready and after delivered events, and reads nothing while hidden', async () => {
  const { list, store, target } = fixture();
  stops.push(keepFresh(store, target));
  changed('ready');
  await vi.advanceTimersByTimeAsync(REFRESH_MS);
  expect(list).toHaveBeenCalledTimes(2);
  changed('changed');
  await vi.advanceTimersByTimeAsync(REFRESH_MS);
  expect(list).toHaveBeenCalledTimes(4);
  target.visibilityState = 'hidden';
  changed('changed');
  await vi.advanceTimersByTimeAsync(REFRESH_MS * 2);
  expect(list).toHaveBeenCalledTimes(4);
  target.visibilityState = 'visible';
  target.dispatchEvent(new Event('visibilitychange'));
  await vi.advanceTimersByTimeAsync(0);
  expect(list).toHaveBeenCalledTimes(5);
});

it('coalesces a burst during a slow list read into one following read', async () => {
  const { list, store, target } = fixture();
  const first = Promise.withResolvers<[]>();
  list.mockReturnValueOnce(first.promise);
  stops.push(keepFresh(store, target));
  for (let count = 0; count < 100; count++) changed('changed');
  expect(list).toHaveBeenCalledTimes(1);
  first.resolve([]);
  await vi.advanceTimersByTimeAsync(0);
  expect(list).toHaveBeenCalledTimes(2);
});

it('cleans up a listener that resolves after its consumer left and retries an unavailable watch', async () => {
  const { store, target } = fixture();
  const subscribing = Promise.withResolvers<() => void>();
  watch.mockReturnValueOnce(subscribing.promise);
  const stop = keepFresh(store, target);
  stop();
  subscribing.resolve(unlisten);
  await vi.advanceTimersByTimeAsync(0);
  expect(unlisten).toHaveBeenCalledTimes(1);
  stops.push(keepFresh(store, target));
  changed('unavailable');
  await vi.advanceTimersByTimeAsync(REFRESH_MS);
  expect(watch).toHaveBeenCalledTimes(3);
});
