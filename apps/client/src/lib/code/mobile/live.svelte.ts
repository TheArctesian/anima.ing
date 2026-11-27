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

const SUMMARY_REFRESH_MS = 30_000;

export function watchSummary(refresh: () => void): () => void {
  const timer = setInterval(() => {
    if (document.visibilityState === 'visible') refresh();
  }, SUMMARY_REFRESH_MS);
  const removeListeners = onReturn(refresh);
  return () => {
    clearInterval(timer);
    removeListeners();
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
    unsubscribe = api.follow(id, () => cursor, {
      next: (entry) => {
        if (Number(entry.sequence) <= Number(cursor)) return;
        show(entry);
        cursor = entry.sequence;
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
