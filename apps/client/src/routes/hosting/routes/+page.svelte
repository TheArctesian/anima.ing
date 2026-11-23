<script lang="ts">
  import { routesApi } from '$lib/application-routes/api';
  import RoutesSection from '$lib/application-routes/RoutesSection.svelte';
  import { RoutesStore } from '$lib/application-routes/store.svelte';
  import { hubIsHosted } from '$lib/hosted/mark';
  import { useHub } from '$lib/hub/context';

  // The routes of the hub (task H21), moved here from Settings (HSE-005, HSE-010); a hosted hub has no streams (OD-HOST-47).
  const hub = useHub();
  const routes = new RoutesStore(routesApi(hub), hubIsHosted(hub.address.origin));
</script>

<RoutesSection store={routes} />
