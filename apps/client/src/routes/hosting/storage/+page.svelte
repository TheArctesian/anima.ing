<script lang="ts">
  import { hostingApi } from '$lib/hosting/api';
  import { VolumeStore } from '$lib/hosting/storage/store.svelte';
  import VolumeList from '$lib/hosting/storage/VolumeList.svelte';
  import { useHub } from '$lib/hub/context';

  // Storage (HSE-015): the volumes of all machines, one group for each machine, read only (stage 1).
  const store = new VolumeStore(hostingApi(useHub()));
  $effect(() => {
    void store.load();
  });
</script>

<VolumeList {store} />
