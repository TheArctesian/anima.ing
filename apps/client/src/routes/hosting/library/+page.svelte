<script lang="ts">
  import { useHub } from '$lib/hub/context';
  import { libraryApi } from '$lib/model-library/api';
  import LibraryView from '$lib/model-library/LibraryView.svelte';
  import { LibraryStore } from '$lib/model-library/store.svelte';

  // The Library below Models (task L1-20 of compute-plan.md): the models of the library and the fit of each variant on
  // each machine (FIT-001 to FIT-005), as Frame 9 of the hosting mockup draws it.
  const store = new LibraryStore(libraryApi(useHub()));

  $effect(() => {
    void store.load();
  });
</script>

<LibraryView {store} />
