import { desktopRunnerShell } from '$lib/desktop/agent-threads/runner';
import { desktopThreadsShell } from '$lib/desktop/agent-threads/shell';
import { daemonUpdateWords } from '$lib/desktop/daemon/update-notice';
import { LiveThreads } from './live/live.svelte';
import { ThreadsStore } from './store.svelte';

/** THR-058: the desktop shell has no watcher of the session files, so the list is read at an interval. */
export const REFRESH_MS = 30_000;

let shared: ThreadsStore | undefined;

/** The one store of the threads of this computer: the page and the count on the bar read the same rows. */
export function threadsStore(): ThreadsStore {
  if (shared !== undefined) return shared;
  const live = desktopRunnerShell === null ? null : new LiveThreads(desktopRunnerShell, () => ({ front: document.visibilityState === 'visible' && document.hasFocus(), chosen: shared?.chosen ?? null }));
  shared = new ThreadsStore(desktopThreadsShell, typeof localStorage === 'undefined' ? null : localStorage, undefined, live);
  if (live !== null) {
    const store = shared;
    live.seenOpen = (id) => {
      store.markSeen(id);
    };
    live.daemonWords = daemonUpdateWords;
    void live.begin();
  }
  return shared;
}

export function keepFresh(store: ThreadsStore, target: Pick<Document, 'visibilityState' | 'addEventListener' | 'removeEventListener'> = document): () => void {
  const read = (): void => {
    if (target.visibilityState === 'visible') void store.loadList();
  };
  read();
  const timer = setInterval(read, REFRESH_MS);
  target.addEventListener('visibilitychange', read);
  return () => {
    clearInterval(timer);
    target.removeEventListener('visibilitychange', read);
  };
}
