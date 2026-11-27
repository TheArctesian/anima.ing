<script lang="ts">
  import { catalogueApi } from '$lib/catalogue/api';
  import CatalogueView from '$lib/catalogue/CatalogueView.svelte';
  import { CatalogueStore } from '$lib/catalogue/store.svelte';
  import { hostingApi } from '$lib/hosting/api';
  import { machineImages } from '$lib/hosting/list';
  import { HostingStore } from '$lib/hosting/store.svelte';
  import { useHub } from '$lib/hub/context';

  // The catalogue that ships with the hub (tasks H38, H39 and H55): its rows say on which machines each program runs, from
  // the containers that the hub reads on each machine (HSE-022).
  const hub = useHub();
  const store = new CatalogueStore(catalogueApi(hub));
  const machines = new HostingStore(hostingApi(hub));
  $effect(() => {
    void store.open();
    void machines.load();
  });
</script>

<CatalogueView {store} machines={machineImages(machines.places)} />
