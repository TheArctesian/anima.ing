import { playHaptic, type Feel } from '$lib/desktop/haptics/haptics';
import type { Progress } from './steps';

const failed = (progress: Progress): boolean => progress.stopped && !progress.connected;

/** A session that ends after it connected plays nothing: that end is not a start that failed. */
function startFeel(before: Progress, now: Progress): Feel | null {
  if (now.connected && !before.connected) return 'success';
  if (failed(now) && !failed(before)) return 'error';
  return null;
}

/**
 * Plays the feedback of each start that `progress` reads (MOT-050, mobile plan M4-4): a success when the steps reach
 * connected, an error when they stop before it, as the failed step shows. It plays in the frame that the steps change
 * (MOT-051). Call it once, while a component starts.
 */
export function playStartFeedback(progress: () => Progress): void {
  let before = progress();
  $effect(() => {
    const now = progress();
    const feel = startFeel(before, now);
    before = now;
    if (feel !== null) playHaptic(feel);
  });
}
