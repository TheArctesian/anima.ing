<script lang="ts">
  import type { Snippet } from 'svelte';
  import { capabilities } from '$lib/platform/platform';
  import WindowControls from './WindowControls.svelte';

  interface Props {
    /** What stands at the start of the band, for example the button of the sidebar. */
    start?: Snippet;
    /** The middle of the band: the path "project / title" of a thread. */
    children?: Snippet;
    /** What stands at the end, before the window controls: the buttons of the panels. */
    end?: Snippet;
    /** This band is the last one of its row, so the window controls of a window with no frame stand at its end. */
    holdsWindowControls?: boolean;
    /** This band is the first one of its row, so on a Mac it leaves room for the three window buttons. */
    clearsWindowButtons?: boolean;
  }

  const { start, children, end, holdsWindowControls = false, clearsWindowButtons = false }: Props = $props();
  const frameless = capabilities().framelessWindow;
  const overlay = capabilities().overlayTitleBar;
</script>

<!--
  The top band of a column of Code: 52px, as in T3 Code (report 02, Section 4.2). The band moves the window; a button,
  a field or a link in it does not, because the drag region of Tauri is the element itself and not what is in it.
-->
<header class="band" class:clears={clearsWindowButtons && overlay} data-tauri-drag-region>
  {#if start}<div class="part">{@render start()}</div>{/if}
  <div class="middle" data-tauri-drag-region>{@render children?.()}</div>
  {#if end}<div class="part">{@render end()}</div>{/if}
  {#if holdsWindowControls && frameless}<WindowControls />{/if}
</header>

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
