<script lang="ts">
  import type { Snippet } from 'svelte';
  import PanelEdge from '../PanelEdge.svelte';
  import { EXPLORER_MIN, explorerMax, explorerWidth } from './explorer';

  interface Props {
    /** The width that is asked for; the column holds it to its limits for the row that it is in. */
    width: number;
    onresize: (width: number) => void;
    /** The tree of the files. */
    children: Snippet;
  }

  const { width, onresize, children }: Props = $props();
  let column = $state<HTMLElement>();
  let row = $state(Number.POSITIVE_INFINITY);
  const held = $derived(explorerWidth(width, row));

  // The limits follow the row: the content of the file beside the column always keeps its 256px.
  $effect(() => {
    const parent = column?.parentElement;
    if (!parent || typeof ResizeObserver !== 'function') return;
    const measure = (): void => {
      const measured = parent.getBoundingClientRect().width;
      if (measured > 0) row = measured;
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(parent);
    return () => {
      observer.disconnect();
    };
  });
</script>

<!--
  The file explorer beside an open file (report 05, Section 2.9): a column at the right with the tree, 256px at
  first. Its left edge is the edge of the panel with its own name and limits: a pull or the arrow keys change the width.
-->
<aside class="explorer" aria-label="File explorer" bind:this={column} style:inline-size="{held}px">
  <PanelEdge width={held} max={explorerMax(row)} min={EXPLORER_MIN} label="Resize file explorer" {onresize} />
  {@render children()}
</aside>

<style>
  .explorer {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    min-block-size: 0;
    border-inline-start: var(--border-thin) solid var(--code-line);
  }
</style>
