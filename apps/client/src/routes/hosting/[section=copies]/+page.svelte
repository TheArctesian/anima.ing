<script lang="ts">
  import { useHead } from '$lib/hosting/sections/head.svelte';
  import { useHub } from '$lib/hub/context';
  import { copiesApi } from '$lib/model-copies/api';
  import CopyList from '$lib/model-copies/CopyList.svelte';
  import PullCopy from '$lib/model-copies/PullCopy.svelte';
  import { CopiesStore } from '$lib/model-copies/store.svelte';

  // Models (HSE-013): the copies of the models on the machines (task L1-7 of compute-plan.md), one group for each machine.
  const store = new CopiesStore(copiesApi(useHub()));
  const head = useHead();
  // While a copy waits, downloads or is removed, the list reads again every few seconds, quietly (PULL-007).
  const EVERY_MS = 5000;

  $effect(() => {
    void store.load();
    head.actions = pull;
    return () => {
      head.actions = undefined;
    };
  });

  $effect(() => {
    if (!store.moving) return;
    const timer = setInterval(() => void store.load(true), EVERY_MS);
    return () => {
      clearInterval(timer);
    };
  });
</script>

{#snippet pull()}<PullCopy {store} />{/snippet}

<CopyList {store} />
