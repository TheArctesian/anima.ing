<script lang="ts">
  import { resolve } from '$app/paths';
  import { hostingApi } from '$lib/hosting/api';
  import HostingList from '$lib/hosting/HostingList.svelte';
  import { useHead } from '$lib/hosting/sections/head.svelte';
  import { HostingStore } from '$lib/hosting/store.svelte';
  import { useHub } from '$lib/hub/context';

  // Applications (HSE-002): the one list of applications and containers of all machines (task H42, stage 1, read only).
  const store = new HostingStore(hostingApi(useHub()));
  const head = useHead();
  $effect(() => {
    void store.load();
    head.actions = install;
    return () => {
      head.actions = undefined;
    };
  });
</script>

<!-- The catalogue is the page below Applications (HSE-004); an install from it waits for task H6. -->
{#snippet install()}<a class="install" href={resolve('/hosting/catalogue')}>Install from the catalogue</a>{/snippet}

<HostingList {store} />

<style>
  .install {
    display: inline-flex;
    align-items: center;
    min-block-size: var(--target-min-touch);
    color: var(--accent);
  }
</style>
