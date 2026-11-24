import { nearestSnap } from '$lib/motion/gesture';

export interface Slot {
  readonly left: number;
  readonly width: number;
}

export interface Sample {
  readonly x: number;
  readonly y: number;
  readonly t: number;
}

export interface Velocity {
  readonly x: number;
  readonly y: number;
}

export interface Bounds {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

/** Only the pointer's last movement counts for the release velocity; an older one has stopped. */
export const VELOCITY_WINDOW_MS = 100;

const msPerSecond = 1000;

const centreOf = (slot: Slot) => slot.left + slot.width / 2;

/** The slot nearest the projected centre (MOT-032). */
export function slotIndex(slots: readonly Slot[], centre: number, velocity: number): number {
  const [first, ...rest] = slots.map(centreOf);
  if (first === undefined) return 0;
  const snap = nearestSnap(centre, velocity, [first, ...rest]);
  return [first, ...rest].indexOf(snap);
}

function gapOf(slots: readonly Slot[]): number {
  const [first, second] = slots;
  return first === undefined || second === undefined ? 0 : second.left - (first.left + first.width);
}

/** Passed tabs move by the dragged width and one gap toward its old place (Section 5.7.4). */
export function shifts(slots: readonly Slot[], from: number, over: number): number[] {
  const dragged = slots[from];
  if (dragged === undefined) return slots.map(() => 0);
  const pitch = dragged.width + gapOf(slots);
  return slots.map((_, index) => {
    if (from < index && index <= over) return -pitch;
    if (over <= index && index < from) return pitch;
    return 0;
  });
}

/** Px/s over the last `VELOCITY_WINDOW_MS` (MOT-031). */
export function velocityOf(samples: readonly Sample[]): Velocity {
  const last = samples.at(-1);
  if (last === undefined) return { x: 0, y: 0 };
  const recent = samples.filter((sample) => last.t - sample.t <= VELOCITY_WINDOW_MS);
  const first = recent[0] ?? last;
  const elapsed = last.t - first.t;
  if (elapsed <= 0) return { x: 0, y: 0 };
  return { x: ((last.x - first.x) / elapsed) * msPerSecond, y: ((last.y - first.y) / elapsed) * msPerSecond };
}

/** A pointer past `margin` means the tab has left the strip (RA-002). */
export function isOutside(x: number, y: number, bounds: Bounds, margin: number): boolean {
  return (
    x < bounds.left - margin || x > bounds.right + margin || y < bounds.top - margin || y > bounds.bottom + margin
  );
}
