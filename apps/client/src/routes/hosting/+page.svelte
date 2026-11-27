<script lang="ts">
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import { routesApi } from '$lib/application-routes/api';
  import { RoutesStore } from '$lib/application-routes/store.svelte';
  import { catalogueApi } from '$lib/catalogue/api';
  import CataloguePicker from '$lib/catalogue/CataloguePicker.svelte';
  import { CatalogueStore } from '$lib/catalogue/store.svelte';
  import { hostingApi } from '$lib/hosting/api';
  import HostingList from '$lib/hosting/HostingList.svelte';
  import { machineImages } from '$lib/hosting/list';
  import { useHead } from '$lib/hosting/sections/head.svelte';
  import HostingOverview from '$lib/hosting/overview/HostingOverview.svelte';
  import { HostingStore } from '$lib/hosting/store.svelte';
  import { hubIsHosted } from '$lib/hosted/mark';
  import { useHub } from '$lib/hub/context';

  // Applications (HSE-002): the one list of applications and containers of all machines (task H42, stage 1, read only).
  const hub = useHub();
  const store = new HostingStore(hostingApi(hub));
  // HSE-022: Install from the catalogue on a machine opens the Catalogue as a picker with that machine chosen.
  const catalogue = new CatalogueStore(catalogueApi(hub));
  let picking = $state(false);
  let machine = $state('');
  // HSE-019: the overview at the top counts the routes too, from a read of its own.
  const routes = new RoutesStore(routesApi(hub), hubIsHosted(hub.address.origin));
  /** A line of the overview shows its row: the list narrows to its machine and to what needs you, and the row opens. */
  function show(id: string, row: string): void {
    store.machine = id;
    store.view = 'applications';
    store.search = '';
    store.show = 'needs-you';
    if (!store.open.includes(row)) store.toggle(row);
  }
  const head = useHead();
  $effect(() => {
    store.show = page.url.searchParams.get('show') === 'needs-you' ? 'needs-you' : 'all';
  });
  $effect(() => {
    void store.load();
    void routes.load();
    head.actions = install;
    return () => {
      head.actions = undefined;
    };
  });
</script>

<!-- The catalogue is the page below Applications (HSE-004); an install from it waits for task H6. -->
{#snippet install()}<a class="install" href={resolve('/hosting/catalogue')}>Install from the catalogue</a>{/snippet}

<HostingOverview hosting={store} {routes} now={new Date()} onshow={show} />
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
