import { createContext } from 'svelte';
import type { Hub } from '$lib/hub/hub';
import { releasePublishing } from './api';

/** A hub without the release bucket and the updater key refuses a release, so the client shows no Builds item (PRO-037, PRO-038). */
export class ReleasePublishing {
  on = $state(false);

  constructor(private readonly hub: Pick<Hub, 'request'>) {}

  async load(): Promise<void> {
    this.on = await releasePublishing(this.hub);
  }
}

const [getPublishing, setPublishing] = createContext<ReleasePublishing>();

export function providePublishing(publishing: ReleasePublishing): void {
  setPublishing(publishing);
}

export function usePublishing(): ReleasePublishing {
  return getPublishing();
}
