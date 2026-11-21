<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** The band's middle: the breadcrumb of a chat, the search of the inbox, the tabs of the right panel. */
    children: Snippet;
    /** Controls at the start, before the middle. */
    start?: Snippet;
    /** Controls at the end: the panel toggles. */
    end?: Snippet;
    /** A line under the band, as on the page with no chat open; a chat's band has none. */
    ruled?: boolean;
  }

  const { children, start, end, ruled = false }: Props = $props();
</script>

<!--
  The 52 px top band of each column of the Code section (02 sections 4.2 and 6.1). In the frameless window the band is
  where the window is dragged; `data-code-band` marks it for the desktop shell, which owns the drag region (D1b).
-->
<div class="band" class:ruled data-code-band>
  {#if start}<div class="side">{@render start()}</div>{/if}
  <div class="middle">{@render children()}</div>
  {#if end}<div class="side">{@render end()}</div>{/if}
</div>

<style>
  .band {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    block-size: var(--code-band);
    min-inline-size: 0;
    padding-inline: var(--space-3);
    background: var(--surface-base);
  }

  .ruled {
    border-block-end: var(--border-thin) solid var(--border-subtle);
  }

  .side {
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
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
  }

  @media (width >= 40rem) {
    .band {
      padding-inline: var(--space-5);
    }
  }
</style>
