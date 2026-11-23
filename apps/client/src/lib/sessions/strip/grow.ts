import { easeCurve, springCurve } from '$lib/motion/animate';
import { modeTokens, springMotion } from '$lib/motion/options';
import { motionPreference, type MotionPreference } from '$lib/motion/preference';
import { firstDirection, scaledFade } from '$lib/motion/roles/path';
import type { RoleTransition } from '$lib/motion/roles/transitions';
import type { Origin } from '../tabs';

export interface GrowParams {
  readonly origin: Origin | null;
  readonly preference?: MotionPreference;
}

/** The tab's scale when gone (t = 0) and the point it scales about. */
interface Path {
  readonly atZero: number;
  readonly origin?: string;
}

/** A closing tab shrinks to nothing about its own centre, in its place in the strip (5.7.3). */
const closing: Path = { atZero: 0 };

/** The tab starts at the control's width, so it reads as the control growing into the tab. */
function startScale(origin: Origin, tab: DOMRectReadOnly): number {
  return tab.width > 0 ? Math.min(Math.max(origin.width / tab.width, 0), 1) : 0;
}

/** The fixed point of `scale` that centres the scaled tab on the control, so it grows along a straight line (MOT-005). */
function originIn(origin: Origin, tab: DOMRectReadOnly, scale: number): string {
  const controlX = origin.left + origin.width / 2;
  const controlY = origin.top + origin.height / 2;
  const fixed = (control: number, centre: number) => (scale < 1 ? (control - scale * centre) / (1 - scale) : control);
  const x = fixed(controlX, tab.left + tab.width / 2) - tab.left;
  const y = fixed(controlY, tab.top + tab.height / 2) - tab.top;
  return `${String(x)}px ${String(y)}px`;
}

function opening(origin: Origin, tab: DOMRectReadOnly): Path {
  const atZero = startScale(origin, tab);
  return { atZero, origin: originIn(origin, tab, atZero) };
}

function draw(path: Path | null) {
  return scaledFade(path?.atZero, path?.origin);
}

/** The path is fixed by the first direction, so a close mid-open goes back to the control with no step (MOT-002). */
export function grow(node: Element, { origin, preference = motionPreference() }: GrowParams): RoleTransition {
  const { reduced } = preference;
  // `null` once fixed is a fade only, so it is kept too.
  const first = firstDirection<Path | null>();
  return ({ direction } = {}) => {
    if (direction === 'out') {
      const css = draw(first(() => (reduced ? null : closing)));
      return { duration: modeTokens(reduced).duration.fast, easing: easeCurve('exit'), css };
    }
    const motion = springMotion('snappy', reduced);
    if (motion.type !== 'spring' || origin === null) {
      return { duration: modeTokens(reduced).duration.base, easing: easeCurve('enter'), css: draw(first(() => null)) };
    }
    const css = draw(first(() => opening(origin, node.getBoundingClientRect())));
    const { easing, durationMs } = springCurve(motion);
    return { duration: durationMs, easing, css };
  };
}
