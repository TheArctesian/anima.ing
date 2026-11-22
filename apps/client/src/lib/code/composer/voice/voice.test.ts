import { describe, expect, it } from 'vitest';
import { bars, BARS, clock, phaseWords } from './voice';

describe('voice input', () => {
  it('writes the time as minutes and seconds, with the limit when there is one', () => {
    expect(clock(0)).toBe('0:00');
    expect(clock(75.9)).toBe('1:15');
    expect(clock(42, 120)).toBe('0:42 / 2:00');
  });

  it('draws 24 bars, the newest last, none under a fifth', () => {
    expect(bars([])).toEqual(Array.from({ length: BARS }, () => 0.2));
    const drawn = bars([0, 0.5, 2]);
    expect(drawn).toHaveLength(BARS);
    expect(drawn.slice(-3)).toEqual([0.2, 0.5, 1]);
    expect(bars(Array.from({ length: 30 }, (_, index) => index / 30))).toHaveLength(BARS);
  });

  it('has words for each phase', () => {
    expect(phaseWords('transcribing')).toBe('Transcribing…');
    expect(phaseWords('cleaning')).toBe('Cleaning up…');
    expect(phaseWords('recording')).toBe('Recording');
    expect(phaseWords('preparing')).toBe('Starting…');
  });
});
