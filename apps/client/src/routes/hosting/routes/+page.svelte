<script lang="ts">
  import { page } from '$app/state';
  import AddRoute from '$lib/application-routes/AddRoute.svelte';
  import { routesApi } from '$lib/application-routes/api';
  import RoutesSection from '$lib/application-routes/RoutesSection.svelte';
  import { RoutesStore } from '$lib/application-routes/store.svelte';
  import { hubIsHosted } from '$lib/hosted/mark';
  import { useHead } from '$lib/hosting/sections/head.svelte';
  import { useHub } from '$lib/hub/context';

  // The routes of the hub (task H21), moved here from Settings (HSE-005, HSE-010); a hosted hub has no streams (OD-HOST-47).
  const hub = useHub();
  const routes = new RoutesStore(routesApi(hub), hubIsHosted(hub.address.origin));
  const head = useHead();
  // HSE-019: the overview opens this section with Needs you chosen, or with one route open.
  $effect(() => {
    if (page.url.searchParams.get('show') === 'needs-you') routes.filter = 'NEEDS_YOU';
    const route = page.url.searchParams.get('route');
    if (route !== null) routes.select(route);
  });
  $effect(() => {
    head.actions = add;
    return () => {
      head.actions = undefined;
    };
  });
</script>

{#snippet add()}<AddRoute />{/snippet}

<RoutesSection store={routes} />
