<script lang="ts">
  import { catalogueApi } from '$lib/catalogue/api';
  import CatalogueView from '$lib/catalogue/CatalogueView.svelte';
  import { CatalogueStore } from '$lib/catalogue/store.svelte';
  import { useHub } from '$lib/hub/context';

  // The catalogue that ships with the hub (tasks H38 and H39); its gallery of HSE-004 belongs to the hosted applications plan.
  const store = new CatalogueStore(catalogueApi(useHub()));
  $effect(() => {
    void store.open();
  });
</script>

<CatalogueView {store} />
