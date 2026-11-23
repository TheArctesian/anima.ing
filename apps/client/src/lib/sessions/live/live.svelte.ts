import type { Hub } from '$lib/hub/hub';
import { liveSessionsApi, type LiveSession } from './api';
import { InFlight } from './inflight';
import { applyChange, newestFirst, withoutSession } from './list';
import { Stopping } from './stopping.svelte';

const LIVE_LIMIT = 200;
const NOT_LOADED = 'Could not load live sessions.';
const LIVE_STOPPED = 'Live updates stopped. Reopen to refresh.';

/** The live sessions of the nav bar (RA-034, D-6). */
export function watchLiveSessions(hub: Hub) {
  const api = liveSessionsApi(hub);
  let sessions = $state<readonly LiveSession[]>([]);
  let loadFailure = $state<string | null>(null);
  let updatesStopped = $state<string | null>(null);
  const stopping = new Stopping();

  const inFlight = new InFlight();

  async function refresh(): Promise<void> {
    const mine = inFlight.begin();
    const result = await api.list(LIVE_LIMIT);
    if (!inFlight.isNewest(mine)) return;
    loadFailure = result.ok ? null : NOT_LOADED;
    if (result.ok) sessions = newestFirst(inFlight.catchUp(result.value));
  }

  async function stop(id: string): Promise<void> {
    if (!stopping.begin(id)) return;
    const result = await api.end(id);
    stopping.end(id, result.ok);
    if (result.ok) {
      inFlight.stopped(id);
      sessions = withoutSession(sessions, id);
    }
  }

  function start(): () => void {
    void refresh();
    return api.watch({
      next: (change) => {
        updatesStopped = null;
        inFlight.pushed(change);
        sessions = applyChange(sessions, change);
      },
      error: () => {
        updatesStopped = LIVE_STOPPED;
      },
      complete: () => undefined,
    });
  }

  return {
    get sessions() {
      return sessions;
    },
    get loadFailure() {
      return loadFailure;
    },
    get updatesStopped() {
      return updatesStopped;
    },
    stopFailure: stopping.failure,
    isStopping: stopping.isStopping,
    refresh,
    stop,
    start,
  };
}
