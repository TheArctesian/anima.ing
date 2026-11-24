import { describe, expect, it, vi } from 'vitest';
import { CardState } from '../card-state.svelte';
import { CAN } from '../draft';
import type { ComposerEngine, Message } from '../engine';
import { FIXTURE_SOURCES } from '../mentions/fixture';
import { pickerCatalogue, pickerChoice } from '../picker/fixture';
import type { Uploaded, UploadWatch } from './upload';

interface Going {
  readonly name: string;
  readonly watch: UploadWatch;
  readonly end: (how: Uploaded) => void;
}

/** An engine whose uploads end when the test says so. */
function engineOf() {
  const going: Going[] = [];
  const sent: Message[] = [];
  const discarded: string[] = [];
  const ok = () => Promise.resolve({ ok: true as const });
  const engine: ComposerEngine = {
    phase: 'idle',
    ended: null,
    send: (message) => {
      sent.push(message);
      return ok();
    },
    interrupt: ok,
    attach: CAN,
    upload: (file, _mime, watch) => new Promise<Uploaded>((end) => going.push({ name: file.name, watch, end })),
    discard: (remote) => {
      discarded.push(remote);
    },
    dictate: CAN,
    steer: CAN,
    queue: CAN,
    queued: [],
    enqueue: ok,
    requeue: ok,
    unqueue: ok,
    sendQueued: ok,
    pick: CAN,
    catalogue: pickerCatalogue,
    choice: pickerChoice,
    choose: ok,
    sources: FIXTURE_SOURCES,
  };
  return { engine, going, sent, discarded };
}

const text = (name: string, size = 10): File => new File([new Uint8Array(size)], name, { type: 'text/plain' });
/** Lets the line of uploads take its next step. */
const settle = async (): Promise<void> => {
  for (let turn = 0; turn < 5; turn += 1) await Promise.resolve();
};

describe('the files of the composer card', () => {
  it('sends the files one after the other, shows how far each is, and sends the message with their ids', async () => {
    const { engine, going, sent } = engineOf();
    const card = new CardState(() => engine, undefined, '⌘');
    card.add([text('a.txt'), text('b.txt')]);
    await settle();
    expect(going.map((file) => file.name)).toEqual(['a.txt']);
    expect(card.face).toMatchObject({ disabled: true, tip: 'An attachment is still being read.' });
    going[0]?.watch.told(4);
    expect(card.attachments[0]?.state).toEqual({ state: 'reading', sent: 4 });
    going[0]?.end({ state: 'sent', id: 'f1' });
    await settle();
    expect(card.attachments[0]).toMatchObject({ state: { state: 'ready' }, remote: 'f1' });
    going[1]?.end({ state: 'sent', id: 'f2' });
    await settle();
    card.prompt = 'Read these';
    await card.submit(false);
    expect(sent[0]?.attachments.map((attachment) => attachment.remote)).toEqual(['f1', 'f2']);
    expect(card.attachments).toEqual([]);
  });

  it('shows the words of a file that did not go, and sends only when it is removed', async () => {
    const { engine, going } = engineOf();
    const card = new CardState(() => engine, undefined, '⌘');
    card.add([text('big.txt')]);
    await settle();
    going[0]?.end({ state: 'failed', words: 'big.txt is too large: the limit is 50 MiB.' });
    await settle();
    expect(card.attachments[0]?.state).toEqual({ state: 'failed', words: 'big.txt is too large: the limit is 50 MiB.' });
    expect(card.face.disabled).toBe(true);
    card.remove(card.attachments[0]?.id ?? '');
    expect(card.attachments).toEqual([]);
  });

});

describe('the files that leave the composer card', () => {
  it('gives up a file that is removed: at once when the runner has it, at its end when it was on its way', async () => {
    const { engine, going, discarded } = engineOf();
    const card = new CardState(() => engine, undefined, '⌘');
    card.add([text('a.txt'), text('b.txt')]);
    await settle();
    going[0]?.end({ state: 'sent', id: 'f1' });
    await settle();
    card.remove(card.attachments[0]?.id ?? '');
    expect(discarded).toEqual(['f1']);
    card.remove(card.attachments[0]?.id ?? '');
    expect(going[1]?.watch.gone()).toBe(true);
    // The last part went before the removal was seen: the file is given up after.
    going[1]?.end({ state: 'sent', id: 'f2' });
    await settle();
    expect(discarded).toEqual(['f1', 'f2']);
    expect(card.attachments).toEqual([]);
  });

  it('keeps a file that was attached while a message was on its way, when that message does not go', async () => {
    const { engine, going } = engineOf();
    let answer: (went: { ok: false; words: string }) => void = () => undefined;
    const card = new CardState(() => ({ ...engine, send: () => new Promise((end) => (answer = end)) }), undefined, '⌘');
    card.prompt = 'First';
    const sending = card.submit(false);
    card.add([text('late.txt')]);
    await settle();
    going[0]?.end({ state: 'sent', id: 'f7' });
    await settle();
    answer({ ok: false, words: 'The runner did not answer.' });
    await sending;
    expect(card.prompt).toBe('First');
    expect(card.attachments).toMatchObject([{ name: 'late.txt', remote: 'f7' }]);
  });

  it('gives up the files of a change of a queued message that is cancelled', async () => {
    const { engine, going, discarded } = engineOf();
    const card = new CardState(() => ({ ...engine, queued: [{ id: 'q1', prompt: 'Then test', saving: false }] }), undefined, '⌘');
    card.edit('q1');
    card.add([text('for-the-queue.txt')]);
    await settle();
    going[0]?.end({ state: 'sent', id: 'f3' });
    await settle();
    card.cancelEdit();
    expect(discarded).toEqual(['f3']);
    expect(card.attachments).toEqual([]);
  });

});

describe('what the composer card refuses and stops', () => {
  it('refuses a folder with words, and shows the name that the program is told', () => {
    const { engine } = engineOf();
    const card = new CardState(() => engine, undefined, '⌘');
    card.add([], 1);
    expect(card.problem).toBe('A folder cannot be attached. Attach the files in it.');
    card.add([text('<b>note<b>\u202e.txt')]);
    expect(card.attachments[0]?.name).toBe('_b_note_b_.txt');
  });

  it('stops the files that are on their way when the composer goes, and does not keep them as unsent', async () => {
    const { engine, going } = engineOf();
    const revoke = vi.spyOn(URL, 'revokeObjectURL');
    const card = new CardState(() => engine, 'chat-files', '⌘');
    card.add([text('a.txt'), text('b.txt')]);
    await settle();
    going[0]?.end({ state: 'sent', id: 'f1' });
    await settle();
    card.keep();
    card.dispose();
    expect(going[1]?.watch.gone()).toBe(true);
    const again = new CardState(() => engine, 'chat-files', '⌘');
    expect(again.attachments.map((attachment) => attachment.name)).toEqual(['a.txt']);
    again.remove(again.attachments[0]?.id ?? '');
    again.keep();
    revoke.mockRestore();
  });
});

describe('the files that the runner holds for the composer card', () => {
  it('gives up the files of a queued message that was open when a different one is opened', async () => {
    const { engine, going, discarded } = engineOf();
    const queued = [
      { id: 'q1', prompt: 'Then test', saving: false },
      { id: 'q2', prompt: 'Then lint', saving: false },
    ];
    const revoke = vi.spyOn(URL, 'revokeObjectURL');
    const card = new CardState(() => ({ ...engine, queued }), undefined, '⌘');
    card.add([text('mine.txt')]);
    await settle();
    going[0]?.end({ state: 'sent', id: 'f1' });
    await settle();
    card.edit('q1');
    card.add([new File([new Uint8Array(4)], 'shot.png', { type: 'image/png' })]);
    await settle();
    going[1]?.end({ state: 'sent', id: 'f2' });
    await settle();
    const preview = card.attachments[0]?.preview;
    card.edit('q2');
    // The file of the first change is given up; the one of the chat's own unsent text is set aside, not lost.
    expect(discarded).toEqual(['f2']);
    expect(revoke).toHaveBeenCalledWith(preview);
    card.cancelEdit();
    expect(card.attachments).toMatchObject([{ name: 'mine.txt', remote: 'f1' }]);
    revoke.mockRestore();
  });

  it('says the reason of the runner under the editor when a file does not go', async () => {
    const said = 'shot.png is too large: too many files wait to go; send or drop some first.';
    const { engine, going } = engineOf();
    const card = new CardState(() => engine, undefined, '⌘');
    card.add([text('shot.png')]);
    await settle();
    going[0]?.end({ state: 'failed', words: said });
    await settle();
    expect(card.problem).toBe(said);
    card.remove(card.attachments[0]?.id ?? '');
    expect(card.problem).toBeNull();
  });

});

describe('the room of the runner, as the composer card counts it', () => {
  it('counts the unsent files of the other chats of the window in the 200 files that the runner holds', async () => {
    const { engine, going } = engineOf();
    const first = new CardState(() => engine, 'chat-full-a', '⌘');
    const second = new CardState(() => engine, 'chat-full-b', '⌘');
    for (const card of [first, second]) {
      card.add(Array.from({ length: 100 }, (_, at) => text(`f${String(at)}.txt`, 1)));
      for (let at = 0; at < 100; at += 1) {
        await settle();
        going.at(-1)?.end({ state: 'sent', id: `held-${String(going.length)}` });
      }
      await settle();
      card.keep();
    }
    going.length = 0;
    const third = new CardState(() => engine, 'chat-full-c', '⌘');
    third.add([text('late.txt')]);
    await settle();
    expect(third.attachments).toEqual([]);
    expect(third.problem).toBe('The files that wait to be sent, of all chats together, can be 200 files and 1 GB. Send or remove the files of a different chat, then attach again.');
    expect(going.map((file) => file.name)).not.toContain('late.txt');
    for (const card of [first, second]) {
      card.dispose();
      for (const attachment of card.attachments) card.remove(attachment.id);
      card.keep();
    }
  });
});

describe('the chat of a file of the composer card', () => {
  it('sends and gives up each file through the engine that took it, also when the engine of a different chat is the one of now', async () => {
    const first = engineOf();
    const second = engineOf();
    let now = first.engine;
    const card = new CardState(() => now, undefined, '⌘');
    card.add([text('a.txt'), text('b.txt')]);
    // A different chat is opened before the line of uploads reaches the files.
    now = second.engine;
    await settle();
    first.going[0]?.end({ state: 'sent', id: 'f1' });
    await settle();
    expect(first.going.map((file) => file.name)).toEqual(['a.txt', 'b.txt']);
    expect(second.going).toEqual([]);
    card.remove(card.attachments[0]?.id ?? '');
    card.dispose();
    first.going[1]?.end({ state: 'sent', id: 'f2' });
    await settle();
    expect(first.discarded).toEqual(['f1', 'f2']);
    expect(second.discarded).toEqual([]);
  });

  it('counts no file that the runner dropped after one hour in its room', async () => {
    const { engine, going } = engineOf();
    let clock = 0;
    const olds = ['chat-old-a', 'chat-old-b'].map((key) => new CardState(() => engine, key, '⌘', () => clock));
    for (const old of olds) {
      old.add(Array.from({ length: 100 }, (_, at) => text(`o${String(at)}.txt`, 1)));
      for (let at = 0; at < 100; at += 1) {
        await settle();
        going.at(-1)?.end({ state: 'sent', id: `old-${String(going.length)}` });
      }
      await settle();
      old.keep();
    }
    const card = new CardState(() => engine, 'chat-new-files', '⌘', () => clock);
    clock = 59 * 60 * 1000;
    card.add([text('over.txt')]);
    expect(card.attachments).toEqual([]);
    clock = 61 * 60 * 1000;
    card.add([text('room.txt')]);
    expect(card.attachments).toHaveLength(1);
    card.dispose();
    for (const old of olds) {
      for (const attachment of old.attachments) old.remove(attachment.id);
      old.keep();
    }
  });
});
