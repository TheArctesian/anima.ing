<script lang="ts">
  import { tick } from 'svelte';
  import Tabs from '$lib/tabs/Tabs.svelte';
  import Icon from '$lib/icons/Icon.svelte';
  import { TAB_ITEMS, type TabName } from './tabs';

  const { selected, onSelect }: { selected: TabName; onSelect: (id: string) => void } = $props();
  const third = $derived(selected === 'overview' || selected === 'checks' ? 'sessions' : selected);
  const visible = $derived(TAB_ITEMS.filter((tab) => tab.id === 'overview' || tab.id === 'checks' || tab.id === third));
  const other = $derived(TAB_ITEMS.filter((tab) => !visible.includes(tab)));
  let menu = $state<HTMLDetailsElement>();

  async function choose(id: string): Promise<void> {
    if (menu) menu.open = false;
    onSelect(id);
    await tick();
    menu?.querySelector('summary')?.focus();
  }
</script>

<div class="strip">
  <Tabs items={visible} {selected} label="Device" idPrefix="device" {onSelect} />
  <details bind:this={menu}>
    <summary>More<Icon name="carbon:chevron-down" size="sm" /></summary>
    <ul>
      {#each other as tab (tab.id)}
        <li><button type="button" aria-current={tab.id === selected ? 'page' : undefined} onclick={() => void choose(tab.id)}>{tab.label}</button></li>
      {/each}
    </ul>
  </details>
</div>

<style>
  .strip {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
  }

  details {
    position: relative;
    z-index: var(--layer-sticky);
    border-block-end: var(--border-thin) solid var(--border-subtle);
  }

  summary {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-block-size: var(--target-min-touch);
    padding-inline: var(--space-2);
    color: var(--text-secondary);
    cursor: pointer;
    list-style: none;
  }

  summary::-webkit-details-marker { display: none; }

  ul {
    position: absolute;
    inset-inline-end: var(--space-2);
    min-inline-size: 9rem;
    margin: 0;
    padding: var(--space-1);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    list-style: none;
  }

  button {
    inline-size: 100%;
    min-block-size: var(--target-min-touch);
    padding-inline: var(--space-2);
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: start;
  }
</style>
