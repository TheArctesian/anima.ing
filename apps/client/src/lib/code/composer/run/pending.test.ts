import { describe, expect, it } from 'vitest';
import { PENDING, roomFor } from './pending';

const size = (file: { size: number }): number => file.size;

describe('the room of the runner for files that wait', () => {
  it('takes files up to 200 together with the ones that wait, in order', () => {
    const waiting = Array.from({ length: PENDING.count - 1 }, () => ({ size: 1 }));
    expect(roomFor(waiting, [{ size: 1 }, { size: 2 }], size)).toEqual({ fits: [{ size: 1 }], over: 1 });
  });

  it('takes a smaller file after one that does not fit in the 1 GB', () => {
    const waiting = [{ size: PENDING.bytes - 10 }];
    expect(roomFor(waiting, [{ size: 11 }, { size: 10 }, { size: 1 }], size)).toEqual({ fits: [{ size: 10 }], over: 2 });
  });
});
