<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { popover } from '$lib/motion/roles/transitions';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import { menuPlace, nextChoice, type MenuChoice, type MenuEntry, type MenuSide } from './menu';

  interface Props {
    /** The panel's ID, which a trigger names in `aria-controls`. */
    id: string;
    label: string;
    entries: readonly MenuEntry[];
    /** The rectangle the panel opens beside: a trigger, or the pointer of a right-click (a rectangle of no size). */
    anchor: () => Anchor;
    side?: MenuSide;
    /** The element the panel grows from, and that a pointer press on does not close it (the trigger). */
    from?: HTMLElement | undefined;
    /** Focus the first choice when it opens: from the keys always; from the pointer too, so arrows work at once. */
    focusFirst?: boolean;
    onChoose: (id: string) => void;
    /** The panel closes; `back` says whether focus goes back to where it came from (Escape, a choice) or stays (a press outside, Tab). */
    onClose: (back: boolean) => void;
  }

  const { id, label, entries, anchor, side = { side: 'below', align: 'start' }, from, focusFirst = true, onChoose, onClose }: Props = $props();
  let panel = $state<HTMLElement>();
  const buttons: HTMLButtonElement[] = [];
  const choices = $derived(entries.filter((entry): entry is MenuChoice => entry.kind === 'choice'));

  function choose(choice: MenuChoice): void {
    if (choice.disabled === true) return;
    onClose(true);
    onChoose(choice.id);
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' || event.key === 'Tab') {
      if (event.key === 'Escape') event.preventDefault();
      onClose(event.key === 'Escape');
      return;
    }
    const next = nextChoice(event.key, buttons.indexOf(document.activeElement as HTMLButtonElement), choices);
    if (next === null) return;
    event.preventDefault();
    buttons[next]?.focus();
  }

  function onOutside(event: PointerEvent): void {
    if (!(event.target instanceof Node) || panel?.contains(event.target) === true || from?.contains(event.target) === true) return;
    onClose(false);
  }

  function place(node: HTMLElement): void {
    const spot = menuPlace(
      anchor(),
      { width: node.offsetWidth, height: node.offsetHeight },
      { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      { ...side, gap: Number.parseFloat(getComputedStyle(node).marginBlockEnd) || 0 },
    );
    node.style.insetInlineStart = `${String(spot.left)}px`;
    node.style.insetBlockStart = `${String(spot.top)}px`;
    if (!focusFirst) return;
    const first = nextChoice('Home', -1, choices);
    (first === null ? node : buttons[first])?.focus();
  }
</script>

<svelte:document onpointerdown={onOutside} />

<!-- A menu (02 section 3.5) on our opaque overlay: items 28 px, 4 by 8 px padding, arrows, Home, End, Escape. -->
<div class="menu" {id} role="menu" aria-label={label} tabindex="-1" bind:this={panel} {onkeydown} transition:popover={{ from }} {@attach place}>
  {#each entries as entry, row (row)}
    {#if entry.kind === 'line'}
      <div class="line" role="separator"></div>
    {:else if entry.kind === 'group'}
      <div class="group" role="presentation">{entry.label}</div>
    {:else}
      <button
        type="button"
        role="menuitem"
        tabindex="-1"
        class:danger={entry.danger === true}
        aria-disabled={entry.disabled === true ? 'true' : undefined}
        bind:this={buttons[choices.indexOf(entry)]}
        onclick={() => {
          choose(entry);
        }}
      >
        {#if entry.icon !== undefined}<CodeIcon name={entry.icon} />{/if}
        <span class="label">{entry.label}</span>
        {#if entry.detail !== undefined}<span class="detail">{entry.detail}</span>{/if}
        {#if entry.shortcut !== undefined}<span class="detail">{entry.shortcut}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: var(--layer-popover);
    display: flex;
    flex-direction: column;
    min-inline-size: var(--code-menu-min);
    max-inline-size: min(var(--code-tip-max), calc(100vw - var(--space-8)));
    margin-block-end: var(--code-offset);
    padding: var(--code-menu-pad);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
  }

  .menu:focus-visible {
    outline: none;
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
  }

  button :global(.code-icon) {
    color: var(--code-quiet-text);
  }

  button:hover,
  button:focus-visible {
    background: var(--surface-hover);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  button.danger {
    color: var(--status-down-text);
  }

  button[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
  }

  .label {
    flex: 1;
    white-space: nowrap;
  }

  .detail {
    padding-inline-start: var(--space-4);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
    font-variant-numeric: tabular-nums;
  }

  .group {
    padding: var(--space-2) var(--space-2) var(--space-1);
    line-height: var(--leading-tight);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }

  .line {
    block-size: var(--border-thin);
    margin: var(--space-1) var(--space-2);
    background: var(--border-strong);
  }
</style>
