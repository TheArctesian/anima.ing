import { invoke } from '@tauri-apps/api/core';
import { capabilities } from '$lib/platform/platform';

/** A feedback pattern of iOS, named for what it means: a step under the finger, a task that ended well, a task that failed. */
export type Feel = 'selection' | 'success' | 'error';

/**
 * Plays `feel` on an iPhone and nothing elsewhere (mobile plan M4-4, MOT-050). iOS plays nothing when System Haptics is
 * off in Settings, so the app has no setting of its own. Call it in the same frame as the change on the screen (MOT-051).
 */
export function playHaptic(feel: Feel): void {
  if (!capabilities().haptics) return;
  // The feedback only doubles a change that the screen shows, so one that does not play blocks nothing.
  invoke('haptic_play', { feel }).catch((reason: unknown) => {
    console.warn('The haptic feedback did not play:', reason instanceof Error ? reason.name : 'unknown');
  });
}
