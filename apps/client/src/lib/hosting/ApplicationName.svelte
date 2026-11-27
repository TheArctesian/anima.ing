<script lang="ts">
  import BrandMark from '$lib/brands/BrandMark.svelte';
  import type { ApplicationRow } from './list';

  /**
   * The first cell of a row of Applications (HSE-002): the mark of its program (HSE-020), its name, which opens an
   * application of several containers, Not managed, and what it is.
   */
  const { row, open, ontoggle }: { row: ApplicationRow; open: boolean; ontoggle: () => void } = $props();
  const opens = $derived(row.containers.length > 1 || row.what === 'Compose project');
  const containers = $derived(`${String(row.containers.length)} container${row.containers.length === 1 ? '' : 's'}`);
  const what = $derived(row.what === 'Compose project' ? `Compose project · ${containers}` : row.what);
</script>

<span class="who">
  <BrandMark mark={row.mark} size="sm" />
  <span>
    {#if opens}
      <button type="button" class="open" aria-expanded={open} onclick={ontoggle}><span aria-hidden="true">{open ? '▾' : '▸'}</span> {row.name}</button>
    {:else}<span class="lone">{row.name}</span>{/if}
    <span class="mark">{row.managedApplicationId === null ? 'Not managed' : 'Managed'}</span> <span class="sub">{what}</span>
  </span>
</span>

<style>
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .open,
  .lone {
    padding: 0;
    background: none;
    border: 0;
    color: var(--text);
    font: inherit;
    font-weight: var(--weight-medium);
    cursor: pointer;
  }

  .lone {
    cursor: auto;
  }

  .sub {
    display: block;
    color: var(--text-secondary);
  }

  /* The mark is a word with a border that repeats it (MAT-005). */
  .mark {
    margin-inline-start: var(--space-2);
    padding: 0 var(--space-2);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-pill);
    color: var(--text-secondary);
  }
</style>
