import { modeTokens, staggerDelayMs } from '../options';
import { motionPreference, type MotionPreference } from '../preference';

/** How far the content rises and how soft it starts (UR-7, D12). */
export const RESOLVE_RISE_PX = 4;
export const RESOLVE_BLUR_PX = 6;

/** Under reduced motion, opacity only (MOT-041). */
export function resolveFrames(reduced: boolean): Keyframe[] {
  if (reduced) return [{ opacity: 0 }, { opacity: 1 }];
  return [
    { opacity: 0, translate: `0 ${String(RESOLVE_RISE_PX)}px`, filter: `blur(${String(RESOLVE_BLUR_PX)}px)` },
    { opacity: 1, translate: '0 0', filter: 'blur(0)' },
  ];
}

/** The resolve entrance (UR-7, D12, MOT-053): it plays once, when the node is mounted, never when a list inside re-renders. */
export function resolve(preference: MotionPreference = motionPreference()) {
  return (node: Element): (() => void) => {
    const reduced = preference.reduced;
    const durationMs = modeTokens(reduced).duration.base;
    if (typeof Element.prototype.animate !== 'function') return () => undefined;
    // The curve is the `ease-enter` token, read where the stylesheet defines it (TOK-003).
    const easing = getComputedStyle(document.documentElement).getPropertyValue('--ease-enter').trim() || 'ease-out';
    const playing = [...node.children].map((child, index) =>
      child.animate(resolveFrames(reduced), {
        duration: durationMs,
        delay: staggerDelayMs(index, reduced),
        easing,
        fill: 'backwards',
      }),
    );
    return () => {
      for (const animation of playing) animation.cancel();
    };
  };
}
