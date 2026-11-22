<script lang="ts">
  import { capabilities } from '$lib/platform/platform';
  import WindowControls from './WindowControls.svelte';

  interface Props {
    /** The page has no navigation bar (the page before sign-in), so the strip spans the window. */
    bare?: boolean;
  }

  const { bare = false }: Props = $props();
</script>

<!--
  The top of a window that the system gives no frame (Windows and Linux; task TC2): the strip moves the window and
  holds the window controls at its end. The root layout mounts it on each page; where the system draws the top of the
  window it is nothing.
-->
{#if capabilities().framelessWindow}
  <div class="strip" class:bare data-tauri-drag-region><WindowControls /></div>
{/if}

<style>
  /* The strip is opaque, and the page and each pinned bar start below it (--titlebar-block-size in global.css), so it
     covers no control. It stands over a modal too (the layer of a toast): a window must close and move then. */
  .strip {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    position: fixed;
    inset-block-start: 0;
    inset-inline: var(--nav-inline-size) 0;
    block-size: var(--control-md);
    padding-inline-end: var(--space-2);
    background: var(--surface-sunken);
    z-index: var(--layer-toast);
  }

  .bare {
    inset-inline-start: 0;
  }

  /* The top band of the page Code is in the place of the strip and moves the window itself. There the strip is only
     the room of the window controls, as high as the band (52px), and a press beside them goes to the band. */
  :global(:root:has([data-code-band])) .strip {
    block-size: calc(var(--space-12) + var(--space-1));
    background: none;
    pointer-events: none;
  }

  :global(:root:has([data-code-band])) .strip > :global(*) {
    pointer-events: auto;
  }

  /* When the hub cannot be reached below the compact breakpoint, its strip is the top of the window and this one
     stands below it. */
  :global(:root[data-nav='icons'][data-hub-banner]) .strip {
    inset-block-start: var(--nav-banner-block-size);
  }

  /* The page starts below the strip. These are the two frames of the root layout (routes/+layout.svelte). The page
     before sign-in fills the window: it gives the strip its height and keeps its own without a scroll. */
  :global(:root[data-titlebar='frameless'] .shell > main) {
    padding-block-start: calc(var(--space-4) + var(--titlebar-block-size));
  }

  :global(:root[data-titlebar='frameless'] .bare) {
    --viewport-block-size: calc(100vh - var(--safe-block-start) - var(--safe-block-end) - var(--titlebar-block-size));

    padding-block-start: var(--titlebar-block-size);
  }
</style>
