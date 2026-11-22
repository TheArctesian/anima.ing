import { describe, expect, it } from 'vitest';
import { attendAll, HeldQueue, heldQueue, NOT_WATCHED, QUEUE_MOST, type Went } from './held.svelte';

/** A chat that records what it was sent, and can be made to refuse. */
function chat(): { sent: string[]; refuse: string | null; send: (prompt: string) => Promise<Went> } {
  const made = {
    sent: [] as string[],
    refuse: null as string | null,
    send: (prompt: string): Promise<Went> => {
      if (made.refuse !== null) return Promise.resolve({ ok: false, words: made.refuse });
      made.sent.push(prompt);
      return Promise.resolve({ ok: true });
    },
  };
  return made;
}

const prompts = (queue: HeldQueue): string[] => queue.messages.map((message) => message.prompt);
const settled = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** One turn of the chat: it runs, then it ends. */
async function turn(queue: HeldQueue, send: (prompt: string) => Promise<Went>): Promise<void> {
  queue.attend('running', send);
  queue.attend('idle', send);
  await settled();
}

describe('a queue held by the window', () => {
  it('sends the first message when the turn ends, one a turn, in order', async () => {
    const queue = new HeldQueue();
    const to = chat();
    queue.attend('running', to.send);
    queue.add('first');
    queue.add('second');
    queue.attend('running', to.send);
    expect(to.sent).toEqual([]);
    queue.attend('idle', to.send);
    await settled();
    expect(to.sent).toEqual(['first']);
    expect(prompts(queue)).toEqual(['second']);
    queue.attend('idle', to.send);
    await settled();
    expect(to.sent).toEqual(['first']);
    await turn(queue, to.send);
    expect(to.sent).toEqual(['first', 'second']);
    expect(queue.empty).toBe(true);
  });

  it('sends nothing when a chat is first seen idle', async () => {
    const queue = new HeldQueue();
    const to = chat();
    queue.add('waits');
    queue.attend('idle', to.send);
    await settled();
    expect(to.sent).toEqual([]);
  });

  it('keeps a message that did not go, in its place, and says why it stopped', async () => {
    const queue = new HeldQueue();
    const to = chat();
    queue.add('first');
    queue.add('second');
    to.refuse = 'The runner did not answer.';
    await turn(queue, to.send);
    expect(prompts(queue)).toEqual(['first', 'second']);
    expect(queue.waits).toBe('The queue stopped: The runner did not answer.');
    to.refuse = null;
    await turn(queue, to.send);
    expect(to.sent).toEqual([]);
    expect(await queue.sendNow(queue.messages[1]?.id ?? '', to.send)).toEqual({ ok: true });
    expect(to.sent).toEqual(['second']);
    expect(queue.waits).toBeNull();
    await turn(queue, to.send);
    expect(to.sent).toEqual(['second', 'first']);
  });

});

describe('what makes a held queue wait', () => {
  it('waits after a stop by the person until they send something', async () => {
    const queue = new HeldQueue();
    const to = chat();
    queue.attend('running', to.send);
    queue.add('after the stop');
    queue.hold('You stopped the turn.');
    queue.attend('idle', to.send);
    await settled();
    expect(to.sent).toEqual([]);
    expect(queue.waits).toBe('You stopped the turn.');
    queue.release();
    await turn(queue, to.send);
    expect(to.sent).toEqual(['after the stop']);
  });

  it('holds nothing back when the queue is empty at the stop', () => {
    const queue = new HeldQueue();
    queue.hold('You stopped the turn.');
    expect(queue.waits).toBeNull();
  });

  it('does not send the message that is open in the composer, and sends it when it is closed', async () => {
    const queue = new HeldQueue();
    const to = chat();
    queue.attend('running', to.send);
    queue.add('draft');
    const id = queue.messages[0]?.id ?? '';
    queue.opened(id);
    queue.attend('idle', to.send);
    await settled();
    expect(to.sent).toEqual([]);
    queue.change(id, 'final');
    queue.opened(null);
    await settled();
    expect(to.sent).toEqual(['final']);
  });

  it('keeps a message due, with no word of a failure, when its chat is not the one on screen', async () => {
    const queue = new HeldQueue();
    const to = chat();
    queue.add('due');
    await turn(queue, () => Promise.resolve(NOT_WATCHED));
    expect(queue.waits).toBeNull();
    queue.attend('idle', to.send);
    await settled();
    expect(to.sent).toEqual(['due']);
  });

  it('sends nothing while nobody watches the chat, and catches up when it is watched again', async () => {
    const queue = new HeldQueue();
    const to = chat();
    queue.attend('running', to.send);
    queue.add('later');
    queue.leave();
    queue.opened(null);
    await settled();
    expect(to.sent).toEqual([]);
    queue.attend('idle', to.send);
    await settled();
    expect(to.sent).toEqual(['later']);
  });

});

describe('the limits of a held queue', () => {
  it('holds twenty messages at most, and refuses a change or a removal of one that is gone', () => {
    const queue = new HeldQueue();
    for (let count = 0; count < QUEUE_MOST; count += 1) expect(queue.add(`m${String(count)}`).ok).toBe(true);
    expect(queue.add('one more')).toEqual({ ok: false, words: 'The queue holds 20 messages. Remove one, or wait for the turn to end.' });
    expect(queue.change('none', 'x').ok).toBe(false);
    expect(queue.remove('none').ok).toBe(false);
    expect(queue.remove(queue.messages[0]?.id ?? '').ok).toBe(true);
    expect(queue.messages).toHaveLength(QUEUE_MOST - 1);
  });

  it('counts a sender that throws as a message that did not go', async () => {
    const queue = new HeldQueue();
    queue.add('kept');
    await turn(queue, () => Promise.reject(new Error('socket closed')));
    expect(prompts(queue)).toEqual(['kept']);
    expect(queue.waits).toBe('The queue stopped: The message could not be sent.');
  });
});

describe('the queues of every chat', () => {
  it('sends to the chat of the queue only, and leaves a chat with no queue alone', async () => {
    const sent: [string, string][] = [];
    const send = (to: string, prompt: string): Promise<Went> => {
      sent.push([to, prompt]);
      return Promise.resolve({ ok: true });
    };
    heldQueue('all-a').add('for a');
    attendAll([{ id: 'all-a', phase: 'running' }, { id: 'all-b', phase: 'running' }], send);
    attendAll([{ id: 'all-a', phase: 'idle' }, { id: 'all-b', phase: 'idle' }], send);
    await settled();
    expect(sent).toEqual([['all-a', 'for a']]);
  });
});
