import type { MobileCodeApi, ThreadEntry } from './api';

export function onReturn(refresh: () => void): () => void {
  const visible = (): void => {
    if (document.visibilityState === 'visible') refresh();
  };
  document.addEventListener('visibilitychange', visible);
  window.addEventListener('online', visible);
  return () => {
    document.removeEventListener('visibilitychange', visible);
    window.removeEventListener('online', visible);
  };
}

export function followThread(
  api: MobileCodeApi,
  id: string,
  afterSequence: string,
  show: (entry: ThreadEntry) => void,
) {
  let cursor = afterSequence;
  let unsubscribe: (() => void) | undefined;
  let error = $state('');
  const resume = (): void => {
    unsubscribe?.();
    error = '';
    unsubscribe = api.follow(id, cursor, {
      next: (entry) => {
        // The shared socket can replay from the subscription's initial position after a drop.
        if (Number(entry.sequence) <= Number(cursor)) return;
        cursor = entry.sequence;
        show(entry);
      },
      error: (reason) => { error = reason.message; },
      complete: () => { error = 'The thread stopped updating.'; },
    });
  };
  resume();
  const removeListeners = onReturn(resume);
  return {
    get error() { return error; },
    resume,
    stop() {
      removeListeners();
      unsubscribe?.();
    },
  };
}
