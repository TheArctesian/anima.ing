<script lang="ts">
  import { tick } from 'svelte';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../icons/icons';
  import { press } from '$lib/motion/roles/press';
  import { popover } from '$lib/motion/roles/transitions';
  import { menuPlace, nextChoice, type MenuChoice, type MenuEntry, type MenuSide } from './menu';

  interface Props {
    /** The trigger's accessible name and the menu's: "More approval options". It holds the trigger's words (WCAG 2.5.3). */
    label: string;
    entries: readonly MenuEntry[];
    onChoose: (id: string) => void;
    /** The trigger: inline words (the approval row's "More") or a 28 px icon button. */
    face: { readonly text: string } | { readonly icon: CodeIconName };
    side?: MenuSide;
    disabled?: boolean;
    /** Open from the start, for the specimen and the gallery. */
    open?: boolean;
  }

  let { label, entries, onChoose, face, side = { side: 'below', align: 'start' }, disabled = false, open = $bindable(false) }: Props = $props();
  const id = $props.id();
  let root: HTMLElement | undefined;
  let trigger = $state<HTMLButtonElement>();
  const buttons: HTMLButtonElement[] = [];
  const choices = $derived(entries.filter((entry): entry is MenuChoice => entry.kind === 'choice'));

  async function show(from: 'Home' | 'End'): Promise<void> {
    open = true;
    await tick();
    const index = nextChoice(from, -1, choices);
    if (index !== null) buttons[index]?.focus();
  }

  function close(): void {
    open = false;
    trigger?.focus();
  }

  function choose(choice: MenuChoice): void {
    if (choice.disabled === true) return;
    open = false;
    trigger?.focus();
    onChoose(choice.id);
  }

  function onTriggerKey(event: KeyboardEvent): void {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    void show(event.key === 'ArrowDown' ? 'Home' : 'End');
  }

  function onMenuKey(event: KeyboardEvent): void {
    if (event.key === 'Escape' || event.key === 'Tab') {
      if (event.key === 'Escape') event.preventDefault();
      close();
      return;
    }
    const next = nextChoice(event.key, buttons.indexOf(document.activeElement as HTMLButtonElement), choices);
    if (next === null) return;
    event.preventDefault();
    buttons[next]?.focus();
  }

  function onOutside(event: PointerEvent): void {
    if (open && root !== undefined && event.target instanceof Node && !root.contains(event.target)) open = false;
  }

  function place(panel: HTMLElement): void {
    if (trigger === undefined) return;
    const spot = menuPlace(
      trigger.getBoundingClientRect(),
      { width: panel.offsetWidth, height: panel.offsetHeight },
      { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      { ...side, gap: Number.parseFloat(getComputedStyle(panel).marginBlockEnd) || 0 },
    );
    panel.style.insetInlineStart = `${String(spot.left)}px`;
    panel.style.insetBlockStart = `${String(spot.top)}px`;
  }

  function indexOf(choice: MenuChoice): number {
    return choices.indexOf(choice);
  }
</script>

<svelte:document onpointerdown={onOutside} />

<!-- A menu (02 section 3.5) on our opaque overlay: items 28 px, 4 by 8 px padding, arrows, Home, End, Escape. -->
<span class="root" bind:this={root}>
  <button
    type="button"
    class={'text' in face ? 'words' : 'square'}
    aria-label={label}
    aria-haspopup="menu"
    aria-expanded={open}
    aria-controls="{id}-menu"
    {disabled}
    bind:this={trigger}
    onclick={() => {
      if (open) open = false;
      else void show('Home');
    }}
    onkeydown={onTriggerKey}
    {@attach press()}
  >
    {#if 'text' in face}{face.text}{:else}<CodeIcon name={face.icon} />{/if}
  </button>
  {#if open}
    <div class="menu" id="{id}-menu" role="menu" aria-label={label} tabindex="-1" onkeydown={onMenuKey} transition:popover={{ from: trigger }} {@attach place}>
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
            bind:this={buttons[indexOf(entry)]}
            onclick={() => { choose(entry); }}
          >
            {#if entry.icon !== undefined}<CodeIcon name={entry.icon} />{/if}
            <span class="label">{entry.label}</span>
            {#if entry.detail !== undefined}<span class="detail">{entry.detail}</span>{/if}
            {#if entry.shortcut !== undefined}<span class="detail">{entry.shortcut}</span>{/if}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</span>

<style>
  .root {
    display: inline-flex;
  }

  button {
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
  }

  .words {
    padding: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
    font-weight: var(--weight-medium);
  }

  .words:hover:not(:disabled) {
    color: var(--text);
    text-decoration: underline;
  }

  .square {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: var(--code-control-dense);
    block-size: var(--code-control-dense);
    border-radius: var(--radius-md);
    color: var(--code-quiet-text);
  }

  .square:hover:not(:disabled),
  .square[aria-expanded='true'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  button:disabled {
    opacity: var(--opacity-disabled);
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: var(--border-focus);
  }

  .menu {
    position: fixed;
    z-index: var(--layer-popover);
    display: flex;
    flex-direction: column;
    min-inline-size: var(--code-menu-min);
    max-inline-size: calc(100vw - var(--space-8));
    margin-block-end: var(--code-offset);
    padding: var(--code-menu-pad);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
  }

  .menu [role='menuitem'] {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-block-size: var(--code-menu-item);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-size: var(--code-text-body);
    text-align: start;
  }

  .menu [role='menuitem'] :global(.code-icon) {
    color: var(--code-quiet-text);
  }

  .menu [role='menuitem']:hover,
  .menu [role='menuitem']:focus-visible {
    background: var(--surface-hover);
  }

  .menu [role='menuitem'].danger {
    color: var(--status-down-text);
  }

  .menu [role='menuitem'][aria-disabled='true'] {
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
