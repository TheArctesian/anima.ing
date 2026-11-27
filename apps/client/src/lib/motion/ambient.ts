import { createSubscriber } from 'svelte/reactivity';
import type { MotionPreference } from './preference';

/** Repeating motion (the "Live" indicator, the terminal cursor) runs only in a visible window without reduced motion (MOT-011). */
export function ambientMotionAllowed(visibility: DocumentVisibilityState, reduced: boolean): boolean {
  return visibility === 'visible' && !reduced;
}

const watchVisibility = createSubscriber((update) => {
  document.addEventListener('visibilitychange', update);
  return () => {
    document.removeEventListener('visibilitychange', update);
  };
});

/** Whether repeating motion may run now; reading it in an attachment starts or stops the motion when that changes. */
export function ambientAllowedNow(preference: MotionPreference): boolean {
  if (typeof document === 'undefined') return false;
  watchVisibility();
  return ambientMotionAllowed(document.visibilityState, preference.reduced);
}
