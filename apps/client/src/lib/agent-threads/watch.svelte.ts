import { desktopRunnerShell } from '$lib/desktop/agent-threads/runner';
import { desktopThreadsShell } from '$lib/desktop/agent-threads/shell';
import { daemonUpdateWords } from '$lib/desktop/daemon/update-notice';
import { LiveThreads } from './live/live.svelte';
import { ThreadsStore } from './store.svelte';
import { Freshness, type Visibility } from './freshness';

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

const subscriptions = new WeakMap<ThreadsStore, { users: number; stop: () => void }>();

export function keepFresh(store: ThreadsStore, target: Visibility = document): () => void {
  let subscription = subscriptions.get(store);
  if (subscription === undefined) {
    const freshness = new Freshness(store, target);
    subscription = { users: 0, stop: () => freshness.stop() };
    subscriptions.set(store, subscription);
  }
  subscription.users++;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    if (--subscription.users === 0) {
      subscription.stop();
      subscriptions.delete(store);
    }
  };
}
