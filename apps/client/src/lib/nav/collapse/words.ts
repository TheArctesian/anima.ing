import { animate, together, type Playback } from '$lib/motion/animate';
import { tweenMotion } from '$lib/motion/options';
import { motionPreference, type MotionPreference } from '$lib/motion/preference';

/** The words fade in as the bar expands (NAV-006); only opacity moves, the width switches at once (MOT-006). */
export function fadeInWords(bar: ParentNode, preference: MotionPreference = motionPreference()): Playback {
  const motion = tweenMotion('base', 'enter', preference.reduced);
  return together(
    [...bar.querySelectorAll<HTMLElement>('.words')].map((words) => {
      animate(words, { opacity: 0 }, { type: 'instant' });
      return animate(words, { opacity: 1 }, motion);
    }),
  );
}
