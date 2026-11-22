<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from '$lib/icons/Icon.svelte';
  import { capabilities } from '$lib/platform/platform';
  import { iconOf, type PanelTabs } from './tabs';

  interface Props {
    state: PanelTabs;
    onshow: (id: string) => void;
    onclose: (id: string) => void;
    /** Opens the menu of the surfaces at the add button. */
    onadd: (anchor: HTMLElement) => void;
    /** Opens the menu of one tab (close others, copy path), at the pointer. */
    onmenu?: (id: string, event: MouseEvent) => void;
    /** What stands at the end of the bar: the buttons of the panels. */
    end?: Snippet;
  }

  const { state: panel, onshow, onclose, onadd, onmenu, end }: Props = $props();
  const frameless = capabilities().framelessWindow;

  /** The arrows move between the tabs, as in a tab list; Delete closes the tab that has the focus. */
  function onkeydown(event: KeyboardEvent, index: number): void {
    const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
    const tab = panel.tabs[index];
    if (step !== 0) {
      const next = panel.tabs[(index + step + panel.tabs.length) % panel.tabs.length];
      if (next) onshow(next.id);
    } else if ((event.key === 'Delete' || event.key === 'Backspace') && tab) {
      onclose(tab.id);
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<!--
  The tab bar of the right panel: 52px, the last band of its row (report 02, Section 8.2). It moves the window, and it
  keeps the room of the window controls of a window with no frame. A tab and its close button are two buttons side by
  side, not one in the other, and the close button is there without hover (R-352: the faults of T3 Code are corrected).
-->
<div class="bar" class:keeps={frameless} data-code-band data-tauri-drag-region="deep">
  <div class="strip" role="tablist" aria-label="Panel surfaces">
    {#each panel.tabs as tab, index (tab.id)}
      {@const active = tab.id === panel.active}
      <div class="tab" class:active>
        <button
          type="button"
          role="tab"
          class="name"
          aria-selected={active}
          tabindex={active ? 0 : -1}
          title={tab.path ?? tab.title}
          onclick={() => {
            onshow(tab.id);
          }}
          onauxclick={(event) => {
            if (event.button === 1) onclose(tab.id);
          }}
          oncontextmenu={(event) => {
            if (!onmenu) return;
            event.preventDefault();
            onmenu(tab.id, event);
          }}
          onkeydown={(event) => {
            onkeydown(event, index);
          }}
        >
          <Icon name={iconOf(tab)} size="sm" />
          <span>{tab.title}</span>
        </button>
        <button
          type="button"
          class="close"
          aria-label="Close {tab.title}"
          tabindex={active ? 0 : -1}
          onclick={() => {
            onclose(tab.id);
          }}
        >
          <Icon name="carbon:close" size="sm" />
        </button>
      </div>
    {/each}
  </div>
  <button
    type="button"
    class="add"
    aria-label="Add panel surface"
    title="Add panel surface"
    onclick={(event) => {
      onadd(event.currentTarget);
    }}
  >
    <Icon name="carbon:add" size="sm" />
  </button>
  <div class="room"></div>
  {#if end}<div class="end">{@render end()}</div>{/if}
</div>

<style>
  .bar {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    block-size: calc(var(--space-12) + var(--space-1));
    padding-inline: var(--space-2) var(--space-3);
    min-inline-size: 0;
    user-select: none;
  }

  /* The room of the three window controls that the root layout draws over the end of the last band. */
  .keeps {
    padding-inline-end: calc(var(--control-md) * 3 + var(--space-1) * 2 + var(--space-2) + var(--space-3));
  }

  .strip {
    display: flex;
    gap: var(--space-1);
    min-inline-size: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .tab {
    display: flex;
    flex: none;
    align-items: center;
    /* 144px with the close button, as the tab of T3 Code. */
    max-inline-size: calc(var(--space-12) * 3);
    block-size: var(--control-sm);
    border-radius: var(--radius-md);
    color: var(--text-secondary);
  }

  .tab:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .active,
  .active:hover {
    background: var(--surface-selected);
    color: var(--text);
  }

  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    block-size: var(--control-sm);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }

  .name {
    flex: 1;
    gap: var(--space-1);
    min-inline-size: 0;
    padding-inline: var(--space-2) var(--space-1);
  }

  .name span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close,
  .add {
    inline-size: var(--control-sm);
    color: var(--text-secondary);
  }

  .close:hover,
  .add:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--accent);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .room {
    flex: 1;
  }

  .end {
    display: flex;
    flex: none;
    gap: var(--space-1);
  }
</style>
