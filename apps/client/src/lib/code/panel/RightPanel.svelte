<script lang="ts">
  import type { Snippet } from 'svelte';
  import EmptyPanel from './EmptyPanel.svelte';
  import PanelEdge from './PanelEdge.svelte';
  import PanelTabBar from './PanelTabBar.svelte';
  import { type PanelTab, type PanelTabs, type SurfaceKind } from './tabs';
  import { clamped, maxWidth } from './width';

  interface Props {
    /** The panel shows. Closed, it has no width and takes no key and no press. */
    open: boolean;
    /** The panel fills the row and the chat column is hidden. */
    maximised?: boolean;
    state: PanelTabs;
    /** The width that is asked for; the panel holds it to its limits for the row that it is in. */
    width: number;
    onresize: (width: number) => void;
    onshow: (id: string) => void;
    onclose: (id: string) => void;
    /** The person asks for a surface: from the empty panel or its keys. */
    onopen: (kind: SurfaceKind) => void;
    onadd: (anchor: HTMLElement) => void;
    onmenu?: (id: string, event: MouseEvent) => void;
    unavailable?: Partial<Record<SurfaceKind, string>>;
    /** What stands at the end of the tab bar. */
    end?: Snippet;
    /** The content of the tab that shows. */
    surface: Snippet<[PanelTab]>;
  }

  const {
    open, maximised = false, state: panel, width, onresize, onshow, onclose, onopen, onadd, onmenu, unavailable, end, surface,
  }: Props = $props();

  let frame = $state<HTMLElement>();
  let limits = $state({ window: 1440, row: 1440 });
  const shownTab = $derived(panel.tabs.find((tab) => tab.id === panel.active));
  const held = $derived(clamped(width, limits.window, limits.row));

  // The limits follow the window and the row that the panel is in (report 02, Section 4.3).
  $effect(() => {
    const row = frame?.parentElement;
    if (!row) return;
    const measure = (): void => {
      limits = { window: window.innerWidth, row: row.getBoundingClientRect().width };
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(row);
    window.addEventListener('resize', measure);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', measure);
    };
  });
</script>

<!--
  The right panel of Code: one panel with tabs beside the chat column (report 02, Section 8). The width does not
  animate: the panel is there or it is not (MOT-006). Its inside keeps its width, so opening it uncovers the chat and
  does not squeeze the tab bar.
-->
<aside
  bind:this={frame}
  class="panel"
  class:maximised
  aria-label="Panel"
  inert={!open}
  style:inline-size={open ? (maximised ? undefined : `${held}px`) : '0px'}
>
  {#if open}
    <div class="inside" style:inline-size={maximised ? '100%' : `${held}px`}>
      {#if !maximised}
        <PanelEdge width={held} max={maxWidth(limits.window, limits.row)} {onresize} />
      {/if}
      <PanelTabBar state={panel} {onshow} {onclose} {onadd} {onmenu} {end} />
      {#if shownTab}
        <div class="surface" role="tabpanel" aria-label={shownTab.title}>{@render surface(shownTab)}</div>
      {:else}
        <EmptyPanel {onopen} {unavailable} />
      {/if}
    </div>
  {/if}
</aside>

<style>
  .panel {
    position: relative;
    display: flex;
    flex: none;
    justify-content: flex-end;
    min-block-size: 0;
    overflow: hidden;
    border-inline-start: var(--border-thin) solid var(--border-subtle);
    background: var(--surface-base);
  }

  .panel[inert] {
    border-inline-start: 0;
  }

  .maximised {
    flex: 1;
    border-inline-start: 0;
  }

  .inside {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    min-block-size: 0;
    block-size: 100%;
  }

  .surface {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-block-size: 0;
    overflow: auto;
  }
</style>
