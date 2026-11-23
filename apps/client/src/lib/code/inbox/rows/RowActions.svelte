<script lang="ts">
  import { press } from '$lib/motion/roles/press';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../../icons/icons';
  import Tooltip from '../../kit/Tooltip.svelte';
  import type { Anchor } from '$lib/overlay/helptip/placement';
  import type { RowAction } from '../actions';

  interface Props {
    kind: 'card' | 'slim';
    shelf: 'list' | 'snoozed' | 'settled';
    /** T3's canSnooze: no Snooze while the thread asks the person (08 inbox 16). */
    snoozes: boolean;
    /** The row shows Woke: a button that dismisses it, in sight also on hover (08 inbox 9). */
    woke: boolean;
    onDismiss: () => void;
    /** Why settle, snooze and wake are off; `null` when the runner keeps them. */
    marksOff: string | null;
    onAct: (action: RowAction) => void;
    /** Opens the row's whole menu under the button: the keyboard way to the menu that right-click opens (09 finding 6). */
    onMenu: (anchor: Anchor) => void;
  }

  const { kind, shelf, snoozes, woke, marksOff, onAct, onMenu, onDismiss }: Props = $props();

  interface Act {
    readonly action: RowAction;
    readonly label: string;
    readonly icon: CodeIconName;
    /** T3's sizes: 12 px for Snooze, Wake and a one-liner's Settle, 14 px for a card's Settle and Un-settle (08 inbox 17). */
    readonly small: boolean;
    readonly word?: string;
    /** Needs the marks of the runner (TH61). */
    readonly mark: boolean;
  }

  /** The on-row actions as in T3 Code: a card has Snooze and Settle, a one-liner its one way back; Unpin is the pin. */
  const acts = $derived.by((): Act[] => {
    if (shelf === 'snoozed') return [{ action: 'wake', label: 'Wake thread now', icon: 'clock', small: true, mark: true }];
    if (shelf === 'settled') return [{ action: 'unsettle', label: 'Un-settle thread', icon: 'undo-2', small: false, mark: true }];
    const list: Act[] = kind === 'card' && snoozes ? [{ action: 'snooze', label: 'Snooze thread', icon: 'clock', small: true, mark: true }] : [];
    list.push({ action: 'settle', label: 'Settle thread', icon: 'circle-check', small: kind === 'slim', word: kind === 'card' ? 'Settle' : undefined, mark: true });
    return list;
  });
</script>

<span class="acts {kind}">
  {#each acts as act (act.action)}
    {@const off = act.mark && marksOff !== null}
    <Tooltip text={off ? `${act.label}: ${marksOff ?? ''}` : act.label}>
      <button
        type="button"
        aria-label={act.label}
        aria-disabled={off ? 'true' : undefined}
        class:small={act.small}
        onclick={() => {
          if (!off) onAct(act.action);
        }}
        {@attach press()}
      >
        <CodeIcon name={act.icon} />{#if act.word !== undefined}<span aria-hidden="true">{act.word}</span>{/if}
      </button>
    </Tooltip>
  {/each}
  <Tooltip text="More actions">
    <button
      type="button"
      aria-label="More actions"
      aria-haspopup="menu"
      onclick={(event) => {
        const box = event.currentTarget.getBoundingClientRect();
        onMenu({ left: box.left, top: box.top, width: box.width, height: box.height });
      }}
      {@attach press()}
    >
      <CodeIcon name="ellipsis" />
    </button>
  </Tooltip>
  {#if woke}
    <Tooltip text="Dismiss Woke notification">
      <button type="button" class="woke" aria-label="Dismiss Woke notification" onclick={onDismiss}><CodeIcon name="alarm-clock" /><span>Woke</span></button>
    </Tooltip>
  {/if}
</span>

<style>
  /* Hidden at rest; on hover or keyboard focus in the row they take the place of the status (02 section 5.5). */
  .acts {
    position: absolute;
    inset-inline-end: var(--code-row-pad-inline);
    display: flex;
    align-items: center;
  }

  /* Hidden at rest but Woke, which stays in sight and can be pressed, as in T3 Code. */
  .acts > :global(.anchor:not(:has(.woke))) {
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--spring-snappy);
  }

  .card {
    /* 24 px, centred on the 20 px status line it covers. */
    inset-block-start: var(--code-card-pad-block);
    block-size: var(--space-6);
  }

  .slim {
    inset-block: 0;
  }

  :global(.row:hover) > .acts > :global(.anchor),
  /* Focus from the keys only: after a click the row keeps focus, and its status must come back (08 inbox 1). */
  :global(.row:has(:focus-visible)) > .acts > :global(.anchor) {
    opacity: 1;
    pointer-events: auto;
  }

  button {
    display: inline-flex;
    gap: var(--space-1);
    align-items: center;
    /* 24 px, the smallest target (09 finding 21). */
    block-size: var(--space-6);
    min-inline-size: var(--space-6);
    justify-content: center;
    padding: 0 calc(var(--space-1) * 1.5);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--code-quiet-text);
    font: inherit;
    font-size: var(--code-text-meta);
    cursor: pointer;
  }

  button :global(.code-icon) {
    inline-size: var(--code-icon-small);
    block-size: var(--code-icon-small);
  }

  button.small :global(.code-icon) {
    inline-size: var(--text-xs);
    block-size: var(--text-xs);
  }

  /* Woke: the 16 px alarm clock and the word in the degraded status colour; underlined on hover. */
  button.woke {
    color: var(--status-degraded-text);
    font-weight: var(--weight-medium);
  }

  button.woke :global(.code-icon) {
    inline-size: var(--icon-sm);
    block-size: var(--icon-sm);
  }

  button.woke:hover {
    color: var(--status-degraded-text);
    text-decoration: underline;
  }

  button:hover {
    color: var(--text);
  }

  button[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
    cursor: default;
  }

  button:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }
</style>
