import { describe, expect, it } from 'vitest';
import type { Chip } from '../draft';
import { outsideChips, soundChips, touchesChip, wholeChips } from './chips';

function chip(start: number, end: number): Chip {
  return { start, end, label: 'a.ts', icon: 'file', detail: 'src/a.ts' };
}

describe('the chips of a prompt', () => {
  it('keeps the chips that are whole, on one line, in order and apart', () => {
    const prompt = 'see [a](a.ts) and\n[b](b.ts)';
    expect(soundChips(prompt, [chip(18, 27), chip(4, 13)])).toEqual([chip(4, 13), chip(18, 27)]);
    expect(soundChips(prompt, [chip(4, 13), chip(10, 16), chip(13, 13), chip(14, 20), chip(20, 99), chip(-1, 2), chip(0.5, 2)])).toEqual([chip(4, 13)]);
  });

  it('widens a stretch to the chips it cuts', () => {
    const chips = [chip(4, 13)];
    expect(wholeChips({ start: 2, end: 6 }, chips)).toEqual({ start: 2, end: 13 });
    expect(wholeChips({ start: 12, end: 15 }, chips)).toEqual({ start: 4, end: 15 });
    expect(wholeChips({ start: 0, end: 4 }, chips)).toEqual({ start: 0, end: 4 });
    expect(wholeChips({ start: 13, end: 13 }, chips)).toEqual({ start: 13, end: 13 });
  });

  it('moves a caret out of a chip, to its nearer edge', () => {
    const chips = [chip(4, 13)];
    expect([3, 4, 5, 8, 9, 12, 13].map((at) => outsideChips(at, chips))).toEqual([3, 4, 4, 4, 13, 13, 13]);
  });

  it('says whether a selection touches a chip', () => {
    const chips = [chip(4, 13)];
    expect(touchesChip({ start: 0, end: 4 }, chips)).toBe(false);
    expect(touchesChip({ start: 0, end: 5 }, chips)).toBe(true);
    expect(touchesChip({ start: 13, end: 20 }, chips)).toBe(false);
  });
});
