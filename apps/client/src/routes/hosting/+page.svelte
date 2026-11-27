<script lang="ts">
  import { resolve } from '$app/paths';
  import { catalogueApi } from '$lib/catalogue/api';
  import CataloguePicker from '$lib/catalogue/CataloguePicker.svelte';
  import { CatalogueStore } from '$lib/catalogue/store.svelte';
  import { hostingApi } from '$lib/hosting/api';
  import HostingList from '$lib/hosting/HostingList.svelte';
  import { machineImages } from '$lib/hosting/list';
  import { useHead } from '$lib/hosting/sections/head.svelte';
  import { HostingStore } from '$lib/hosting/store.svelte';
  import { useHub } from '$lib/hub/context';

  // Applications (HSE-002): the one list of applications and containers of all machines (task H42, stage 1, read only).
  const hub = useHub();
  const store = new HostingStore(hostingApi(hub));
  // HSE-022: Install from the catalogue on a machine opens the Catalogue as a picker with that machine chosen.
  const catalogue = new CatalogueStore(catalogueApi(hub));
  let picking = $state(false);
  let machine = $state('');
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

<HostingList {store} oninstall={(id) => { machine = id; picking = true; }} />
<CataloguePicker store={catalogue} machines={machineImages(store.places)} bind:machine open={picking} onClose={() => { picking = false; }} />

<style>
  .install {
    display: inline-flex;
    align-items: center;
    min-block-size: var(--target-min-touch);
    color: var(--accent);
  }
</style>
