// @vitest-environment jsdom
import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { RunRow } from '$lib/agent-threads/live/run';
import Composer from './Composer.svelte';
import { pointAt } from './editor/view/points';

const ROW: RunRow = { id: 'c1', program: 'claude_code', folder: '/work/anima', mode: 'ask', model: null, state: 'working', session: null, title: 'Fix the gate', started: new Date(0), ended: null, open: [] };
const WENT: CallResult = { ok: true, outcome: { outcome: 'ok' } };

let app: ReturnType<typeof mount> | undefined;

beforeAll(() => {
  /* jsdom has no media queries and no element animations; the picker asks for a coarse pointer and fades its popup. */
  window.matchMedia = (query: string) => ({ matches: false, media: query }) as MediaQueryList;
  Element.prototype.animate ??= () => ({ cancel: () => undefined, finished: Promise.resolve() }) as unknown as Animation;
  Element.prototype.scrollIntoView ??= () => undefined;
});

afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

/** The composer of a chat as the chat column mounts it, over the runner's adapter and the queue of the window. */
function composer(row: RunRow) {
  const sent: string[] = [];
  const calls: string[] = [];
  const props = $state({
    row: row as RunRow | null,
    draft: row.id,
    onSend: (text: string) => {
      sent.push(`${props.row?.id ?? 'none'}: ${text}`);
      return Promise.resolve(WENT);
    },
    onInterrupt: () => {
      calls.push('interrupt');
      return Promise.resolve(WENT);
    },
    onStop: () => Promise.resolve(WENT),
  });
  app = mount(Composer, { target: document.body, props });
  flushSync();
  return { props, sent, calls };
}

function box(): HTMLElement {
  const found = document.querySelector<HTMLElement>('[role="textbox"]');
  if (found === null) throw new Error('no text box');
  return found;
}

function type(text: string): void {
  box().focus();
  const at = pointAt(box(), 0);
  document.getSelection()?.setBaseAndExtent(at.node, at.offset, at.node, at.offset);
  document.dispatchEvent(new Event('selectionchange'));
  for (const char of text) {
    box().dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: char, bubbles: true, cancelable: true }));
    flushSync();
  }
}

function press(key: string, more: KeyboardEventInit = {}): void {
  box().dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...more }));
  flushSync();
}

async function settled(): Promise<void> {
  await tick();
  await new Promise((resolve) => setTimeout(resolve, 0));
  flushSync();
}

const bubbles = (): string[] => Array.from(document.querySelectorAll('ol > li .bubble')).map((bubble) => bubble.textContent.trim());
const button = (name: string): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>(`button[aria-label="${name}"]`);

describe('the composer of a chat that the runner drives', () => {
  it('steers with Enter and queues with the modifier, and sends the queued message when the turn ends', async () => {
    const { props, sent } = composer({ ...ROW, id: 'c-queue' });
    type('look at the log');
    press('Enter');
    await settled();
    expect(sent).toEqual(['c-queue: look at the log']);
    type('then run the gates');
    press('Enter', { metaKey: true });
    await settled();
    expect({ sent, text: box().textContent, bubbles: bubbles() }).toEqual({ sent: ['c-queue: look at the log'], text: '', bubbles: ['then run the gates'] });
    props.row = { ...ROW, id: 'c-queue', state: 'idle' };
    await settled();
    expect(sent).toEqual(['c-queue: look at the log', 'c-queue: then run the gates']);
    expect(bubbles()).toEqual([]);
  });

  it('keeps the queue after a stop by the person, and says that it waits', async () => {
    const { props, sent, calls } = composer({ ...ROW, id: 'c-stop' });
    type('after this');
    press('Enter', { metaKey: true });
    await settled();
    button('Stop generation')?.click();
    await settled();
    expect(calls).toEqual(['interrupt']);
    props.row = { ...ROW, id: 'c-stop', state: 'idle' };
    await settled();
    expect(sent).toEqual([]);
    expect(bubbles()).toEqual(['after this']);
    expect(document.querySelector('.waits')?.textContent).toMatch(/^You stopped the turn/);
  });

});

describe('the queue of the composer across chats and states', () => {
  it('does not send the queue of one chat to the chat that is opened after it', async () => {
    const { props, sent } = composer({ ...ROW, id: 'c-first' });
    type('for the first chat');
    press('Enter', { metaKey: true });
    await settled();
    props.draft = 'c-second';
    props.row = { ...ROW, id: 'c-second', state: 'idle' };
    await settled();
    expect(sent).toEqual([]);
    expect(bubbles()).toEqual([]);
    props.draft = 'c-first';
    props.row = { ...ROW, id: 'c-first', state: 'idle' };
    await settled();
    expect(sent).toEqual(['c-first: for the first chat']);
  });

  it('does not take a run whose state is not known for a turn that ended', async () => {
    const { props, sent } = composer({ ...ROW, id: 'c-unknown' });
    type('when it is done');
    press('Enter', { metaKey: true });
    await settled();
    props.row = { ...ROW, id: 'c-unknown', state: 'unknown' };
    await settled();
    expect(sent).toEqual([]);
    props.row = { ...ROW, id: 'c-unknown', state: 'idle' };
    await settled();
    expect(sent).toEqual(['c-unknown: when it is done']);
  });

  it('is a read-only bar for a chat that no program runs, and for one whose program ended', () => {
    const { props } = composer({ ...ROW, id: 'c-ended', state: 'stopped', ended: 'The program was closed.' });
    expect(document.body.textContent).toContain('The program was closed.');
    expect(document.querySelector('[role="textbox"]')).toBeNull();
    props.row = null;
    flushSync();
    expect(document.body.textContent).toContain('no program of this computer runs it');
  });
});
