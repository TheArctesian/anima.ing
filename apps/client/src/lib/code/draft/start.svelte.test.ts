// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, expect, it } from 'vitest';
import { LiveThreads } from '$lib/agent-threads/live/live.svelte';
import { runRowFrom, type Action, type Outcome } from '$lib/agent-threads/live/run';
import { madeShell, rowFrame, stateFrame, THREAD } from '$lib/agent-threads/live/tests/frames.fixture';
import { ThreadsStore } from '$lib/agent-threads/store.svelte';
import ThreadBody from '../chat/ThreadBody.svelte';
import { MODELS_WAITING } from './choice';
import { ThreadDraft } from './thread-draft.svelte';

// The order of a start on this Mac (2026-11-23, runner log and Anima.log): the runner answers `start` with `Started`,
// the page opens the thread, and the watch's first update of it comes later. The thread has no session file under the
// runner's ID, so the shell's read of one is `not_found`; the page must not read it nor stop on its error.

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  window.matchMedia = (query: string) => ({ matches: false, media: query, addEventListener: () => undefined, removeEventListener: () => undefined }) as unknown as MediaQueryList;
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
  Element.prototype.scrollTo ??= () => undefined;
  globalThis.ResizeObserver ??= class {
    observe = (): undefined => undefined;
    unobserve = (): undefined => undefined;
    disconnect = (): undefined => undefined;
  };
});

afterEach(async () => {
  if (app) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

async function settled(): Promise<void> {
  for (let round = 0; round < 3; round += 1) {
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 0));
    flushSync();
  }
}

/** A runner that starts THREAD; its `list` has the new row once `listed` says so. */
async function started(listed: () => boolean): Promise<{ shell: ReturnType<typeof madeShell>; store: ThreadsStore; reads: string[] }> {
  const row = runRowFrom(rowFrame('starting'));
  const shell = madeShell((action: Action): Outcome => {
    if (action.call === 'start') return { outcome: 'started', thread: THREAD };
    if (action.call === 'folder') return { outcome: 'folder', notice: [] };
    if (action.call === 'list') return { outcome: 'threads', threads: listed() && row !== null ? [row] : [] };
    return { outcome: 'ok' };
  });
  const live = new LiveThreads(shell, () => ({ front: true, chosen: null }));
  await live.begin();
  const reads: string[] = [];
  const files = { list: () => Promise.resolve([]), read: (id: string) => (reads.push(id), Promise.reject(new Error('not_found'))) };
  const store = new ThreadsStore(files, null, () => new Date('2026-11-23T03:06:43Z'), live);
  app = mount(ThreadBody, { target: document.body, props: { store, now: new Date('2026-11-23T03:06:43Z'), toolsOpen: false } });
  const draft = new ThreadDraft('/w/anima.ing');
  const engine = draft.engine((action) => live.call(action), (id) => void store.open(id, true), MODELS_WAITING);
  expect(await engine.send({ prompt: 'Fix the test', attachments: [] } as unknown as Parameters<typeof engine.send>[0])).toEqual({ ok: true });
  await settled();
  return { shell, store, reads };
}

it("opens the started thread from the runner's rows, before the watch's first update, and reads no session file", async () => {
  const { store, reads } = await started(() => true);
  expect(store.chosen).toBe(THREAD);
  expect(reads).toEqual([]);
  expect(store.threadProblem).toBeNull();
  expect(document.querySelector('[aria-label="Conversation"]')).not.toBeNull();
});

it('shows the thread as loading until the watch brings it, when the runner lists it late', async () => {
  const { shell, store, reads } = await started(() => false);
  expect(reads).toEqual([]);
  expect(document.querySelector('[role="status"][aria-label="Loading the thread"]')).not.toBeNull();
  shell.emit(stateFrame(1, 'working'));
  await settled();
  expect(store.liveThread?.row.id).toBe(THREAD);
  expect(document.querySelector('[aria-label="Conversation"]')).not.toBeNull();
});
