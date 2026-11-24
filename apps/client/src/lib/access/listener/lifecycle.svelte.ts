import { untrack } from 'svelte';
import type { Hub } from '$lib/hub/hub';
import { forgetListener } from './check';

/** A disconnected port can return as a different program; its earlier ownership proof is stale. */
export function watchAccessConnection(hub: Hub): void {
  $effect(() => {
    if (hub.status.current.connection !== 'connected') untrack(() => forgetListener(hub.address.origin));
  });
  $effect(() => () => {
    hub.close().catch((reason: unknown) => {
      console.warn('Hub connection attempt failed while closing:', reason instanceof Error ? reason.name : 'unknown');
    });
  });
}
