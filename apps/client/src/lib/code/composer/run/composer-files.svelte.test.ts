// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { RunRow } from '$lib/agent-threads/live/run';
import Composer from '../Composer.svelte';
import type { AttachAct } from './upload';

const ROW: RunRow = { id: 'f1-chat', program: 'claude_code', folder: '/work/anima', mode: 'ask', model: null, state: 'idle', session: null, title: 'Fix the gate', started: new Date(0), ended: null, open: [] };
const WENT: CallResult = { ok: true, outcome: { outcome: 'ok' } };

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  window.matchMedia = (query: string) => ({ matches: false, media: query }) as MediaQueryList;
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
  URL.createObjectURL ??= () => 'blob:made';
  URL.revokeObjectURL ??= () => undefined;
});

afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

/** The composer as the chat column mounts it, over a runner that takes each part; `hold` keeps an answer back until the test lets it go. */
function composer(chat: string, hold = false) {
  const acts: string[] = [];
  const sent: [string, readonly string[] | undefined][] = [];
  const waiting: (() => void)[] = [];
  const props = $state({
    row: { ...ROW, id: chat } as RunRow | null,
    draft: chat,
    onSend: (text: string, files?: readonly string[]) => {
      sent.push([text, files]);
      return Promise.resolve(WENT);
    },
    onInterrupt: () => Promise.resolve(WENT),
    onStop: () => Promise.resolve(WENT),
    onAttach: async (act: AttachAct, thread: string): Promise<CallResult> => {
      acts.push(`${thread} ${act.call}`);
      if (hold) await new Promise<void>((go) => waiting.push(go));
      return act.call === 'unattach' ? WENT : { ok: true, outcome: { outcome: 'attached', id: 'file-1', received: 3 } };
    },
  });
  app = mount(Composer, { target: document.body, props });
  flushSync();
  return { props, acts, sent, waiting };
}

function choose(files: readonly File[]): void {
  const input = document.querySelector<HTMLInputElement>('input[type="file"]');
  if (input === null) throw new Error('no file input');
  Object.defineProperty(input, 'files', { value: files, configurable: true });
  input.dispatchEvent(new Event('change', { bubbles: true }));
  flushSync();
}

async function settled(): Promise<void> {
  for (let turn = 0; turn < 4; turn += 1) {
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  flushSync();
}

const button = (name: string): HTMLElement | null => document.querySelector<HTMLElement>(`button[aria-label="${name}"]`);

describe('the files of the composer of a chat that the runner drives', () => {
  it('TH111: sends a chosen file to the runner, shows its name as text, and sends the message with its id', async () => {
    const { acts, sent } = composer('chat-a');
    expect(button('Attach files')?.getAttribute('aria-disabled')).toBe('false');
    choose([new File(['abc'], '<img src=x onerror=alert(1)>.txt', { type: 'text/plain' })]);
    await settled();
    expect(acts).toEqual(['chat-a attach']);
    expect(document.querySelector('img[src="x"]')).toBeNull();
    expect(document.querySelector('li .name')?.textContent).toBe('_img src_x onerror_alert_1_.txt');
    button('Submit message')?.click();
    await settled();
    expect(sent).toEqual([['', ['file-1']]]);
    expect(document.querySelector('li .name')).toBeNull();
  });

  it('TH111: tells the runner when a file that it holds is removed before the send', async () => {
    const { acts } = composer('chat-b');
    choose([new File(['abc'], 'notes.txt', { type: 'text/plain' })]);
    await settled();
    button('Remove notes.txt')?.click();
    await settled();
    expect(acts).toEqual(['chat-b attach', 'chat-b unattach']);
  });

  it('names the chat of a file in each call, and gives the file up there when a different chat is opened while it is on its way', async () => {
    const { props, acts, waiting } = composer('chat-c', true);
    choose([new File(['abc'], 'notes.txt', { type: 'text/plain' })]);
    await settled();
    props.row = { ...ROW, id: 'other-chat' };
    props.draft = 'other-chat';
    flushSync();
    for (const go of waiting.splice(0)) go();
    await settled();
    expect(acts).toEqual(['chat-c attach', 'chat-c unattach']);
    expect(document.querySelector('li .name')).toBeNull();
  });

  it('sends a file to the chat that the page names for its calls, also when the row of the run says a different id', async () => {
    const { props, acts, sent } = composer('chat-d');
    props.row = { ...ROW, id: 'row-of-a-different-chat' };
    flushSync();
    choose([new File(['abc'], 'notes.txt', { type: 'text/plain' })]);
    await settled();
    button('Submit message')?.click();
    await settled();
    expect(acts).toEqual(['chat-d attach']);
    expect(sent).toEqual([['', ['file-1']]]);
  });
});
