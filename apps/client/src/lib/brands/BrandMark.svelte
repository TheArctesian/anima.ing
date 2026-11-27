<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { BrandMark } from './mark';

  /**
   * The tile of a program in Hosting (HSE-020): its brand mark, else its first letter, in the accent colour, as the
   * Library of the models draws a publisher. The name beside it says what it is, so the tile is hidden from readers.
   */
  const { mark, size = 'md' }: { mark: BrandMark; size?: 'sm' | 'md' } = $props();
</script>

<span class="tile {size}" aria-hidden="true">{#if 'icon' in mark}<Icon name={mark.icon} size={size} />{:else}{mark.letter}{/if}</span>

<style>
  .tile {
    display: inline-grid;
    flex: none;
    place-items: center;
    border-radius: var(--radius-md);
    background: var(--accent-tint);
    color: var(--accent);
    font-weight: var(--weight-medium);
    line-height: 1;
  }

  .md {
    inline-size: var(--control-md);
    block-size: var(--control-md);
  }

  .sm {
    inline-size: var(--control-sm);
    block-size: var(--control-sm);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
  }
</style>
