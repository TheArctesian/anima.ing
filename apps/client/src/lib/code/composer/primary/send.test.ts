import { describe, expect, it } from 'vitest';
import { CAN, cannot } from '../draft';
import { enterSends, followUpOf, modifierWord, sendFace, showsSend, showsStop, stopAfter, type Turn } from './send';

const idle: Turn = { running: false, filled: true, busy: false, editing: false, followUp: 'steer', alternate: false, steer: CAN, queue: CAN, blocked: null };

describe('the round button of the composer', () => {
  it('sends when no turn runs, and cannot with nothing to send', () => {
    expect(sendFace(idle, '⌘')).toMatchObject({ action: 'send', icon: 'arrow', label: 'Submit message', disabled: false });
    expect(sendFace({ ...idle, filled: false }, '⌘').disabled).toBe(true);
  });

  it('steers into a running turn by default and queues with the modifier', () => {
    const running = { ...idle, running: true };
    expect(sendFace(running, '⌘')).toMatchObject({ action: 'steer', label: 'Steer message', tip: 'Click to steer, ⌘-click to queue' });
    expect(sendFace({ ...running, alternate: true }, 'Ctrl')).toMatchObject({ action: 'queue', icon: 'list-plus' });
    expect(sendFace({ ...running, followUp: 'queue' }, 'Ctrl').tip).toBe('Click to queue, Ctrl-click to steer');
  });

  it('never offers a follow-up the engine lacks, and says why', () => {
    const running = { ...idle, running: true, queue: cannot('The runner keeps no queue yet.') };
    expect(followUpOf({ ...running, alternate: true })).toBe('steer');
    expect(sendFace(running, '⌘').tip).toBe('Click to steer. The runner keeps no queue yet.');
    expect(followUpOf({ ...running, followUp: 'queue', steer: cannot('x') })).toBe('queue');
  });

  it('says why when nothing can be sent, and that a send is on its way', () => {
    expect(sendFace({ ...idle, blocked: 'The program of this chat ended.' }, '⌘')).toMatchObject({ disabled: true, tip: 'The program of this chat ended.' });
    expect(sendFace({ ...idle, busy: true }, '⌘')).toMatchObject({ disabled: true, label: 'Submitting message' });
    expect(sendFace({ ...idle, editing: true, running: true }, '⌘')).toMatchObject({ action: 'update', icon: 'check', label: 'Update queued message' });
  });
});

describe('stop', () => {
  it('stands in place of send only while a turn runs with an empty prompt', () => {
    expect(showsStop({ running: true, filled: false, editing: false }, 'none')).toBe(true);
    expect(showsSend({ running: true, filled: false, editing: false })).toBe(false);
    expect(showsStop({ running: true, filled: true, editing: false }, 'none')).toBe(false);
    expect(showsStop({ running: true, filled: true, editing: false }, 'pending')).toBe(true);
    expect(showsStop({ running: false, filled: false, editing: false }, 'none')).toBe(false);
  });

  it('lands when the turn stops and is forgotten when the next one runs', () => {
    expect(stopAfter('pending', false)).toBe('landed');
    expect(stopAfter('timed_out', false)).toBe('landed');
    expect(stopAfter('landed', true)).toBe('none');
    expect(stopAfter('pending', true)).toBe('pending');
  });
});

describe('keys', () => {
  it('Enter sends; Shift, Alt and an input method type', () => {
    const enter = { key: 'Enter', shift: false, alt: false, composing: false };
    expect(enterSends(enter)).toBe(true);
    expect(enterSends({ ...enter, shift: true })).toBe(false);
    expect(enterSends({ ...enter, alt: true })).toBe(false);
    expect(enterSends({ ...enter, composing: true })).toBe(false);
    expect(enterSends({ ...enter, key: 'a' })).toBe(false);
  });

  it('names the modifier of the platform', () => {
    expect(modifierWord('MacIntel')).toBe('⌘');
    expect(modifierWord('Linux x86_64')).toBe('Ctrl');
  });
});
