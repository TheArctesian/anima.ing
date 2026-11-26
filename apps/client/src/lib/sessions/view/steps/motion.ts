import type { Attachment } from 'svelte/attachments';
import type { TransitionConfig } from 'svelte/transition';
import { ambientAllowedNow } from '$lib/motion/ambient';
import { easeCurve, springCurve } from '$lib/motion/animate';
import { modeTokens, springMotion, type SpringName } from '$lib/motion/options';
import { motionPreference, type MotionPreference } from '$lib/motion/preference';
import { duration } from '$lib/motion/tokens';

/*
 * The motion of the connection steps (5.7.3 "Connection steps"; operator, 2026-11-26). Each motion says something: the
 * ring, the arc and the light say which step runs now and run only while it runs, in a visible window without reduced
 * motion (MOT-011); the ripple and the check say that a step ended; the line under the steps says what runs. Each moves
 * only transform and opacity (MOT-006).
 */

/** How far the ring of a step goes out from its mark, as a scale of the mark. */
const BEACON_SCALE = 2;
const LANDED_SCALE = 1.8;
/** The light is 2/5 of a lane wide, and crosses the whole lane from before its start to past its end. */
const LIGHT_FROM = '-100%';
const LIGHT_TO = '250%';
/** The check grows from this scale into the mark; the new line rises this far into the old one's place. */
const CHECK_FROM = 0.6;
const RISE_PX = 6;
/** The reel comes up this far into the pane when the start begins. */
const ARRIVE_PX = 16;

type Animated = HTMLElement & { animate?: HTMLElement['animate'] };

/** `ease-enter`, read from the tokens' CSS, which already holds the curve for this motion setting. */
function enterCurve(element: Element): string {
  return getComputedStyle(element).getPropertyValue('--ease-enter').trim() || 'linear';
}

/** The ring of the step that runs: it goes out from the mark and fades, again and again, while the step runs. */
export function beacon(preference: MotionPreference = motionPreference()): Attachment<Animated> {
  return (ring) => {
    if (!ambientAllowedNow(preference) || ring.animate === undefined) return;
    const beat = ring.animate([{ scale: 1, opacity: 0.6 }, { scale: BEACON_SCALE, opacity: 0 }], {
      duration: duration.highlight, iterations: Infinity, easing: enterCurve(ring),
    });
    return () => {
      beat.cancel();
    };
  };
}

/** The arc around the mark of the step that runs: it turns, once in `duration-highlight`, while the step runs. */
export function spin(preference: MotionPreference = motionPreference()): Attachment<Animated> {
  return (arc) => {
    if (!ambientAllowedNow(preference) || arc.animate === undefined) return;
    const turn = arc.animate([{ rotate: '0turn' }, { rotate: '1turn' }], { duration: duration.highlight, iterations: Infinity });
    return () => {
      turn.cancel();
    };
  };
}

/** A light that runs along the segment of the step that runs: the start is on its way through that step. */
export function signal(preference: MotionPreference = motionPreference()): Attachment<Animated> {
  return (light) => {
    if (!ambientAllowedNow(preference) || light.animate === undefined) return;
    const run = light.animate(
      [{ translate: `${LIGHT_FROM} 0`, opacity: 0 }, { opacity: 1, offset: 0.35 }, { translate: `${LIGHT_TO} 0`, opacity: 0 }],
      { duration: duration.highlight, iterations: Infinity, easing: enterCurve(light) },
    );
    return () => {
      run.cancel();
    };
  };
}

/** One ring goes out from the mark of a step that ended; with reduced motion the check alone says it. */
export function landed(preference: MotionPreference = motionPreference()): Attachment<Animated> {
  return (ring) => {
    if (preference.reduced || ring.animate === undefined) return;
    const out = ring.animate(
      [{ scale: 1, opacity: 0.5, visibility: 'visible' }, { scale: LANDED_SCALE, opacity: 0, visibility: 'visible' }],
      { duration: duration.base * 3, easing: enterCurve(ring) },
    );
    return () => {
      out.cancel();
    };
  };
}

/** The reel comes up into the pane when the start begins; with reduced motion it is there at once. */
export function arrive(preference: MotionPreference = motionPreference()): Attachment<Animated> {
  return (reel) => {
    if (preference.reduced || reel.animate === undefined) return;
    const up = reel.animate([{ opacity: 0, translate: `0 ${String(ARRIVE_PX)}px` }, { opacity: 1, translate: '0 0' }], {
      duration: duration.base * 2, easing: enterCurve(reel),
    });
    return () => {
      up.cancel();
    };
  };
}

type Direction = 'in' | 'out' | 'both';

function sprung(name: SpringName, reduced: boolean, css: (t: number) => string): TransitionConfig {
  const motion = springMotion(name, reduced);
  if (motion.type !== 'spring') return { duration: modeTokens(reduced).duration.base, easing: easeCurve('enter'), css: (t) => `opacity: ${String(t)};` };
  const { easing, durationMs } = springCurve(motion);
  return { duration: durationMs, easing, css };
}

function leave(reduced: boolean, css: (t: number) => string): TransitionConfig {
  return { duration: modeTokens(reduced).duration.fast, easing: easeCurve('exit'), css: reduced ? (t) => `opacity: ${String(t)};` : css };
}

/** The mark of a step: a new mark grows into the node on `spring-snappy`, the old one shrinks out. */
export function pop(_node: Element, { preference = motionPreference() }: { preference?: MotionPreference } = {}) {
  return ({ direction }: { direction?: Direction } = {}): TransitionConfig => {
    const css = (t: number): string => `opacity: ${String(t)}; scale: ${String(CHECK_FROM + (1 - CHECK_FROM) * t)};`;
    return direction === 'out' ? leave(preference.reduced, css) : sprung('snappy', preference.reduced, css);
  };
}

/** The line under the steps: the new line rises into place on `spring-default` while the old one fades up and out. */
export function rise(_node: Element, { preference = motionPreference() }: { preference?: MotionPreference } = {}) {
  return ({ direction }: { direction?: Direction } = {}): TransitionConfig =>
    direction === 'out'
      ? leave(preference.reduced, (t) => `opacity: ${String(t)}; translate: 0 ${String((t - 1) * RISE_PX)}px;`)
      : sprung('default', preference.reduced, (t) => `opacity: ${String(t)}; translate: 0 ${String((1 - t) * RISE_PX)}px;`);
}
