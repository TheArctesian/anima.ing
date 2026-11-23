<script lang="ts">
  import EmptyState from '$lib/states/EmptyState.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import LoadingState from '$lib/states/LoadingState.svelte';
  import Notice from '$lib/states/Notice.svelte';
  import RouteDetails from './RouteDetails.svelte';
  import RouteList from './RouteList.svelte';
  import { NOT_LINUX } from './route';
  import type { RoutesStore } from './store.svelte';

  /**
   * Task H21: the body of the section Routes of Hosting (mockup routes, Frames 1, 1b and 3). The hub is the proxy; each
   * route sends a name to one application on one machine. Add a route is in the page header (`AddRoute`).
   */
  const { store, now }: { store: RoutesStore; now?: () => Date } = $props();
  const shown = $derived(store.shown);
  $effect(() => {
    void store.open();
  });
</script>

<div class="routes">
  {#if store.notLinux}
    <Notice tone="warning" title={NOT_LINUX} />
  {:else if store.status === 'failed'}
    <ErrorState message={store.problem} onRetry={() => void store.load()} />
  {:else if store.status !== 'ready'}
    <LoadingState label="Loading the routes" />
  {:else if shown !== null}
    <RouteDetails {store} route={shown} {now} />
  {:else if store.routes.length === 0}
    <EmptyState
      title="No routes yet"
      line="A route sends a name of yours, for example photos.example.com, to one application on one of your machines. The hub is the proxy and gets the certificate. A new route is private: only your devices on the network of Anima reach it, until you make it public."
    />
    <p class="note">Adding the first route turns on the proxy of the hub. Until then nothing listens for the internet.</p>
  {:else}
    <p class="note">The hub is the proxy: each route sends a name of yours to one application on one machine.</p>
    <RouteList {store} {now} />
    {#if store.hosted}
      <p class="note">Streams of TCP and UDP need a hub that you host: this hub is hosted by Anima.ing, and its front door has no address of its own for this hub yet.</p>
    {/if}
  {/if}
</div>

<style>
  .routes {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding-block: var(--space-3);
    min-inline-size: 0;
  }

  .note {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

</style>
