<script lang="ts">
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import FitBadge from './FitBadge.svelte';
  import { capabilityWords, factsLine, markOf, runs } from './fit';
  import type { LibraryStore } from './store.svelte';

  /**
   * The models of the library (SEL-005), as the model lists of Unsloth Studio and LM Studio show them: the mark of the
   * publisher, the name, the parameters and the context, the capability tags (LIBM-008), and the best fit of any variant
   * on any machine (FIT-005). Frame 9 of the hosting mockup.
   */
  const { store }: { store: LibraryStore } = $props();
  const fitting = $derived(store.models.filter((model) => store.fitsOf(model.name).some(runs)).length);
  let filter = $state<'all' | 'fits'>('all');
</script>

<section class="pane" aria-label="The models of the library">
  <div class="tools">
    <Segmented
      label="Show"
      size="sm"
      options={[{ value: 'all', label: 'All', count: store.models.length }, { value: 'fits', label: 'Fits a machine', count: fitting }]}
      bind:value={filter}
      onchange={(value) => { store.onlyFitting = value === 'fits'; }}
    />
  </div>
  <ul>
    {#each store.shown as model (model.name)}
      <li>
        <button type="button" class:on={model.name === store.selected} aria-current={model.name === store.selected || undefined} onclick={() => { store.select(model.name); }}>
          <span class="mark" aria-hidden="true">{markOf(model)}</span>
          <span class="name">{model.name}</span>
          <FitBadge fit={store.best(model.name)} />
          <span class="facts">{factsLine(model)}</span>
          {#if model.capabilities.length > 0}
            <span class="chips">{#each model.capabilities as capability (capability)}<span class="chip">{capabilityWords(capability)}</span>{/each}</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--surface-raised);
  }

  .tools {
    padding: var(--space-3) var(--space-4);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    overflow-y: auto;
  }

  button {
    display: grid;
    grid-template-columns: var(--control-md) minmax(0, 1fr) auto;
    gap: var(--space-1) var(--space-3);
    align-items: center;
    inline-size: 100%;
    padding: var(--space-3) var(--space-4);
    border: 0;
    border-block-start: var(--border-thin) solid var(--border-subtle);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    text-align: start;
    cursor: pointer;
  }

  button:hover {
    background: var(--surface-hover);
  }

  .on,
  .on:hover {
    background: var(--surface-selected);
  }

  .mark {
    display: grid;
    grid-row: span 3;
    place-items: center;
    align-self: start;
    inline-size: var(--control-md);
    block-size: var(--control-md);
    border-radius: var(--radius-md);
    background: var(--accent-tint);
    color: var(--accent);
    font-weight: var(--weight-medium);
  }

  .name {
    overflow: hidden;
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .facts,
  .chips {
    grid-column: 2 / -1;
  }

  .facts {
    color: var(--text-secondary);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .chip {
    padding-inline: var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--surface-base);
    color: var(--text-secondary);
    font-size: var(--text-xs);
  }
</style>
