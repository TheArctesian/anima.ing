<script lang="ts">
  import type { Snippet } from 'svelte';
  import { edgeFade } from '$lib/scroll/edges';

  interface Props {
    /** The region's name, so the keyboard can reach and scroll it: "Chats". */
    label: string;
    children: Snippet;
    /** `hidden` draws no bar at all, as the inbox list does; `hover` shows a 6 px bar while the pointer is over it. */
    bar?: 'hover' | 'hidden';
  }

  const { label, children, bar = 'hover' }: Props = $props();
</script>

<!-- A scroll area (02 section 3.7): the edges with more content fade, and the thin bar shows on hover only. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="area {bar}" role="region" aria-label={label} tabindex="0" data-own-keys {@attach edgeFade}>
  {@render children()}
</div>

<style>
  .area {
    --fade: var(--space-6);

    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
  }

  .hover:hover,
  .hover:focus-visible {
    scrollbar-color: var(--border-strong) transparent;
  }

  .hidden {
    --fade: var(--space-3);

    scrollbar-width: none;
  }

  .area::-webkit-scrollbar {
    inline-size: var(--code-scroll-thumb);
  }

  .hidden::-webkit-scrollbar {
    display: none;
  }

  .area::-webkit-scrollbar-thumb {
    border-radius: var(--radius-pill);
    background: transparent;
  }

  .hover:hover::-webkit-scrollbar-thumb {
    background: var(--border-strong);
  }

  .area:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .area:global([data-more-above]) {
    mask-image: linear-gradient(to bottom, transparent, var(--text) var(--fade));
  }

  .area:global([data-more-below]) {
    mask-image: linear-gradient(to top, transparent, var(--text) var(--fade));
  }

  .area:global([data-more-above][data-more-below]) {
    mask-image: linear-gradient(to bottom, transparent, var(--text) var(--fade), var(--text) calc(100% - var(--fade)), transparent);
  }
</style>
