<script lang="ts">
  import CodeIcon from '../icons/CodeIcon.svelte';
  import { popover } from '$lib/motion/roles/transitions';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import { menuPlace, nextChoice, reasonOf, type MenuChoice, type MenuEntry, type MenuSide } from './menu';
  import Self from './MenuPanel.svelte';

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
    /** A submenu: its keys stay in it, and Left closes it back to its row. */
    nested?: boolean;
  }

  const { id, label, entries, anchor, side = { side: 'below', align: 'start' }, from, focusFirst = true, onChoose, onClose, nested = false }: Props = $props();
  let panel = $state<HTMLElement>();
  const buttons: HTMLButtonElement[] = [];
  const choices = $derived(entries.filter((entry): entry is MenuChoice => entry.kind === 'choice'));

  /** A disabled choice names the line that says why it is off (09 finding 23). */
  function described(choice: MenuChoice, row: number): string | undefined {
    const reason = choice.disabled === true ? reasonOf(entries, row) : null;
    return reason === null ? undefined : `${id}-reason-${String(reason)}`;
  }

  /** The choice whose submenu is open, by its index in `choices`, and whether its first row takes focus. */
  let sub = $state<{ at: number; focus: boolean } | null>(null);

  function choose(choice: MenuChoice): void {
    if (choice.disabled === true) return;
    if (choice.items !== undefined) {
      sub = { at: choices.indexOf(choice), focus: true };
      return;
    }
    onClose(true);
    onChoose(choice.id);
  }

  function onkeydown(event: KeyboardEvent): void {
    // A submenu's keys are its own; Tab goes on up, so the whole menu closes.
    if (nested && event.key !== 'Tab') event.stopPropagation();
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === 'ArrowRight' && choices[at]?.items !== undefined && choices[at].disabled !== true) {
      event.preventDefault();
      sub = { at, focus: true };
      return;
    }
    if (nested && event.key === 'ArrowLeft') {
      event.preventDefault();
      onClose(true);
      return;
    }
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
      <div class="group" id={entry.reason === true ? `${id}-reason-${String(row)}` : undefined} role="presentation">{entry.label}</div>
    {:else}
      <button
        type="button"
        role="menuitem"
        tabindex="-1"
        class:danger={entry.danger === true}
        aria-disabled={entry.disabled === true ? 'true' : undefined}
        aria-describedby={described(entry, row)}
        aria-haspopup={entry.items === undefined ? undefined : 'menu'}
        aria-expanded={entry.items === undefined ? undefined : sub?.at === choices.indexOf(entry)}
        onpointerenter={() => {
          const at = choices.indexOf(entry);
          if (entry.items !== undefined && entry.disabled !== true) sub = { at, focus: false };
          else if (sub !== null && sub.at !== at) sub = null;
        }}
        bind:this={buttons[choices.indexOf(entry)]}
        onclick={() => {
          choose(entry);
        }}
      >
        {#if entry.icon !== undefined}<CodeIcon name={entry.icon} />{/if}
        <span class="label">{entry.label}</span>
        {#if entry.detail !== undefined}<span class="detail">{entry.detail}</span>{/if}
        {#if entry.shortcut !== undefined}<span class="detail">{entry.shortcut}</span>{/if}
        {#if entry.items !== undefined}<CodeIcon name="chevron-right" />{/if}
      </button>
      {#if entry.items !== undefined && sub?.at === choices.indexOf(entry)}
        {@const at = choices.indexOf(entry)}
        <Self
          id="{id}-sub-{String(at)}"
          label={entry.label}
          entries={entry.items}
          side={{ side: 'end', align: 'start' }}
          from={buttons[at]}
          anchor={() => buttons[at]?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 }}
          focusFirst={sub.focus}
          nested
          onChoose={(chosen) => {
            onClose(true);
            onChoose(chosen);
          }}
          onClose={(back) => {
            sub = null;
            if (back) buttons[at]?.focus();
          }}
        />
      {/if}
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
