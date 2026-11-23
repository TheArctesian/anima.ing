// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, expect, it } from 'vitest';
import { ThreadsStore } from '$lib/agent-threads/store.svelte';
import { shownTitle, threadFrom, type Thread } from '$lib/agent-threads/thread';
import ThreadBody from '../ThreadBody.svelte';
// A Codex thread of this Mac in code mode (`anima agents thread <id> --json`, 2026-11-22), its scratch folder renamed:
// each tool is one `exec` call of JavaScript, and a `wait` polls the script that was still running.
import CAPTURE from './codex-code-mode-thread.json';

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  window.matchMedia = (query: string) => ({ matches: false, media: query, addEventListener: () => undefined, removeEventListener: () => undefined }) as unknown as MediaQueryList;
  Element.prototype.scrollTo ??= () => undefined;
  globalThis.ResizeObserver ??= class {
    observe = (): undefined => undefined;
    unobserve = (): undefined => undefined;
    disconnect = (): undefined => undefined;
  };
});

afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

function captured(): Thread {
  const thread = threadFrom(CAPTURE);
  if (thread === null) throw new Error('the capture did not read');
  return thread;
}

async function opened(thread: Thread): Promise<void> {
  const store = new ThreadsStore({ list: () => Promise.resolve([thread]), read: () => Promise.resolve(thread) });
  await store.open(thread.id);
  app = mount(ThreadBody, { target: document.body, props: { store, now: new Date('2026-11-22T12:00:00Z') } });
  flushSync();
  await tick();
}

it('shows a stored Codex thread as a chat timeline, its exec calls as the commands they ran', async () => {
  await opened(captured());
  expect(document.querySelector('table')).toBeNull();
  document.querySelector<HTMLButtonElement>('.fold-line')?.click();
  flushSync();
  const group = document.querySelector<HTMLButtonElement>('.work .line');
  expect(group?.getAttribute('aria-label')).toBe('Ran 2 commands, tool call failed');
  group?.click();
  flushSync();
  document.querySelectorAll<HTMLButtonElement>('.work .group .line[aria-expanded="false"]').forEach((row) => {
    row.click();
  });
  flushSync();
  const text = document.body.textContent ?? '';
  expect(text).toContain('pwd -P and 1 more');
  expect(text).toContain('fatal: not a git repository');
  expect(text).toContain('printf hi > codex.txt');
  expect(text).not.toContain('tools.exec_command');
  expect(text).not.toContain('cell_id');
  expect(text).not.toContain('chunk_id');
});

it('names a Codex thread with no title by its program, folder and start, never "Untitled thread"', () => {
  const thread = captured();
  expect(thread.title).toBeNull();
  expect(shownTitle(thread)).toMatch(/^Codex in e2e-1430, /);
  expect(shownTitle({ ...thread, title: 'Fix the login page' })).toBe('Fix the login page');
});
