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

// The files of a prompt (TH111) through the calls the runner reads: `Action::Attach`, `Unattach` and `Send` of
// crates/agent-threads/src/control.rs, each with only its own fields.

const FILE_ID = '0190a000-0000-7000-8000-0000000000f1';
const OTHER = '0190a000-0000-7000-8000-000000000002';

/** ChatFoot over a runner that takes each file; `hold` keeps the answer to `attach` back until the test lets it go. */
async function footWithFiles(hold: boolean) {
  const waiting: (() => void)[] = [];
  const base = madeShell((action: Action) => (action.call === 'attach' ? { outcome: 'attached', id: FILE_ID, received: 3 } : { outcome: 'ok' }));
  const shell = {
    ...base,
    call: async (action: Action) => {
      if (hold && action.call === 'attach') await new Promise<void>((go) => waiting.push(go));
      return base.call(action);
    },
  };
  const live = new LiveThreads(shell, () => ({ front: true, chosen: THREAD }));
  await live.begin();
  base.emit(stateFrame(1, 'idle'));
  const props = $state({ thread: live.threads.get(THREAD) ?? null, live, id: THREAD });
  app = mount(ChatFoot, { target: document.body, props });
  flushSync();
  return { calls: base.calls, props, waiting };
}

function choose(file: File): void {
  const input = document.querySelector<HTMLInputElement>('input[type="file"]');
  if (input === null) throw new Error('no file input');
  Object.defineProperty(input, 'files', { value: [file], configurable: true });
  input.dispatchEvent(new Event('change', { bubbles: true }));
  flushSync();
}

const press = (name: string): void => document.querySelector<HTMLElement>(`button[aria-label="${name}"]`)?.click();
const keysOf = (calls: readonly Action[], call: string): string[][] => calls.filter((each) => each.call === call).map((each) => Object.keys(each).sort());

describe('the files of a prompt on the Code page', () => {
  beforeAll(() => {
    URL.createObjectURL ??= () => 'blob:made';
    URL.revokeObjectURL ??= () => undefined;
  });

  it('TH111: attaches, removes and sends with exactly the fields the runner reads', async () => {
    const { calls } = await footWithFiles(false);
    choose(new File(['abc'], 'notes.txt', { type: 'text/plain' }));
    await settled();
    press('Remove notes.txt');
    await settled();
    choose(new File(['abc'], 'plan.txt', { type: 'text/plain' }));
    await settled();
    press('Submit message');
    await settled();
    expect(keysOf(calls, 'attach')).toEqual([
      ['bytes', 'call', 'mime', 'name', 'size', 'thread'],
      ['bytes', 'call', 'mime', 'name', 'size', 'thread'],
    ]);
    expect(keysOf(calls, 'unattach')).toEqual([['attachment', 'call', 'thread']]);
    expect(keysOf(calls, 'send')).toEqual([['attachments', 'call', 'text', 'thread']]);
    expect(calls.find((each) => each.call === 'send')).toMatchObject({ thread: THREAD, attachments: [FILE_ID] });
    expect(calls.filter((each) => each.call === 'attach' || each.call === 'unattach').every((each) => 'thread' in each && each.thread === THREAD)).toBe(true);
  });

  it('gives a file up in the chat it was chosen in when a different chat opens while it is on its way', async () => {
    const { calls, props, waiting } = await footWithFiles(true);
    choose(new File(['abc'], 'notes.txt', { type: 'text/plain' }));
    await settled();
    props.id = OTHER;
    flushSync();
    for (const go of waiting.splice(0)) go();
    await settled();
    expect(calls.find((each) => each.call === 'unattach')).toEqual({ call: 'unattach', thread: THREAD, attachment: FILE_ID });
  });
});
