import { threshold } from '$lib/motion/tokens';

/** The clock the loading thresholds run on; a test passes its own. These waits are not animation timers (MOT-042). */
export interface LoadingClock {
  /** Milliseconds from any fixed origin. */
  now(): number;
  /** Calls `run` once after `ms`; returns the function that cancels it. */
  after(ms: number, run: () => void): () => void;
}

export const browserClock: LoadingClock = {
  now: () => performance.now(),
  after: (ms, run) => {
    const timer = setTimeout(run, ms);
    return () => {
      clearTimeout(timer);
    };
  },
};

export interface SkeletonGate {
  begin(): void;
  /** The load finished; the skeleton leaves once it has been shown for `threshold-loading-min`. */
  end(): void;
  /** Leave at once, for a page that is going away. */
  stop(): void;
}

/** The skeleton shows only after `threshold-loading` and then stays at least `threshold-loading-min`, so it never flashes (5.7.3 Loading). */
export function skeletonGate(clock: LoadingClock, onChange: (visible: boolean) => void): SkeletonGate {
  let pending: (() => void) | undefined;
  let shownAt: number | undefined;

  const clearPending = () => {
    pending?.();
    pending = undefined;
  };
  const hide = () => {
    pending = undefined;
    shownAt = undefined;
    onChange(false);
  };

  return {
    begin: () => {
      clearPending();
      if (shownAt !== undefined) return;
      pending = clock.after(threshold.loading, () => {
        pending = undefined;
        shownAt = clock.now();
        onChange(true);
      });
    },
    end: () => {
      clearPending();
      if (shownAt === undefined) return;
      const remaining = threshold.loadingMin - (clock.now() - shownAt);
      if (remaining <= 0) hide();
      else pending = clock.after(remaining, hide);
    },
    stop: () => {
      clearPending();
      if (shownAt !== undefined) hide();
    },
  };
}
