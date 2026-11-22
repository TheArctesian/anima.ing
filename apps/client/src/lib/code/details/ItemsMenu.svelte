<script lang="ts">
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { ITEMS, atDefault, shows, toggled, type ItemChoices } from './items';
  import Popup from './row/Popup.svelte';

  interface Props {
    choices: ItemChoices;
    anchor: () => Anchor;
    from?: HTMLElement | undefined;
    onChange: (choices: ItemChoices) => void;
    onClose: (back: boolean) => void;
  }

  const { choices, anchor, from, onChange, onClose }: Props = $props();
  let menu = $state<HTMLElement>();

  /** Arrow keys, Home and End walk the rows; a switch keeps the menu open so that several can change. */
  function onkeydown(event: KeyboardEvent): void {
    const rows = [...(menu?.querySelectorAll<HTMLElement>('button:not(:disabled)') ?? [])];
    const at = rows.indexOf(document.activeElement as HTMLElement);
    const to = event.key === 'ArrowDown' ? (at + 1) % rows.length : event.key === 'ArrowUp' ? (at <= 0 ? rows.length - 1 : at - 1) : event.key === 'Home' ? 0 : event.key === 'End' ? rows.length - 1 : -1;
    if (to < 0) return;
    event.preventDefault();
    rows[to]?.focus();
  }

  $effect(() => {
    menu?.querySelector<HTMLElement>('button')?.focus();
  });
</script>

<!-- "Choose what to show" (07 section 8): twelve switches and a reset. It opens by a right-click on the card. -->
<Popup {anchor} {from} {onClose}>
  <div class="menu" role="menu" aria-label="Choose what to show" tabindex="-1" bind:this={menu} {onkeydown}>
    <div class="group" role="presentation">Show in thread details</div>
    {#each ITEMS as item (item.id)}
      {@const on = shows(choices, item.id)}
      <button type="button" role="menuitemcheckbox" aria-checked={on} onclick={() => onChange(toggled(choices, item.id))}>
        <span class="mark">{#if on}<CodeIcon name="check" />{/if}</span>
        {item.label}
      </button>
    {/each}
    <div class="line" role="separator"></div>
    <button
      type="button"
      role="menuitem"
      disabled={atDefault(choices)}
      onclick={() => {
        onChange({});
        onClose(true);
      }}
    >
      <span class="mark"></span>
      Reset to default
    </button>
  </div>
</Popup>

<style>
  .menu {
    display: flex;
    flex-direction: column;
    padding: var(--code-menu-pad);
    overflow-y: auto;
  }

  .group {
    padding: var(--space-1) var(--space-2);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }

  button {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--code-menu-item);
    padding: var(--space-1) var(--space-2);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-body);
    text-align: start;
    white-space: nowrap;
  }

  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--surface-hover);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  button:disabled {
    opacity: var(--opacity-disabled);
  }

  .mark {
    display: inline-flex;
    flex: none;
    inline-size: var(--icon-sm);
    block-size: var(--icon-sm);
  }

  .line {
    block-size: var(--border-thin);
    margin: var(--space-1) var(--space-2);
    background: var(--border-strong);
  }
</style>
