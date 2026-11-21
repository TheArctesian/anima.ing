import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CardState } from './card-state.svelte';
import { CAN, cannot } from './draft';
import type { ComposerEngine, Message, Outcome } from './engine';
import { FIXTURE_SOURCES } from './mentions/fixture';
import { pickerCatalogue, pickerChoice } from './picker/fixture';

/** An engine that records each call and answers as told. */
function engineOf(over: Partial<ComposerEngine> = {}): { engine: ComposerEngine; calls: string[]; answer: { next: Outcome } } {
  const calls: string[] = [];
  const answer = { next: { ok: true } as Outcome };
  const call = (name: string) => (first?: Message | string, second?: Message) => {
    const said = typeof first === 'string' ? `${first} ${second?.prompt ?? ''}`.trim() : (first?.prompt ?? '');
    calls.push(`${name} ${said}`.trim());
    return Promise.resolve(answer.next);
  };
  const engine: ComposerEngine = {
    phase: 'idle',
    ended: null,
    send: call('send'),
    interrupt: call('interrupt'),
    attach: CAN,
    dictate: CAN,
    steer: CAN,
    queue: CAN,
    queued: [{ id: 'q1', prompt: 'Then run the gates', saving: false }],
    enqueue: call('enqueue'),
    requeue: call('requeue'),
    unqueue: call('unqueue'),
    sendQueued: call('sendQueued'),
    pick: CAN,
    catalogue: pickerCatalogue,
    choice: pickerChoice,
    choose: () => undefined,
    sources: FIXTURE_SOURCES,
    ...over,
  };
  return { engine, calls, answer };
}

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe('the composer card', () => {
  it('sends the prompt and empties the editor, and keeps the words when the send fails', async () => {
    const { engine, calls, answer } = engineOf();
    const card = new CardState(() => engine, undefined, '⌘');
    card.prompt = 'Run the gates';
    await card.submit(false);
    expect(calls).toEqual(['send Run the gates']);
    expect(card.prompt).toBe('');
    answer.next = { ok: false, words: 'The runner did not answer.' };
    card.prompt = 'Again';
    await card.submit(false);
    expect(card.prompt).toBe('Again');
    expect(card.problem).toBe('The runner did not answer.');
  });

  it('sends nothing for an empty prompt or a chat that ended', async () => {
    const { engine, calls } = engineOf({ ended: 'The program was closed.' });
    const card = new CardState(() => engine, undefined, '⌘');
    card.prompt = 'Hello';
    await card.submit(false);
    const idle = engineOf();
    await new CardState(() => idle.engine, undefined, '⌘').submit(false);
    expect([...calls, ...idle.calls]).toEqual([]);
  });

  it('steers into a running turn, queues with the modifier, and steers when the engine has no queue', async () => {
    const { engine, calls } = engineOf({ phase: 'running' });
    const card = new CardState(() => engine, undefined, '⌘');
    card.prompt = 'Also the baseline';
    await card.submit(false);
    card.prompt = 'Later';
    await card.submit(true);
    const bare = engineOf({ phase: 'running', queue: cannot('No queue.') });
    const other = new CardState(() => bare.engine, undefined, '⌘');
    other.prompt = 'Now';
    await other.submit(true);
    expect([...calls, ...bare.calls]).toEqual(['send Also the baseline', 'enqueue Later', 'send Now']);
  });

  it('keeps the unsent text of a chat for the next composer of that chat', () => {
    const { engine } = engineOf();
    const first = new CardState(() => engine, 'chat-1', '⌘');
    first.prompt = 'Half a thought';
    first.keep();
    expect(new CardState(() => engine, 'chat-1', '⌘').prompt).toBe('Half a thought');
    expect(new CardState(() => engine, 'chat-2', '⌘').prompt).toBe('');
    first.prompt = '';
    first.keep();
    expect(new CardState(() => engine, 'chat-1', '⌘').prompt).toBe('');
  });

});

describe('the queue, stop and attachments of the card', () => {
  it('opens a queued message, saves it, and gives the chat its own text back', async () => {
    const { engine, calls } = engineOf({ phase: 'running' });
    const card = new CardState(() => engine, undefined, '⌘');
    card.prompt = 'My own words';
    card.edit('q1');
    expect(card.prompt).toBe('Then run the gates');
    expect(card.face.action).toBe('update');
    card.prompt = 'Then run all the gates';
    await card.submit(false);
    expect(calls).toEqual(['requeue q1 Then run all the gates']);
    expect(card.editing).toBeNull();
    expect(card.prompt).toBe('My own words');
    card.edit('q1');
    card.cancelEdit();
    expect(card.prompt).toBe('My own words');
  });

  it('waits ten seconds for a stop, lets it be pressed again, and guards half a second after it lands', async () => {
    const { engine, calls } = engineOf({ phase: 'running' });
    const card = new CardState(() => engine, undefined, '⌘');
    await card.interrupt();
    await card.interrupt();
    expect(calls).toEqual(['interrupt']);
    vi.advanceTimersByTime(10_000);
    expect(card.stop).toBe('timed_out');
    card.turned(false);
    expect(card.stop).toBe('landed');
    card.prompt = 'Too soon';
    await card.submit(false);
    expect(calls).toEqual(['interrupt']);
    vi.advanceTimersByTime(500);
    expect(card.stop).toBe('none');
  });

  it('will not send while an attachment failed', () => {
    const { engine } = engineOf();
    const card = new CardState(() => engine, undefined, '⌘');
    card.attachments = [{ id: 'a1', name: 'x.csv', kind: 'file', size: 0, mime: 'text/csv', preview: null, state: { state: 'failed', words: 'Empty.' } }];
    expect(card.face).toMatchObject({ disabled: true, tip: 'Remove the attachment that failed, then send.' });
    card.remove('a1');
    expect(card.attachments).toEqual([]);
  });
});
