<script lang="ts">
  import { press } from '$lib/motion/roles/press';
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../../icons/icons';
  import Tooltip from '../../kit/Tooltip.svelte';
  import type { RowAction } from '../actions';

  interface Props {
    kind: 'card' | 'slim';
    shelf: 'list' | 'snoozed' | 'settled';
    pinned: boolean;
    /** Why settle, snooze and wake are off; `null` when the runner keeps them. */
    marksOff: string | null;
    onAct: (action: RowAction) => void;
  }

  const { kind, shelf, pinned, marksOff, onAct }: Props = $props();

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
    inset-block-start: calc(var(--code-card-pad-block) + var(--border-focus));
    block-size: var(--space-5);
  }

  .slim {
    inset-block: 0;
  }

  :global(.row:hover) > .acts,
  :global(.row:focus-within) > .acts {
    opacity: 1;
    pointer-events: auto;
  }

  button {
    display: inline-flex;
    gap: var(--space-1);
    align-items: center;
    block-size: var(--space-5);
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
