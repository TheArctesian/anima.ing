<script lang="ts">
  import type { Snippet } from 'svelte';
  import { capabilities } from '$lib/platform/platform';

  interface Props {
    /** What stands at the start of the band, for example the button of the sidebar. */
    start?: Snippet;
    /** The middle of the band: the path "project / title" of a thread. */
    children?: Snippet;
    /** What stands at the end, before the window controls: the buttons of the panels. */
    end?: Snippet;
    /**
     * This band is the last one of its row. In a window with no frame the root layout draws the window controls over
     * its end, so the band keeps that room free.
     */
    clearsWindowControls?: boolean;
    /** This band is the first one of its row, so on a Mac it leaves room for the three window buttons. */
    clearsWindowButtons?: boolean;
  }

  const { start, children, end, clearsWindowControls = false, clearsWindowButtons = false }: Props = $props();
  const frameless = capabilities().framelessWindow;
  const overlay = capabilities().overlayTitleBar;
</script>

<!--
  The top band of a column of Code: 52px, as in T3 Code (report 02, Section 4.2). The band moves the window, also by a
  text in it ("deep"); a button, a field or a link in it does not, because Tauri starts no drag from those. A row has
  more than one band, so a band is no landmark. `data-code-band` tells the root layout that the page has its own top.
-->
<div
  class="band"
  class:clears={clearsWindowButtons && overlay}
  class:keeps={clearsWindowControls && frameless}
  data-code-band
  data-tauri-drag-region="deep"
>
  {#if start}<div class="part">{@render start()}</div>{/if}
  <div class="middle">{@render children?.()}</div>
  {#if end}<div class="part">{@render end()}</div>{/if}
</div>

<style>
  .band {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    /* 52px: 48 + 4. */
    block-size: calc(var(--space-12) + var(--space-1));
    padding-inline: var(--space-3);
    min-inline-size: 0;
    user-select: none;
  }

  /* The three window buttons of a Mac stand over the start of the first band (tauri.macos.conf.json). */
  .clears {
    padding-inline-start: calc(var(--titlebar-buttons-inline-size) + var(--space-3));
  }

  /* The room of the three window controls that the root layout draws at the end of the last band. */
  .keeps {
    padding-inline-end: calc(var(--control-md) * 3 + var(--space-1) * 2 + var(--space-2) + var(--space-3));
  }

  .part {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }

  .middle {
    display: flex;
    flex: 1;
    align-items: center;
    min-inline-size: 0;
    align-self: stretch;
  }
</style>
