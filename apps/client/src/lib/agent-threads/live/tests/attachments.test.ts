import { describe, expect, it } from 'vitest';
import { outcomeFrom, updateFrom } from '../run';

describe('the files that go with a prompt', () => {
  it('TH111: a part of a file is answered with its ID and the bytes received so far', () => {
    expect(outcomeFrom({ outcome: 'attached', id: 'f1', received: 2097152 })).toEqual({ outcome: 'attached', id: 'f1', received: 2097152 });
    expect(outcomeFrom({ outcome: 'attached', id: 'f1' })).toBeNull();
  });

  it('TH111: a refusal of a file has the words that name it', () => {
    expect(outcomeFrom({ outcome: 'refused', why: { refusal: 'too_large', said: 'big.png is too large: the limit is 10 MiB.' } })).toEqual({
      outcome: 'refused',
      why: 'too_large',
      said: 'big.png is too large: the limit is 10 MiB.',
    });
    expect(outcomeFrom({ outcome: 'refused', why: { refusal: 'cannot_attach', said: 'x' } })).toMatchObject({ why: 'cannot_attach' });
  });

  it('TH111: a prompt says how many images went and the names of the other files; an older runner says none', () => {
    const withFiles = updateFrom({ thread: 't', seq: 1, change: 'prompt', text: 'look', source: 'this computer', images: 2, files: ['notes.txt'] });
    expect(withFiles?.change).toEqual({ change: 'prompt', text: 'look', source: 'this computer', images: 2, files: ['notes.txt'] });
    const older = updateFrom({ thread: 't', seq: 2, change: 'prompt', text: 'hi', source: 'this computer' });
    expect(older?.change).toEqual({ change: 'prompt', text: 'hi', source: 'this computer', images: 0, files: [] });
  });
});
