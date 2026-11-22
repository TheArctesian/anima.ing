<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import EmptyState from '$lib/states/EmptyState.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import LoadingState from '$lib/states/LoadingState.svelte';
  import Notice from '$lib/states/Notice.svelte';
  import RouteDetails from './RouteDetails.svelte';
  import RouteList from './RouteList.svelte';
  import { NOT_LINUX } from './route';
  import type { RoutesStore } from './store.svelte';

  /**
   * Task H21: the section Routes of Settings, in the group Hub and account (mockup routes, Frames 1, 1b and 3). The hub
   * is the proxy; each route sends a name to one application on one machine. Adding a route needs the hub's record of
   * the applications and their ports (task H6), so Add a route is off with that reason.
   */
  const { store, now }: { store: RoutesStore; now?: () => Date } = $props();
  const id = $props.id();
  const shown = $derived(store.shown);
  $effect(() => {
    void store.open();
  });
</script>

{#snippet add()}
  <div class="add">
    <FormButton tone="primary" icon="carbon:add" aria-disabled="true" aria-describedby="{id}-add">Add a route</FormButton>
    <p class="why" id="{id}-add">Not built: a route goes to an application that the hub has recorded with its ports, and the hub keeps no such record yet (task H6).</p>
  </div>
{/snippet}

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
      action={add}
    />
    <p class="note">Adding the first route turns on the proxy of the hub. Until then nothing listens for the internet.</p>
  {:else}
    <div class="head">
      <p class="note">The hub is the proxy: each route sends a name of yours to one application on one machine.</p>
      {@render add()}
    </div>
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

  .head {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: var(--space-4);
  }

  .add {
    display: flex;
    flex-direction: column;
    align-items: end;
    gap: var(--space-1);
    max-inline-size: calc(var(--space-16) * 4);
  }

  .note,
  .why {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .why {
    text-align: end;
  }
</style>
