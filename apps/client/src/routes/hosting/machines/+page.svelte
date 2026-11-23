<script lang="ts">
  import { hostingApi } from '$lib/hosting/api';
  import MachineList from '$lib/hosting/machines/MachineList.svelte';
  import { MachineStore } from '$lib/hosting/machines/store.svelte';
  import { useHub } from '$lib/hub/context';

  // Machines (HSE-016): the machines that can host, with their status, read only (stage 1).
  const store = new MachineStore(hostingApi(useHub()));
  $effect(() => {
    void store.load();
  });
</script>

<MachineList {store} />
