import type { Step } from './steps';

/*
 * The steps stand in a reel in the middle of the pane (5.7.3; operator, 2026-11-26: "centered in the middle of the
 * screen shown one by one"). The step that runs is in the middle. The steps that ended roll up above it and get smaller
 * and fainter. A step that has not started is not shown: it waits under the middle and rises into it when it starts.
 */

/** The step in the middle of the reel: the one that runs or stopped, else the last. */
export function reelFocus(steps: readonly Step[]): number {
  const running = steps.findIndex((step) => step.mark === 'active' || step.mark === 'failed');
  return running === -1 ? steps.length - 1 : running;
}

export interface Pose {
  /** Where the row sits, in rows of the reel from the middle; up is negative. */
  readonly y: number;
  readonly scale: number;
  readonly opacity: number;
}

/** The middle row, then the rows over it that stay in sight. */
const SEEN: readonly Pose[] = [
  { y: 0, scale: 1, opacity: 1 },
  { y: -1, scale: 0.8, opacity: 0.55 },
  { y: -1.75, scale: 0.65, opacity: 0.25 },
];
/** A row that rolled up past the top. */
const GONE: Pose = { y: -2.4, scale: 0.55, opacity: 0 };
/** A step that has not started, under the middle. */
const NEXT: Pose = { y: 0.75, scale: 0.9, opacity: 0 };

/** Where the row of a step sits, by its place from the middle: 0 is the middle, -1 the row above it, 1 a later step. */
export function rowPose(place: number): Pose {
  if (place > 0) return NEXT;
  return SEEN[-place] ?? GONE;
}
