<script lang="ts">
  import { useHead } from '$lib/hosting/sections/head.svelte';
  import { useHub } from '$lib/hub/context';
  import { copiesApi } from '$lib/model-copies/api';
  import CopyList from '$lib/model-copies/CopyList.svelte';
  import FoundList from '$lib/model-copies/FoundList.svelte';
  import PullCopy from '$lib/model-copies/PullCopy.svelte';
  import { CopiesStore } from '$lib/model-copies/store.svelte';

  // Models (HSE-013): the copies of the models on the machines (task L1-7 of compute-plan.md), one group for each machine,
  // then the models that the machines found where a different program keeps them (task L1-22, PULL-012).
  const store = new CopiesStore(copiesApi(useHub()));
  const head = useHead();
  // While a copy waits, downloads or is removed, the list reads again every few seconds, quietly (PULL-007).
  const EVERY_MS = 5000;
  // After Find models the machines answer when they next ask for work: the list reads again for a minute.
  const FINDING_READS = 12;

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

  $effect(() => {
    if (!store.finding) return;
    let reads = 0;
    const timer = setInterval(() => {
      reads += 1;
      void store.load(true);
      if (reads >= FINDING_READS) store.finding = false;
    }, EVERY_MS);
    return () => {
      clearInterval(timer);
    };
  });
</script>

{#snippet pull()}<PullCopy {store} />{/snippet}

<CopyList {store} />
<FoundList {store} />
