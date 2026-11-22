<script lang="ts">
  import type { ComponentProps, Snippet } from 'svelte';
  import BrowserToolbar from './BrowserToolbar.svelte';
  import DeviceToolbar from './DeviceToolbar.svelte';
  import StartPage from './StartPage.svelte';
  import Unreachable from './Unreachable.svelte';
  import type { Viewport } from './viewport';

  /** What the tab shows under its row: the start page, a page, or the reason that a page did not load. */
  export type BrowserView = { readonly kind: 'start' } | { readonly kind: 'page' } | { readonly kind: 'unreachable'; readonly what?: 'site' | 'file'; readonly detail?: string | undefined };

  interface Props {
    view: BrowserView;
    toolbar: ComponentProps<typeof BrowserToolbar>;
    start: ComponentProps<typeof StartPage>;
    /** What draws the page: the surface gives it the room and the size, and holds no web view itself. */
    page: Snippet<[Viewport | undefined]>;
    /** The size of the page, while the device toolbar shows. */
    viewport?: Viewport | undefined;
    onviewport?: ((viewport: Viewport) => void) | undefined;
    /** The find bar, while it is open; it floats at the top of the page. */
    find?: Snippet | undefined;
    ondownload?: (() => void) | undefined;
  }

  const { view, toolbar, start, page, viewport, onviewport, find, ondownload }: Props = $props();
</script>

<!--
  The browser surface of the right panel (report 05, Section 6): the row with the address, then the device toolbar while
  it is on, then the start page, the page or the reason that it did not load.
-->
<div class="browser" data-surface="browser">
  <BrowserToolbar {...toolbar} />
  {#if viewport !== undefined && onviewport !== undefined}
    <DeviceToolbar {viewport} {onviewport} onclose={() => toolbar.onmore('device_toolbar')} />
  {/if}
  <div class="stage">
    {#if find !== undefined}<div class="find">{@render find()}</div>{/if}
    {#if view.kind === 'start'}
      <StartPage {...start} />
    {:else if view.kind === 'unreachable'}
      <Unreachable kind={view.what ?? 'site'} detail={view.detail} onreload={toolbar.onreload} onexternal={toolbar.onexternal} {ondownload} />
    {:else}
      <div class="frame" class:sized={viewport !== undefined}>
        <div class="page" style:inline-size={viewport === undefined ? undefined : `${String(viewport.width)}px`} style:block-size={viewport === undefined ? undefined : `${String(viewport.height)}px`}>
          {@render page(viewport)}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .browser {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
    background: var(--surface-base);
  }

  .stage {
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
  }

  .find {
    position: absolute;
    inset-block-start: var(--space-3);
    inset-inline-end: var(--space-3);
    z-index: var(--layer-sticky);
    inline-size: min(22rem, 100% - var(--space-6));
  }

  /* The bar counts its border and its padding in the width that it is given here. */
  .find > :global(*) {
    box-sizing: border-box;
  }

  .frame {
    display: flex;
    flex: 1;
    min-block-size: 0;
  }

  /* With a device size the page keeps that size and the frame scrolls to it. */
  .frame.sized {
    padding: var(--space-3);
    overflow: auto;
    background: var(--surface-sunken);
  }

  .page {
    display: flex;
    flex: 1;
    min-inline-size: 0;
  }

  .sized > .page {
    flex: none;
    margin-inline: auto;
    border: var(--border-thin) solid var(--code-line);
    background: var(--surface-base);
  }
</style>
