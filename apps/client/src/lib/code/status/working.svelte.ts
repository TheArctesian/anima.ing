import type { Attachment } from 'svelte/attachments';
import { createSubscriber } from 'svelte/reactivity';
import { ambientMotionAllowed } from '$lib/motion/ambient';
import { motionPreference, type MotionPreference } from '$lib/motion/preference';

/*
 * The two repeating motions of live work, copied from T3 Code (operator, 2026-11-23: "A: copy T3"; MOT-011). Each runs
 * only in a visible window without reduced motion, by the Settings choice or the system (MOT-043), and stops when the
 * work stops; each moves opacity or transform only. The times are T3's own (`apps/web/src/index.css`), not tokens.
 */

/** T3's `status-pulse`: 2 s, full for 40%, six steps down to half, half for 40%, six steps back. */
const PULSE_MS = 2000;
const PULSE: Keyframe[] = [
  { offset: 0, opacity: 1 },
  { offset: 0.4, opacity: 1, easing: 'steps(6)' },
  { offset: 0.5, opacity: 0.5 },
  { offset: 0.9, opacity: 0.5, easing: 'steps(6)' },
  { offset: 1, opacity: 1 },
];

/** T3's `live-tool-shine`: a band crosses the words in 2.2 s, in 30 steps so the screen changes 30 times, not each frame. */
const SHINE_MS = 2200;
const SHINE_STEPS = 'steps(30)';

const watchVisibility = createSubscriber((update) => {
  document.addEventListener('visibilitychange', update);
  return () => {
    document.removeEventListener('visibilitychange', update);
  };
});

/** Whether repeating motion may run now; reading it in an attachment starts or stops the motion when that changes. */
function allowed(preference: MotionPreference): boolean {
  if (typeof document === 'undefined') return false;
  watchVisibility();
  return ambientMotionAllowed(document.visibilityState, preference.reduced);
}

/** The pulse of the mark of a thread that works; every mark pulses in time with the others, as in T3. */
export function pulse(preference: MotionPreference = motionPreference()): Attachment<HTMLElement> {
  return (node) => {
    if (!allowed(preference)) return;
    const pulsing = node.animate(PULSE, { duration: PULSE_MS, iterations: Infinity });
    pulsing.startTime = 0;
    return () => {
      pulsing.cancel();
    };
  };
}

/**
 * The shine of the line of a tool that runs: the host holds the words, and a band (`[data-band]`) with a brighter copy
 * of them. The band slides across the host while the copy slides back as far, so the bright words stand still under
 * the moving band. The host is marked `data-shining` while it runs.
 */
export function shine(preference: MotionPreference = motionPreference()): Attachment<HTMLElement> {
  return (host) => {
    const band = host.querySelector<HTMLElement>('[data-band]');
    const copy = band?.firstElementChild;
    if (!allowed(preference) || band === null || !(copy instanceof HTMLElement)) return;
    // The band shows only while the shine runs, so it is marked before it is measured.
    host.dataset['shining'] = '';
    const width = host.clientWidth;
    const reach = band.offsetWidth;
    copy.style.inlineSize = `${String(width)}px`;
    const timing: KeyframeAnimationOptions = { duration: SHINE_MS, iterations: Infinity, easing: SHINE_STEPS };
    const moving = band.animate([{ transform: `translateX(${String(-reach)}px)` }, { transform: `translateX(${String(width)}px)` }], timing);
    const holding = copy.animate([{ transform: `translateX(${String(reach)}px)` }, { transform: `translateX(${String(-width)}px)` }], timing);
    return () => {
      moving.cancel();
      holding.cancel();
      delete host.dataset['shining'];
    };
  };
}
