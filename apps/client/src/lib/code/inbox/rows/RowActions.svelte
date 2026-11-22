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
    pinned: boolean;
    /** Why settle, snooze and wake are off; `null` when the runner keeps them. */
    marksOff: string | null;
    onAct: (action: RowAction) => void;
    /** Opens the row's whole menu under the button: the keyboard way to the menu that right-click opens (09 finding 6). */
    onMenu: (anchor: Anchor) => void;
  }

  const { kind, shelf, pinned, marksOff, onAct, onMenu }: Props = $props();

  interface Act {
    readonly action: RowAction;
    readonly label: string;
    readonly icon: CodeIconName;
    readonly word?: string;
    /** Needs the marks of the runner (TH61). */
    readonly mark: boolean;
  }

  /** The on-row actions (02 sections 5.5 and 5.7): a card has Snooze and Settle, a one-liner its one way back. */
  const acts = $derived.by((): Act[] => {
    if (shelf === 'snoozed') return [{ action: 'wake', label: 'Wake thread now', icon: 'alarm-clock-off', mark: true }];
    if (shelf === 'settled') return [{ action: 'unsettle', label: 'Un-settle thread', icon: 'undo-2', mark: true }];
    const list: Act[] = pinned ? [{ action: 'unpin', label: 'Unpin thread', icon: 'pin-off', mark: false }] : [];
    if (kind === 'card') list.push({ action: 'snooze', label: 'Snooze thread', icon: 'clock', mark: true });
    list.push({ action: 'settle', label: 'Settle thread', icon: 'check', word: kind === 'card' ? 'Settle' : undefined, mark: true });
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
</span>

<style>
  /* Hidden at rest; on hover or keyboard focus in the row they take the place of the status (02 section 5.5). */
  .acts {
    position: absolute;
    inset-inline-end: var(--code-row-pad-inline);
    display: flex;
    align-items: center;
    opacity: 0;
    pointer-events: none;
  }

  .card {
    /* 24 px, centred on the 20 px status line it covers. */
    inset-block-start: var(--code-card-pad-block);
    block-size: var(--space-6);
  }

  .slim {
    inset-block: 0;
  }

  :global(.row:hover) > .acts,
  /* Focus from the keys only: after a click the row keeps focus, and its status must come back (08 inbox 1). */
  :global(.row:has(:focus-visible)) > .acts {
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
    inline-size: var(--text-sm);
    block-size: var(--text-sm);
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
