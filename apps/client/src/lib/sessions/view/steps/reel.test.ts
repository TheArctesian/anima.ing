import { expect, it } from 'vitest';
import { reelFocus, rowPose } from './reel';
import { connectionSteps, type Progress } from './steps';

const steps = (progress: Partial<Progress>) =>
  connectionSteps({ ticketed: false, waking: false, opened: false, connected: false, stopped: false, ...progress });

it.each([
  [{}, 0],
  [{ ticketed: true }, 1],
  [{ ticketed: true, stopped: true }, 1],
  [{ ticketed: true, opened: true }, 2],
  [{ ticketed: true, opened: true, connected: true }, 3],
])('5.7.3: for progress %o the reel has step %d in its middle', (progress, focus) => {
  expect(reelFocus(steps(progress))).toBe(focus);
});

it('5.7.3: the step in the middle is whole; each step that ended sits higher, smaller and fainter, then is gone', () => {
  expect(rowPose(0)).toEqual({ y: 0, scale: 1, opacity: 1 });
  const upward = [0, -1, -2, -3].map((place) => rowPose(place));
  const falling = (values: number[]): number[] => [...values].sort((a, b) => b - a);
  for (const part of ['y', 'scale', 'opacity'] as const) {
    const values = upward.map((pose) => pose[part]);
    expect(values, part).toEqual(falling(values));
    expect(new Set(values).size, part).toBe(values.length);
  }
  expect(rowPose(-3).opacity).toBe(0);
});

it('5.7.3: a step that has not started is not shown, and waits under the middle to rise into it', () => {
  for (const place of [1, 2, 3]) {
    expect(rowPose(place).opacity).toBe(0);
    expect(rowPose(place).y).toBeGreaterThan(0);
  }
});
