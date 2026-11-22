// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { LiveThreads } from '$lib/agent-threads/live/live.svelte';
import type { Action, Outcome } from '$lib/agent-threads/live/run';
import { madeShell, stateFrame, THREAD } from '$lib/agent-threads/live/tests/frames.fixture';
import { pointAt } from '../composer/editor/view/points';
import { FILE_SEARCH_DELAY_MS } from '../composer/mentions/menu';
import ChatFoot from './ChatFoot.svelte';

// The `@` list of the Code page asks the runner for the files of the chat's folder (TH108), through the calls the
// runner reads: the fields of `Action::Files` (crates/agent-threads/src/control.rs), and no `id` beside the frame's.

const FOUND: Outcome = { outcome: 'files', paths: [{ path: 'src/main.rs', folder: false }], more: false };

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  window.matchMedia = (query: string) => ({ matches: false, media: query }) as MediaQueryList;
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
  Element.prototype.scrollIntoView ??= () => undefined;
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

function type(text: string): void {
  const box = document.querySelector<HTMLElement>('[role="textbox"]');
  if (box === null) throw new Error('no text box');
  box.focus();
  const at = pointAt(box, 0);
  document.getSelection()?.setBaseAndExtent(at.node, at.offset, at.node, at.offset);
  document.dispatchEvent(new Event('selectionchange'));
  for (const char of text) {
    box.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: char, bubbles: true, cancelable: true }));
    flushSync();
  }
}

describe('the @ list of the Code page', () => {
  it('TH108: asks the runner for the files of the open chat and lists what it sent', async () => {
    const shell = madeShell((action: Action) => (action.call === 'files' ? FOUND : { outcome: 'ok' }));
    const live = new LiveThreads(shell, () => ({ front: true, chosen: THREAD }));
    await live.begin();
    shell.emit(stateFrame(1, 'idle'));
    const thread = live.threads.get(THREAD) ?? null;
    expect(thread).not.toBeNull();
    app = mount(ChatFoot, { target: document.body, props: { thread, live, id: THREAD } });
    flushSync();

    type('@main');
    await new Promise((resolve) => setTimeout(resolve, FILE_SEARCH_DELAY_MS + 20));
    await settled();

    const asked = shell.calls.filter((call) => call.call === 'files');
    expect(asked.length).toBeGreaterThan(0);
    for (const call of asked) {
      expect(Object.keys(call).sort()).toEqual(['call', 'most', 'query', 'thread']);
      expect(call).toMatchObject({ call: 'files', thread: THREAD });
      // The runner refuses more than 200 paths.
      expect(call.call === 'files' && call.most <= 200).toBe(true);
    }
    expect(asked.at(-1)).toMatchObject({ query: 'main' });
    expect(document.body.textContent).toContain('main.rs');
    expect(document.body.textContent).not.toContain('The runner cannot search the folder of a chat yet.');
  });
});
