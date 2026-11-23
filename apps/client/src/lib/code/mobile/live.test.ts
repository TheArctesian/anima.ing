// @vitest-environment jsdom
import { expect, test, vi } from 'vitest';
import type { Sink } from '$lib/hub/hub';
import type { MobileCodeApi, ThreadEntry } from './api';
import { followThread } from './live.svelte';

test('coming online continues the open thread at its latest cursor and skips replayed entries', () => {
  let receiver: Sink<ThreadEntry> | undefined;
  const stop = vi.fn();
  const follow: MobileCodeApi['follow'] = vi.fn((_thread, _cursor, sink) => {
    receiver = sink;
    return stop;
  });
  const api = {
    follow,
    inbox: vi.fn<MobileCodeApi['inbox']>(),
    snapshot: vi.fn<MobileCodeApi['snapshot']>(),
  };
  const received: string[] = [];
  const stream = followThread(api, 'thread-a', '64', (entry) => { received.push(entry.sequence); });
  try {
    receiver?.next({ sequence: '65', content: { change: 'prompt', text: 'Check the build.' } });
    window.dispatchEvent(new Event('online'));
    expect(stop).toHaveBeenCalledTimes(1);
    expect(follow).toHaveBeenLastCalledWith('thread-a', '65', expect.anything());

    receiver?.next({ sequence: '65', content: { change: 'prompt', text: 'Check the build.' } });
    receiver?.next({ sequence: '66', content: { change: 'state', state: 'working' } });
    expect(received).toEqual(['65', '66']);
  } finally {
    stream.stop();
  }
});
