<script lang="ts">
  import type { Snippet } from 'svelte';
  import '../details.css';

  interface Props {
    /** The name of the two halves as one group: "Open in editor". */
    label: string;
    /** The menu of the second half is open: the row keeps its hover fill. */
    open?: boolean;
    element?: HTMLElement | undefined;
    primary: Snippet;
    secondary: Snippet;
  }

  let { label, open = false, element = $bindable(), primary, secondary }: Props = $props();
</script>

<!-- A split row (07 section 0.2): at rest it looks like one row; the line between its halves shows on hover. -->
<div class="split" class:open role="group" aria-label={label} bind:this={element}>
  {@render primary()}
  <span class="line" aria-hidden="true"></span>
  {@render secondary()}
</div>

<style>
  .split {
    display: flex;
    align-items: center;
    inline-size: 100%;
    border-radius: var(--radius-md);
  }

  .split:hover,
  .split:focus-within,
  .split.open {
    background: var(--surface-hover);
  }

  .line {
    flex: none;
    inline-size: var(--border-thin);
    block-size: var(--icon-sm);
  }

  .split:hover .line,
  .split:focus-within .line,
  .split.open .line {
    background: var(--border-strong);
  }
</style>
