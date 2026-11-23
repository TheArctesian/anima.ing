import type { LoadingClock } from '$lib/motion/loading/loading';
import { threshold } from '$lib/motion/tokens';

export type ActionState = 'idle' | 'pending' | 'done';

export interface ActionFeedback {
  begin(): void;
  succeed(): void;
  fail(): void;
  stop(): void;
}

/** Quick work shows no pending label, so it never flashes (spec 5.7.3, Feedback). */
export function actionFeedback(clock: LoadingClock, holdMs: number, onChange: (state: ActionState) => void): ActionFeedback {
  let cancel: (() => void) | undefined;
  const clear = () => {
    cancel?.();
    cancel = undefined;
  };
  const settle = (state: ActionState) => {
    clear();
    onChange(state);
  };

  return {
    begin: () => {
      clear();
      cancel = clock.after(threshold.loading, () => {
        cancel = undefined;
        onChange('pending');
      });
    },
    succeed: () => {
      settle('done');
      cancel = clock.after(holdMs, () => {
        cancel = undefined;
        onChange('idle');
      });
    },
    fail: () => {
      settle('idle');
    },
    stop: clear,
  };
}
