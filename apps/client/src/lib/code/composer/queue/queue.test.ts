import { describe, expect, it } from 'vitest';
import { foldWords, labelled, queueKeys, queueName, sendNowTip, sendsFirst, shown, type Queued } from './queue';

const queue = (count: number): readonly Queued[] => Array.from({ length: count }, (_, index) => ({ id: String(index), prompt: `message ${String(index)}`, saving: false }));

describe('the queued messages', () => {
  it('shows three in full and folds four to the first two', () => {
    expect(shown(queue(3), false)).toHaveLength(3);
    expect(shown(queue(4), false).map((message) => message.id)).toEqual(['0', '1']);
    expect(shown(queue(4), true)).toHaveLength(4);
  });

  it('names the list and the fold', () => {
    expect(queueName(1)).toBe('1 queued message');
    expect(queueName(4)).toBe('4 queued messages');
    expect(foldWords(4, false)).toBe('Show all 4 queued');
    expect(foldWords(4, true)).toBe('Show fewer');
  });

  it('says where "Send now" sends', () => {
    expect(sendNowTip(true)).toBe('Send into the current turn');
    expect(sendNowTip(false)).toBe('Start a turn with this message');
  });

  it('shows each chip by its label', () => {
    const prompt = 'Read [gate.ts](quality/gate.ts) then $review it';
    const link = '[gate.ts](quality/gate.ts)';
    const chips = [
      { start: prompt.indexOf('$review'), end: prompt.indexOf('$review') + 7, label: 'review', icon: 'sparkles' as const, detail: 'review' },
      { start: prompt.indexOf(link), end: prompt.indexOf(link) + link.length, label: 'gate.ts', icon: 'file-code' as const, detail: 'quality/gate.ts' },
    ];
    expect(labelled(prompt, chips)).toBe('Read gate.ts then review it');
    expect(labelled('plain', [])).toBe('plain');
    expect(labelled('ab', [{ start: 1, end: 9, label: 'x', icon: 'file' as const, detail: '' }])).toBe('ab');
  });
});

describe('the keys of the queue', () => {
  it('names the shortcut in the tip of the message that has it', () => {
    expect(sendNowTip(true, queueKeys(true).send)).toBe('Send into the current turn (⌘⇧Enter)');
    expect(sendNowTip(false, queueKeys(false).send)).toBe('Start a turn with this message (Ctrl+Shift+Enter)');
    expect(sendNowTip(true)).toBe('Send into the current turn');
    expect(queueKeys(true).edit).toBe('⌥↑');
  });

  it('sends the first message on Enter with Shift and ⌘ or Ctrl, and on nothing else', () => {
    const press = { key: 'Enter', shift: true, alt: false, command: true };
    expect(sendsFirst(press)).toBe(true);
    for (const other of [{ shift: false }, { command: false }, { alt: true }, { key: 'a' }]) expect(sendsFirst({ ...press, ...other })).toBe(false);
  });
});
