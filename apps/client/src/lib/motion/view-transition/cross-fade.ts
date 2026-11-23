import './cross-fade.css';

/** WebKit before Safari 18 and jsdom have no view transitions. */
interface ViewTransitions {
  readonly startViewTransition?: (update: () => void) => Partial<Pick<ViewTransition, 'ready' | 'finished' | 'updateCallbackDone'>> | undefined;
}

/** The same under reduced motion, because only opacity animates (MOT-006, MOT-040); nothing waits for the fade (MOT-042). */
export function crossFadeView(change: () => void, view: ViewTransitions = document): void {
  if (typeof view.startViewTransition !== 'function') {
    change();
    return;
  }
  // MOT-010: the whole page is captured; F-5 keeps session colours.
  const transition = view.startViewTransition(change);
  // A skipped transition (a second change, a hidden page) is not an error, and `change` has run: mark them handled.
  for (const outcome of [transition?.ready, transition?.finished, transition?.updateCallbackDone]) {
    outcome?.catch(() => undefined);
  }
}
