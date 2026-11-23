<script lang="ts">
  import EmptyState from '$lib/states/EmptyState.svelte';
  import ErrorState from '$lib/states/ErrorState.svelte';
  import LoadingState from '$lib/states/LoadingState.svelte';
  import ModelDetail from './ModelDetail.svelte';
  import ModelList from './ModelList.svelte';
  import type { LibraryStore } from './store.svelte';

  /** The Library of Hosting (task L1-20): the list of the models beside the selected model (Frame 9 of the hosting mockup). */
  const { store }: { store: LibraryStore } = $props();
</script>

{#if store.status === 'failed'}
  <ErrorState message={store.problem} onRetry={() => void store.load()} />
{:else if store.status !== 'ready' && store.models.length === 0}
  <LoadingState label="Reading the library" />
{:else if store.models.length === 0}
  <EmptyState title="The library has no model" line="Add one from Hugging Face with anima model add and the name of its repository." />
{:else}
  <div class="split">
    <ModelList {store} />
    {#if store.model !== undefined}<ModelDetail {store} model={store.model} />{/if}
  </div>
{/if}

<style>
  .split {
    display: grid;
    grid-template-columns: calc(var(--space-16) * 4 + var(--space-10)) minmax(0, 1fr);
    gap: var(--space-4);
    align-items: start;
  }

  @media (max-width: 900.98px) {
    .split {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
